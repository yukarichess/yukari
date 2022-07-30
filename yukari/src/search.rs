use std::{time::Instant, cell::Cell, cmp::{Ordering, Reverse}};

use tinyvec::ArrayVec;
use yukari_movegen::{Board, Move, Zobrist, MoveType, Piece};

use crate::eval::EvalState;

const MATE_VALUE: i32 = 10_000;

#[derive(PartialEq, Eq, Debug)]
enum SortClass {
    CapturePromotion(Piece, Piece),
    Capture(Piece, Piece),
    Promotion(Piece),
    Quiet(u64),
}

impl PartialOrd for SortClass {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        /*
            - capture-promotions are sorted by most valuable victim, then by most valuable promotion piece
            - capture-promotions > captures if they have a more valuable victim
            - capture-promotions > promotions & quiet moves

            - captures are sorted by most valuable victim, then by least valuable attacker
            - captures > promotions & quiet moves

            - promotions are sorted by most valuable promotion piece
            - promotions > quiet moves

            - quiet moves are sorted by a quiet move heuristic
        */
        // (victim, capture_prom, attacker, prom, score)
        let to_partial = |class: &SortClass| {
            match class {
                SortClass::CapturePromotion(victim, prom) => (Some(*victim), Some(*prom), Some(Reverse(Piece::Pawn)), None, u64::MAX),
                SortClass::Capture(victim, attacker) => (Some(*victim), None, Some(Reverse(*attacker)), None, u64::MAX),
                SortClass::Promotion(piece) => (None, None, None, Some(*piece), u64::MAX),
                SortClass::Quiet(score) => (None, None, None, None, *score),
            }
        };

        to_partial(self).partial_cmp(&to_partial(other))
    }
}

impl Ord for SortClass {
    fn cmp(&self, other: &Self) -> Ordering {
        /*
            - capture-promotions are sorted by most valuable victim, then by most valuable promotion piece
            - capture-promotions > captures if they have a more valuable victim
            - capture-promotions > promotions & quiet moves

            - captures are sorted by most valuable victim, then by least valuable attacker
            - captures > promotions & quiet moves

            - promotions are sorted by most valuable promotion piece
            - promotions > quiet moves

            - quiet moves are sorted by a quiet move heuristic
        */
        // (victim, capture_prom, attacker, prom, score)
        let to_partial = |class: &SortClass| {
            match class {
                SortClass::CapturePromotion(victim, prom) => (Some(*victim), Some(*prom), Some(Reverse(Piece::Pawn)), None, u64::MAX),
                SortClass::Capture(victim, attacker) => (Some(*victim), None, Some(Reverse(*attacker)), None, u64::MAX),
                SortClass::Promotion(piece) => (None, None, None, Some(*piece), u64::MAX),
                SortClass::Quiet(score) => (None, None, None, None, *score),
            }
        };

        to_partial(self).cmp(&to_partial(other))
    }
}

struct History {
    fail_highs: [[Cell<u64>; 64]; 64],
    fail_lows: [[Cell<u64>; 64]; 64],
}

impl History {
    pub fn new() -> Self {
        #[allow(clippy::declare_interior_mutable_const)]
        const ZERO: Cell<u64> = Cell::new(1);
        #[allow(clippy::declare_interior_mutable_const)]
        const ZEROES: [Cell<u64>; 64] = [ZERO; 64];
        Self { fail_highs: [ZEROES; 64], fail_lows: [ZEROES; 64] }
    }

    pub fn fail_high(&self, m: Move, depth: i32) {
        let x = self.fail_highs[m.dest.into_inner() as usize][m.from.into_inner() as usize].get();
        self.fail_highs[m.dest.into_inner() as usize][m.from.into_inner() as usize].replace(x + (depth * depth) as u64);
    }

    pub fn fail_low(&self, m: Move) {
        let x = self.fail_lows[m.dest.into_inner() as usize][m.from.into_inner() as usize].get();
        self.fail_lows[m.dest.into_inner() as usize][m.from.into_inner() as usize].replace(x + 1);
    }

    pub fn classify(&self, b: &Board, m: Move) -> SortClass {
        let fail_highs = self.fail_highs[m.dest.into_inner() as usize][m.from.into_inner() as usize].get();
        let fail_lows = self.fail_lows[m.dest.into_inner() as usize][m.from.into_inner() as usize].get();
        let history = fail_highs / fail_lows;
        match m.kind {
            MoveType::Normal | MoveType::Castle | MoveType::DoublePush => SortClass::Quiet(history),
            MoveType::Capture => SortClass::Capture(b.piece_from_square(m.dest).unwrap(), b.piece_from_square(m.from).unwrap()),
            MoveType::EnPassant => SortClass::Capture(Piece::Pawn, Piece::Pawn),
            MoveType::Promotion => SortClass::Promotion(m.prom.unwrap()),
            MoveType::CapturePromotion => SortClass::CapturePromotion(b.piece_from_square(m.dest).unwrap(), m.prom.unwrap()),
        }
    }
}

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
    history: History,
}

impl<'a> Search<'a> {
    #[must_use]
    pub fn new(stop_after: Option<Instant>, zobrist: &'a Zobrist) -> Self {
        Self {
            nodes: 0,
            qnodes: 0,
            nullmove_attempts: 0,
            nullmove_success: 0,
            stop_after,
            zobrist,
            history: History::new(),
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

            let eval = eval.clone().update_eval(board, m);

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
        const R: i32 = 3;

        if depth <= 0 {
            pv.set_len(0);
            return self.quiesce(board, lower_bound, upper_bound, eval);
        }

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

        // Check extension
        if board.in_check() {
            depth += 1;
        }

        let mut found_pv = false;

        moves.sort_by_key(|a| Reverse(self.history.classify(board, *a)));
        //moves.reverse();

        let mut moves_searched = 0;

        for m in moves {
            self.nodes += 1;

            let mut child_pv = ArrayVec::new();
            let eval = eval.clone().update_eval(board, m);
            let board = board.make(m, self.zobrist);
            let mut score;

            // Push the move to check for repetition draws
            keystack.push(board.hash());
            if !found_pv {
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
                lower_bound = upper_bound;
                break;
            }

            if self.nodes & 1023 == 0 {
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
                found_pv = true;
            }

            moves_searched += 1;
        }

        if lower_bound == upper_bound && !moves[moves_searched].is_capture() {
            self.history.fail_high(moves[moves_searched], depth);
        }

        for m in 0..moves_searched {
            if !moves[m].is_capture() {
                self.history.fail_low(moves[m]);
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
    pub fn nodes(&self) -> u64 {
        self.nodes
    }

    #[must_use]
    pub fn qnodes(&self) -> u64 {
        self.qnodes
    }

    #[must_use]
    pub fn nullmove_success(&self) -> f64 {
        100.0 * (self.nullmove_success as f64) / (self.nullmove_attempts as f64)
    }
}
