use std::time::Instant;

use tinyvec::ArrayVec;
use yukari_movegen::{Board, Move, Zobrist};

use crate::eval::EvalState;

const MATE_VALUE: i32 = 10_000;

// TODO: when 50-move rule is implemented, this can be limited to searching from the last irreversible move.
#[must_use]
pub fn is_repetition_draw(keystack: &[u64], hash: u64) -> bool {
    keystack.iter().filter(|key| **key == hash).count() >= 3
}

pub struct Search<'a> {
    nodes: u64,
    qnodes: u64,
    nullmove_attempts: u64,
    nullmove_success: u64,
    stop_after: Option<Instant>,
    zobrist: &'a Zobrist,
}

impl<'a> Search<'a> {
    #[must_use]
    pub const fn new(stop_after: Option<Instant>, zobrist: &'a Zobrist) -> Self {
        Self {
            nodes: 0,
            qnodes: 0,
            nullmove_attempts: 0,
            nullmove_success: 0,
            stop_after,
            zobrist,
        }
    }

    fn quiesce(
        &mut self,
        board: &Board,
        mut alpha: i32,
        beta: i32,
        eval: &EvalState,
        pv: &mut ArrayVec<[Move; 32]>,
    ) -> i32 {
        let eval_int = eval.get(board.side());

        pv.set_len(0);

        if eval_int >= beta {
            return beta;
        }
        alpha = alpha.max(eval_int);

        board.generate_captures_coro(|m| {
            self.qnodes += 1;

            let eval = eval.clone().update_eval(board, m);

            // Pre-empt stand pat by skipping moves with bad evaluation.
            // One can think of this as delta pruning, with the delta being zero.
            if eval.get(board.side()) <= alpha {
                return true;
            }

            let board = board.make(m, self.zobrist);
            let mut child_pv = ArrayVec::new();
            let score = -self.quiesce(&board, -beta, -alpha, &eval, &mut child_pv);

            if score >= beta {
                alpha = beta;
                return false;
            }

            if score > alpha {
                alpha = score;
                pv.set_len(0);
                pv.push(m);
                for m in child_pv {
                    pv.push(m);
                }
            }

            true
        });

        alpha
    }

    #[allow(clippy::too_many_arguments)]
    fn search(
        &mut self,
        board: &Board,
        mut depth: i32,
        mut lower_bound: i32,
        upper_bound: i32,
        eval: &EvalState,
        pv: &mut ArrayVec<[Move; 32]>,
        mate: i32,
        keystack: &mut Vec<u64>,
    ) -> i32 {
        // Check extension
        if board.in_check() {
            depth += 1;
        }

        if depth <= 0 {
            return self.quiesce(board, lower_bound, upper_bound, eval, pv);
        }

        const R: i32 = 3;

        if !board.in_check() && depth >= 2 {
            keystack.push(board.hash());
            let board = board.make_null(self.zobrist);
            let mut child_pv = ArrayVec::new();
            let score = -self.search(
                &board,
                depth - 1 - R,
                -upper_bound,
                -upper_bound + 1,
                eval,
                &mut child_pv,
                mate,
                keystack,
            );
            keystack.pop();

            self.nullmove_attempts += 1;

            if score >= upper_bound {
                self.nullmove_success += 1;
                return upper_bound;
            }
        }

        if !board.in_check() && depth == 1 && eval.get(board.side()) - 200 >= upper_bound {
            return upper_bound;
        }

        let moves: [Move; 256] = [Move::default(); 256];
        let mut moves = ArrayVec::from(moves);
        moves.set_len(0);
        board.generate(&mut moves);

        // Is this checkmate or stalemate?
        if moves.is_empty() {
            pv.set_len(0);
            if board.in_check() {
                return -mate;
            }
            return 0;
        }

        // Is this a repetition draw?
        if is_repetition_draw(keystack, board.hash()) {
            pv.set_len(0);
            return 0;
        }

        let mut finding_pv = true;

        for m in moves {
            self.nodes += 1;

            let mut child_pv = ArrayVec::new();
            let eval = eval.clone().update_eval(board, m);
            let board = board.make(m, self.zobrist);
            let mut score;

            // Push the move to check for repetition draws
            keystack.push(board.hash());
            if finding_pv {
                score = -self.search(
                    &board,
                    depth - 1,
                    -upper_bound,
                    -lower_bound,
                    &eval,
                    &mut child_pv,
                    mate - 1,
                    keystack,
                );
            } else {
                score = -self.search(
                    &board,
                    depth - 1,
                    -lower_bound - 1,
                    -lower_bound,
                    &eval,
                    &mut child_pv,
                    mate - 1,
                    keystack,
                );
                if score > lower_bound {
                    score = -self.search(
                        &board,
                        depth - 1,
                        -upper_bound,
                        -lower_bound,
                        &eval,
                        &mut child_pv,
                        mate - 1,
                        keystack,
                    );
                }
            }
            keystack.pop();

            if score >= upper_bound {
                pv.set_len(0);
                return upper_bound;
            }

            if self.nodes.trailing_zeros() >= 10 {
                if let Some(time) = self.stop_after {
                    if Instant::now() >= time {
                        pv.set_len(0);
                        return lower_bound;
                    }
                }
            }

            if score > lower_bound {
                lower_bound = score;
                pv.set_len(0);
                pv.push(m);
                for m in child_pv {
                    pv.push(m);
                }
                finding_pv = false;
            }
        }
        lower_bound
    }

    pub fn search_root(
        &mut self,
        board: &Board,
        depth: i32,
        pv: &mut ArrayVec<[Move; 32]>,
        keystack: &mut Vec<u64>,
    ) -> i32 {
        let eval = EvalState::eval(board);
        self.search(
            board, depth, -100_000, 100_000, &eval, pv, MATE_VALUE, keystack,
        )
    }

    #[must_use]
    pub const fn nodes(&self) -> u64 {
        self.nodes
    }

    #[must_use]
    pub const fn qnodes(&self) -> u64 {
        self.qnodes
    }

    #[must_use]
    pub fn nullmove_success(&self) -> f64 {
        100.0 * (self.nullmove_success as f64) / (self.nullmove_attempts as f64)
    }
}
