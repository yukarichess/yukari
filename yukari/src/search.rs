use std::{time::Instant};

use yukari_movegen::{Board, Move, Zobrist};
use tinyvec::ArrayVec;

use crate::eval::{Eval, EvalState};

const MATE_VALUE: i32 = 10_000;

pub struct Search<'a> {
    eval: Eval,
    nodes: u64,
    qnodes: u64,
    stop_after: Option<Instant>,
    zobrist: &'a Zobrist,
}

impl<'a> Search<'a> {
    pub fn new(stop_after: Option<Instant>, zobrist: &'a Zobrist) -> Self {
        Self {
            eval: Eval::new(),
            nodes: 0,
            qnodes: 0,
            stop_after,
            zobrist,
        }
    }

    fn quiesce(&mut self, board: &Board, mut alpha: i32, beta: i32, eval: &EvalState) -> i32 {
        let eval_int = eval.get(board.side());

        if eval_int >= beta {
            return beta;
        }
        alpha = alpha.max(eval_int);

        board.generate_captures_incremental(|m| {
            self.qnodes += 1;

            let eval = self.eval.update_eval(board, &m, eval);

            // Pre-empt stand pat by skipping moves with bad evaluation.
            // One can think of this as delta pruning, with the delta being zero.
            if eval.get(board.side()) <= alpha {
                return true;
            }

            let board = board.make(m, self.zobrist);
            alpha = alpha.max(-self.quiesce(&board, -beta, -alpha, &eval));

            if alpha >= beta {
                alpha = beta;
                return false;
            }
            true
        });

        alpha
    }

    #[allow(clippy::too_many_arguments)]
    fn search(&mut self, board: &Board, depth: i32, mut alpha: i32, beta: i32, eval: &EvalState, pv: &mut ArrayVec<[Move; 32]>, mate: i32, keystack: &mut Vec<u64>) -> i32 {
        if depth <= 0 {
            pv.set_len(0);
            return self.quiesce(board, alpha, beta, eval);
        }

        const R: i32 = 3;

        if !board.in_check() && depth >= R {
            keystack.push(board.hash());
            let board = board.make_null(self.zobrist);
            let mut child_pv = ArrayVec::new();
            let score = -self.search(&board, depth - 1 - R, -beta, -beta + 1, eval, &mut child_pv, mate, keystack);
            keystack.pop();

            if score >= beta {
                return beta;
            }
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
            } else {
                return 0;
            }
        }

        // Is this a repetition draw?
        if keystack.iter().filter(|hash| **hash == board.hash()).count() >= 2 {
            pv.set_len(0);
            return 0;
        }

        keystack.push(board.hash());

        for m in moves {
            self.nodes += 1;

            let mut child_pv = ArrayVec::new();
            let eval = self.eval.update_eval(board, &m, eval);
            let board = board.make(m, self.zobrist);
            let score = -self.search(&board, depth - 1, -beta, -alpha, &eval, &mut child_pv, mate - 1, keystack);

            if score >= beta {
                keystack.pop();
                pv.set_len(0);
                return beta;
            }

            if self.nodes & 1023 == 0 {
                if let Some(time) = self.stop_after {
                    if Instant::now() >= time {
                        keystack.pop();
                        pv.set_len(0);
                        return alpha;
                    }
                }
            }

            if score > alpha {
                alpha = score;
                pv.set_len(0);
                pv.push(m);
                for m in child_pv {
                    pv.push(m);
                }
            }
        }

        keystack.pop();
        alpha
    }

    pub fn search_root(&mut self, board: &Board, depth: i32, pv: &mut ArrayVec<[Move; 32]>, keystack: &mut Vec<u64>) -> i32 {
        let eval = self.eval.eval(board);
        self.search(board, depth, -100_000, 100_000, &eval, pv, MATE_VALUE, keystack)
    }

    pub fn nodes(&self) -> u64 {
        self.nodes
    }

    pub fn qnodes(&self) -> u64 {
        self.qnodes
    }

    pub fn from_tuning_weights(&mut self, weights: &[i32]) {
        self.eval.from_tuning_weights(weights);
    }
}
