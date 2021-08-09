use std::{time::Instant};

use yukari_movegen::{Board, Move, Zobrist};
use tinyvec::ArrayVec;

use crate::{eval::{Eval, EvalState}, tt::TranspositionTable};

const MATE_VALUE: i32 = 10_000;

// TODO: when 50-move rule is implemented, this can be limited to searching from the last irreversible move.
pub fn is_repetition_draw(keystack: &[u64], hash: u64) -> bool {
    keystack.iter().filter(|key| **key == hash).count() >= 3
}

pub struct Search<'a> {
    eval: Eval,
    nodes: u64,
    qnodes: u64,
    nullmove_attempts: u64,
    nullmove_success: u64,
    stop_after: Option<Instant>,
    zobrist: &'a Zobrist,
    tt: TranspositionTable<(i8, i32, i8)>
}

impl<'a> Search<'a> {
    pub fn new(stop_after: Option<Instant>, zobrist: &'a Zobrist) -> Self {
        Self {
            eval: Eval::new(),
            nodes: 0,
            qnodes: 0,
            nullmove_attempts: 0,
            nullmove_success: 0,
            stop_after,
            zobrist,
            tt: TranspositionTable::new(1024*1024*16)
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
    fn search(&mut self, board: &Board, mut depth: i32, mut alpha: i32, beta: i32, eval: &EvalState, pv: &mut ArrayVec<[Move; 32]>, mate: i32, keystack: &mut Vec<u64>) -> i32 {
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

            self.nullmove_attempts += 1;

            if score >= beta {
                self.nullmove_success += 1;
                return beta;
            }
        }

        if !board.in_check() && depth == 1 && eval.get(board.side()) - 200 >= beta {
            return beta;
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
        if is_repetition_draw(keystack, board.hash()) {
            pv.set_len(0);
            return 0;
        }

        // Attempt to look up the move in the transposition table
        if let Some(&(tt_depth, tt_score, bound)) = self.tt.get(board.hash()) {
            if tt_depth as i32 >= depth {
                if bound == 0 {
                    return tt_score;
                } else if bound == 1 && tt_score <= alpha {
                    return alpha;
                } else if bound == 2 && tt_score >= beta {
                    return beta;
                }
            }
        }

        // Check extension
        if board.in_check() {
            depth += 1;
        }

        keystack.push(board.hash());

        let mut found_pv = false;

        for m in moves {
            self.nodes += 1;

            let mut child_pv = ArrayVec::new();
            let eval = self.eval.update_eval(board, &m, eval);
            let board = board.make(m, self.zobrist);
            let mut score;

            if !found_pv {
                score = -self.search(&board, depth - 1, -beta, -alpha, &eval, &mut child_pv, mate - 1, keystack);
            } else {
                score = -self.search(&board, depth - 1, -alpha - 1, -alpha, &eval, &mut child_pv, mate - 1, keystack);
                if score > alpha {
                    score = -self.search(&board, depth - 1, -beta, -alpha, &eval, &mut child_pv, mate - 1, keystack);
                }
            }

            if score >= beta {
                keystack.pop();
                pv.set_len(0);
                self.tt.set(board.hash(), (depth as i8, beta, 2));
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
                found_pv = true;
            }
        }

        keystack.pop();

        self.tt.set(board.hash(), (depth as i8, alpha, if found_pv { 0 } else { 1 }));

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

    pub fn nullmove_success(&self) -> f64 {
        100.0 * (self.nullmove_success as f64) / (self.nullmove_attempts as f64)
    }

    pub fn from_tuning_weights(&mut self, weights: &[i32]) {
        self.eval.from_tuning_weights(weights);
    }

    #[cfg(test)]
    pub fn DONOTUSE_get_tt(&self) -> &TranspositionTable<(i8, i32, i8)>{
        &self.tt
    }
}

#[cfg(test)]
mod tests {
    use tinyvec::ArrayVec;
    use yukari_movegen::{Board, Zobrist};

    use crate::Search;

    #[test]
    fn lasker_reichhelm() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("8/k7/3p4/p2P1p2/P2P1P2/8/8/K7 w - - 0 1", &zobrist).unwrap();

        let mut search = Search::new(None, &zobrist);
        let mut pv = ArrayVec::new();
        let mut keystack = Vec::new();
        for i in 1..=100 {
            keystack.clear();
            dbg!(search.search_root(&startpos, i, &mut pv, &mut keystack));
            eprintln!("PV [{}]: {}", i, &pv);
            eprintln!("TT stats [{}]: {}", i, &search.DONOTUSE_get_tt())
        }
    }
}
