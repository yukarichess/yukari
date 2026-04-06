use std::{cmp::Ordering, sync::{Arc, atomic::{self, AtomicBool, AtomicU64}}, time::Instant};

use rayon::iter::{IntoParallelRefMutIterator, ParallelIterator};
use tinyvec::ArrayVec;
use yukari_movegen::{Board, Colour, Move, Piece};

const MATE_VALUE: i32 = 10_000;

// TODO: when 50-move rule is implemented, this can be limited to searching from the last irreversible move.
#[must_use]
pub fn is_repetition_draw(keystack: &[u64], hash: u64) -> bool {
    keystack.iter().filter(|key| **key == hash).count() >= 3
}

#[derive(Copy, Clone, Default, PartialEq, Eq)]
#[repr(u8)]
enum TtFlags {
    #[default]
    Exact = 0,
    Upper = 1,
    Lower = 2,
}

#[derive(Default)]
#[repr(align(16))]
pub struct TtEntry {
    key: AtomicU64,
    data: AtomicU64,
}

#[derive(Default, Clone, Copy)]
struct TtData {
    flags: TtFlags,
    depth: u8,
    score: i16,
    m: Option<Move>,
}

const _TT_ENTRY_IS_16_BYTE: () = assert!(std::mem::size_of::<TtEntry>() == 16);
const _TT_DATA_IS_8_BYTE: () = assert!(std::mem::size_of::<TtData>() == 8);

#[derive(PartialEq, Eq, Copy, Clone, Debug, Default)]
enum MoveOrder {
    #[default]
    TtMove,
    GoodCapture(Piece, Piece),
    Quiet(i32),
    BadCapture(Piece, Piece),
}

impl PartialOrd for MoveOrder {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MoveOrder {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            // TT Move sorts above all others
            (MoveOrder::TtMove, MoveOrder::TtMove) => Ordering::Equal, // shouldn't happen?
            (MoveOrder::TtMove, _) => Ordering::Less,
            (_, MoveOrder::TtMove) => Ordering::Greater,

            // Good captures sort above quiets and bad captures; ties broken by highest MVV/LVA score.
            (MoveOrder::GoodCapture(a_mvv, a_lva), MoveOrder::GoodCapture(b_mvv, b_lva)) => {
                b_mvv.cmp(a_mvv).then_with(|| a_lva.cmp(b_lva))
            }
            (MoveOrder::GoodCapture(_, _), _) => Ordering::Less,
            (_, MoveOrder::GoodCapture(_, _)) => Ordering::Greater,

            // Quiets sort above bad captures; ties broken by highest history score.
            (MoveOrder::Quiet(a), MoveOrder::Quiet(b)) => b.cmp(a),
            (MoveOrder::Quiet(_), _) => Ordering::Less,
            (_, MoveOrder::Quiet(_)) => Ordering::Greater,

            // Bad captures; ties broken by highest MVV/LVA score.
            (MoveOrder::BadCapture(a_mvv, a_lva), MoveOrder::BadCapture(b_mvv, b_lva)) => {
                b_mvv.cmp(a_mvv).then_with(|| a_lva.cmp(b_lva))
            }
        }
    }
}

impl MoveOrder {
    pub fn classify(
        board: &Board, tt_move: Option<Move>, history: &[[[i16; 64]; 64]; 12], conthist: &[[i16; 2 * 6 * 64]; 2 * 6 * 64], last_last_m: Option<(Piece, Move)>, last_m: Option<(Piece, Move)>, m: Move,
    ) -> Self {
        if let Some(tt_move) = tt_move && tt_move == m {
            return Self::TtMove;
        }

        if m.is_capture() {
            let dest_piece = board.piece_from_square(m.dest).unwrap_or(Piece::Pawn);
            let from_piece = board.piece_from_square(m.from).unwrap();
            if (dest_piece >= from_piece) || board.static_exchange_evaluation(m) >= 0 {
                return Self::GoodCapture(dest_piece, from_piece);
            }
            return Self::BadCapture(dest_piece, from_piece);
        }

        let coloured_piece = 6 * usize::from(board.side() == Colour::Black) + board.piece_from_square(m.from).unwrap() as usize;
        let mut score = i32::from(history[coloured_piece][m.from.into_inner() as usize][m.dest.into_inner() as usize]);
        if let Some((last_piece, last_m)) = last_last_m {
            let last_index = 6 * 64 * usize::from(board.side() == Colour::Black)
                + 64 * (last_piece as usize)
                + usize::from(last_m.dest.into_inner());
            let curr_index = 6 * 64 * usize::from(board.side() == Colour::Black)
                + 64 * (board.piece_from_square(m.from).unwrap() as usize)
                + usize::from(m.dest.into_inner());
            score += i32::from(conthist[last_index][curr_index]);
        }
        if let Some((last_piece, last_m)) = last_m {
            let last_index = 6 * 64 * usize::from(board.side() == Colour::Black)
                + 64 * (last_piece as usize)
                + usize::from(last_m.dest.into_inner());
            let curr_index = 6 * 64 * usize::from(board.side() == Colour::Black)
                + 64 * (board.piece_from_square(m.from).unwrap() as usize)
                + usize::from(m.dest.into_inner());
            score += i32::from(conthist[last_index][curr_index]);
        }
        Self::Quiet(score)
    }
}

#[derive(Clone)]
#[repr(align(64))]
struct Thread {
    nodes: u64,
    qnodes: u64,
    seldepth: usize,
    stop: Arc<AtomicBool>,
    stop_after: Option<Instant>,
    node_limit: Option<u64>,
    board: Vec<Board>,
    pv: Vec<Vec<Move>>,
    keystack: Vec<u64>,
    index: usize,
    history: [[[i16; 64]; 64]; 12],
    conthist: [[i16; 2 * 6 * 64]; 2 * 6 * 64],
    corrhist_p: [[i32; 16384]; 2],
    path: Vec<Option<(Piece, Move)>>,
}

impl Thread {
    fn eval(&self, ply: usize) -> i32 {
        const CORRHIST_GRAIN: i32 = 256;

        let eval = self.board[ply].eval(self.board[ply].side());

        let entry_p = self.corrhist_p[self.board[ply].side() as usize][self.board[ply].hash_pawns() as usize & 16383];
        let corrhist = entry_p / CORRHIST_GRAIN;
        (eval + corrhist).clamp(-MATE_VALUE + 1, MATE_VALUE - 1)
    }

    pub fn quiesce(&mut self, mut alpha: i32, beta: i32, ply: usize, tt: &[TtEntry]) -> i32 {
        let expected_pvnode = alpha != beta - 1;

        if self.pv.len() <= ply {
            self.pv.push(Vec::new());
        } else {
            self.pv[ply].clear();
        }

        self.seldepth = self.seldepth.max(ply);

        let mut best = self.eval(ply);
        if best >= beta {
            return best;
        }
        alpha = alpha.max(best);

        let tt_entry = self.probe_tt(tt, &self.board[ply], ply);
        if let Some(entry) = tt_entry && !expected_pvnode {
            let score = i32::from(entry.score);
            match entry.flags {
                TtFlags::Exact => {
                    return score;
                }
                TtFlags::Upper => {
                    if score <= alpha {
                        return score;
                    }
                }
                TtFlags::Lower => {
                    if score >= beta {
                        return score;
                    }
                }
            }
        }

        let mut moves = ArrayVec::new();
        self.board[ply].generate_quiesce(&mut moves);

        for m in &moves {
            if self.board[ply].static_exchange_evaluation(*m) < 0 {
                continue;
            }

            self.qnodes += 1;

            if self.board.len() <= ply + 1 {
                self.board.push(self.board[ply].make(*m));
            } else {
                self.board[ply+1] = self.board[ply].make(*m);
            }

            let score = -self.quiesce(-beta, -alpha, ply + 1, tt);

            if score > best {
                best = score;

                self.pv[ply].clear();
                self.pv[ply].push(*m);
                let (this_pv, next_pv) = self.pv.split_at_mut(ply+1);
                let (this_pv, next_pv) = (this_pv.last_mut().unwrap(), next_pv.first().unwrap());
                this_pv.extend_from_slice(next_pv);
            }

            if score > alpha {
                alpha = score;
            }

            if score >= beta {
                return score;
            }
        }

        best
    }

    fn probe_tt(&self, tt: &[TtEntry], board: &Board, ply: usize) -> Option<TtData> {
        let entry = (board.hash() & ((tt.len() - 1) as u64)) as usize;
        let entry = &tt[entry];
        let entry_key = entry.key.load(atomic::Ordering::Acquire);
        let entry_data = entry.data.load(atomic::Ordering::Acquire);
        let mut entry: TtData = unsafe { std::mem::transmute(entry_data) };

        if entry_key ^ entry_data == board.hash() {
            if i32::from(entry.score) >= MATE_VALUE - 500 {
                entry.score -= ply as i16;
            }
            if i32::from(entry.score) <= -MATE_VALUE + 500 {
                entry.score += ply as i16;
            }
            return Some(entry);
        }
        None
    }

    #[cfg(target_arch = "aarch64")]
    #[inline(always)]
    fn prefetch_tt(&self, tt: &[TtEntry], board: &Board, m: Move) {
        use core::arch::aarch64::{_prefetch, _PREFETCH_READ, _PREFETCH_LOCALITY3};

        let entry = (board.hash_after(m) & ((tt.len() - 1) as u64)) as usize;
        let entry = &tt[entry];
        unsafe { _prefetch(entry as *const _ as *const i8, _PREFETCH_READ, _PREFETCH_LOCALITY3) }
    }

    #[cfg(target_arch = "x86_64")]
    #[inline(always)]
    fn prefetch_tt(&self, tt: &[TtEntry], board: &Board, m: Move) {
        use core::arch::x86_64::{_mm_prefetch, _MM_HINT_T0};

        let entry = (board.hash_after(m) & ((tt.len() - 1) as u64)) as usize;
        let entry = &tt[entry];
        unsafe { _mm_prefetch::<_MM_HINT_T0>(entry as *const _ as *const i8) }
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    #[inline(always)]
    fn prefetch_tt(&self, tt: &[TtEntry], board: &Board, m: Move) {}

    fn write_tt(&self, tt: &[TtEntry], board: &Board, ply: usize, mut data: TtData) {
        let entry = (board.hash() & ((tt.len() - 1) as u64)) as usize;
        let entry = &tt[entry];
        if i32::from(data.score) >= MATE_VALUE - 500 {
            data.score += ply as i16;
        }
        if i32::from(data.score) <= -MATE_VALUE + 500 {
            data.score -= ply as i16;
        }
        let data = unsafe { std::mem::transmute::<TtData, u64>(data) };
        entry.key.store(board.hash() ^ data, atomic::Ordering::Release);
        entry.data.store(data, atomic::Ordering::Release);
    }

    fn update_history(
        &mut self, ply: usize, last_last_m: Option<(Piece, Move)>, last_m: Option<(Piece, Move)>, m: Move, bonus: i32,
    ) {
        const HISTORY_MAX: i32 = 16384;
        let board = &self.board[ply];
        let bonus = bonus.clamp(-HISTORY_MAX, HISTORY_MAX);
        // History Heuristic
        {
            let coloured_piece = 6 * usize::from(board.side() == Colour::Black) + board.piece_from_square(m.from).unwrap() as usize;
            let history = &mut self.history[coloured_piece][m.from.into_inner() as usize][m.dest.into_inner() as usize];
            let bonus = bonus - i32::from(*history) * bonus.abs() / HISTORY_MAX;
            *history += bonus as i16;
        }

        // N-2 Continuation History (Follow Up History)
        if let Some((last_piece, last_m)) = last_last_m {
            let last_index = 6 * 64 * usize::from(board.side() == Colour::Black)
                + 64 * (last_piece as usize)
                + usize::from(last_m.dest.into_inner());
            let curr_index = 6 * 64 * usize::from(board.side() == Colour::Black)
                + 64 * (board.piece_from_square(m.from).unwrap() as usize)
                + usize::from(m.dest.into_inner());
            let conthist = &mut self.conthist[last_index][curr_index];
            let bonus = bonus - i32::from(*conthist) * bonus.abs() / HISTORY_MAX;
            *conthist += bonus as i16;
        }

        // N-1 Continuation History (Counter Move History)
        if let Some((last_piece, last_m)) = last_m {
            let last_index = 6 * 64 * usize::from(board.side() == Colour::Black)
                + 64 * (last_piece as usize)
                + usize::from(last_m.dest.into_inner());
            let curr_index = 6 * 64 * usize::from(board.side() == Colour::Black)
                + 64 * (board.piece_from_square(m.from).unwrap() as usize)
                + usize::from(m.dest.into_inner());
            let conthist = &mut self.conthist[last_index][curr_index];
            let bonus = bonus - i32::from(*conthist) * bonus.abs() / HISTORY_MAX;
            *conthist += bonus as i16;
        }
    }

    fn update_corrhist(&mut self, ply: usize, depth: i32, diff: i32) {
        const CORRHIST_GRAIN: i32 = 256;
        const CORRHIST_WEIGHT_SCALE: i32 = 256;
        const CORRHIST_MAX: i32 = 256 * 32;

        let diff = diff * CORRHIST_GRAIN;
        let weight = 16.min(depth + 1);

        // pawns
        let entry_p = &mut self.corrhist_p[self.board[ply].side() as usize][self.board[ply].data().hash_pawns() as usize & 16383];

        *entry_p = ((*entry_p * (CORRHIST_WEIGHT_SCALE - weight) + diff * weight) / CORRHIST_WEIGHT_SCALE)
            .clamp(-CORRHIST_MAX, CORRHIST_MAX);
    }

    pub fn search(&mut self, depth: i32, mut alpha: i32, beta: i32, ply: usize, tt: &[TtEntry], excluded_move: Option<Move>) -> i32 {
        let expected_pvnode = alpha != beta - 1;

        if self.pv.len() <= ply {
            self.pv.push(Vec::new());
        } else {
            self.pv[ply].clear();
        }

        self.seldepth = self.seldepth.max(ply);

        // Draw by insufficient material
        if self.board[ply].insufficient_material() && ply > 0 {
            return 0;
        }

        // Is this a repetition draw?
        if is_repetition_draw(&self.keystack, self.board[ply].hash()) && ply > 0 {
            return 0;
        }

        if depth <= 0 {
            return self.quiesce(alpha, beta, ply, tt);
        }

        let tt_entry = self.probe_tt(tt, &self.board[ply], ply);
        if let Some(entry) = tt_entry && excluded_move.is_none() && !expected_pvnode && i32::from(entry.depth) >= depth {
            let score = i32::from(entry.score);
            match entry.flags {
                TtFlags::Exact => {
                    return score;
                }
                TtFlags::Upper => {
                    if score <= alpha {
                        return score;
                    }
                }
                TtFlags::Lower => {
                    if score >= beta {
                        return score;
                    }
                }
            }
        }

        let eval = self.eval(ply);
        let rfp_margin = 45 * depth;
        if excluded_move.is_none() && !self.board[ply].in_check() && depth <= 8 && eval - rfp_margin >= beta {
            return eval - rfp_margin;
        }

        let razor_margin = 250 * depth;
        if excluded_move.is_none() && !self.board[ply].in_check() && depth == 1 && alpha.abs() < 2000 && eval + razor_margin <= alpha {
            let score = self.quiesce(alpha, alpha + 1, ply, tt);
            if score <= alpha {
                return score;
            }
        }

        let (try_probcut_beta, try_probcut_alpha, a, b, sigma, s) = match depth {
            1 => (true, true, 1.034_839_3, 4.134_490_5, 55.742_91, 0), // R² = 0.971229
            2 => (true, false, 1.038_158, 6.550_359_2, 63.202_9, 0), // R² = 0.963539
            3 => (true, false, 1.067_644_7, 1.667_016_3, 75.316_055, 0), // R² = 0.951649
            4 => (true, false, 1.033_658_9, 0.810_709_7, 63.553_913, 1), // R² = 0.966171
            5 => (true, false, 1.059_259, -3.001_881, 68.158_165, 1), // R² = 0.963068
            _ => (false, false, 0.0, 0.0, 0.0, 0),
        };
        if excluded_move.is_none() && !self.board[ply].in_check() && alpha >= -1000 && beta <= 1000 && !expected_pvnode && try_probcut_beta {
            let bound = ((beta as f32 + sigma - b) / a).round() as i32;
            let score = self.search(s, bound - 1, bound, ply, tt, None);
            if score >= bound {
                return beta;
            }
        }

        if excluded_move.is_none() && !self.board[ply].in_check() && alpha >= -1000 && beta <= 1000 && !expected_pvnode && try_probcut_alpha {
            let bound = ((alpha as f32 - sigma - b) / a).round() as i32;
            let score = self.search(s, bound, bound + 1, ply, tt, None);
            if score <= bound {
                return alpha;
            }
        }

        let mut moves = ArrayVec::new();
        self.board[ply].generate(&mut moves);

        // Is this checkmate or stalemate?
        if moves.is_empty() {
            if self.board[ply].in_check() {
                return -MATE_VALUE + (ply as i32);
            }
            return 0;
        }

        // Is this a singular search where we have excluded the only legal move?
        if moves.len() == 1 && excluded_move.is_some() {
            return alpha;
        }

        let mut moves = {
            let tt_move = tt_entry.and_then(|e| e.m);
            let last_m = *self.path.last().unwrap_or(&None);
            let last_last_m = *self.path.iter().rev().nth(1).unwrap_or(&None);
            moves
                .into_iter()
                .map(|m| (m, MoveOrder::classify(&self.board[ply], tt_move, &self.history, &self.conthist, last_last_m, last_m, m)))
                .collect::<ArrayVec<[(Move, MoveOrder); 256]>>()
        };
        moves.sort_by_key(|(_, order)| *order);

        if excluded_move.is_none() {
            self.keystack.push(self.board[ply].hash());
        }

        let mut best = i32::MIN;
        let mut best_move = None;
        let mut raised_alpha = false;

        for (movecount, (m, _)) in moves.iter().enumerate() {
            if Some(*m) == excluded_move {
                continue;
            }

            self.prefetch_tt(tt, &self.board[ply], *m);

            // SEE Pruning
            if !self.board[ply].in_check() && depth <= 2 && movecount > 1 && best > -MATE_VALUE + 500 {
                if !m.is_capture() {
                    let threshold = -(depth as f32 * 0.0) as i32;
                    if self.board[ply].static_exchange_evaluation(*m) < threshold {
                        continue;
                    }
                }
            }

            let mut extension = 0;

            // Singular extension: is the TT move uniquely good?
            if let Some(tt_entry) = tt_entry
                && excluded_move.is_none()
                && ply > 0
                && Some(*m) == tt_entry.m
            {
                if depth >= 7 && matches!(tt_entry.flags, TtFlags::Exact | TtFlags::Lower) && tt_entry.score.abs() < 9500 {
                    let singular_beta = (i32::from(tt_entry.score) - depth * 2).max(-MATE_VALUE + 1);
                    let singular_depth = (depth - 1) / 2;
                    let score = self.search(singular_depth, singular_beta - 1, singular_beta, ply, tt, Some(*m));

                    // Multicut: Another move failed high, so this position is very good; prune.
                    if score >= singular_beta && singular_beta >= beta {
                        self.keystack.pop();
                        return singular_beta;
                    }

                    // The TT move seems uniquely good; extend.
                    if score < singular_beta {
                        extension += 1;
                    } else if tt_entry.score as i32 >= beta {
                        extension -= 1;
                    }
                // Low depth singular extension: Determine singularity by static eval vs alpha.
                } else if !self.board[ply].in_check() && depth <= 7 && eval <= alpha - 25 && tt_entry.flags == TtFlags::Lower {
                    extension += 1;
                }
            }

            self.nodes += 1;

            self.path.push(Some((self.board[ply].piece_from_square(m.from).unwrap(), *m)));

            if self.board.len() <= ply + 1 {
                self.board.push(self.board[ply].make(*m));
            } else {
                self.board[ply+1] = self.board[ply].make(*m);
            }

            let mut score;
            if movecount == 0 {
                score = -self.search(depth - 1 + extension, -beta, -alpha, ply + 1, tt, None);
            } else {
                // Late Move Reduction
                let mut reduction = 0;
                if depth >= 3 && movecount >= 4 && !m.is_capture() {
                    let depth = (depth as f32).ln();
                    let movecount = (movecount as f32).ln();
                    reduction += (depth * movecount).mul_add(0.5, 1.0) as i32;
                    reduction -= i32::from(expected_pvnode);
                    // credit: adam
                }

                score = -self.search(depth - 1 - reduction + extension, -alpha - 1, -alpha, ply + 1, tt, None);
                if score > alpha && score < beta {
                    score = -self.search(depth - 1 + extension, -beta, -alpha, ply + 1, tt, None);
                }
            }

            self.path.pop();

            if score > best {
                best = score;
                best_move = Some(*m);

                self.pv[ply].clear();
                self.pv[ply].push(*m);
                let (this_pv, next_pv) = self.pv.split_at_mut(ply+1);
                let (this_pv, next_pv) = (this_pv.last_mut().unwrap(), next_pv.first().unwrap());
                this_pv.extend_from_slice(next_pv);
            }

            if self.index == 0 {
                if let Some(node_limit) = self.node_limit && self.nodes + self.qnodes >= node_limit {
                    self.stop.store(true, atomic::Ordering::Release);
                }

                if self.nodes.trailing_zeros() >= 10 && let Some(time) = self.stop_after && Instant::now() >= time {
                    self.stop.store(true, atomic::Ordering::Release);
                }
            }

            if self.stop.load(atomic::Ordering::Acquire) {
                if excluded_move.is_none() {
                    self.keystack.pop();
                }
                return best;
            }

            if score > alpha {
                alpha = score;
                raised_alpha = true;
            }

            if score >= beta {
                let bonus = 250 * depth - 300;
                let last_m = *self.path.last().unwrap_or(&None);
                let last_last_m = *self.path.iter().rev().nth(1).unwrap_or(&None);
                if !m.is_capture() {
                    for (m, _) in moves.into_iter().take(movecount) {
                        if m.is_capture() {
                            continue;
                        }
                        self.update_history(ply, last_last_m, last_m, m, -bonus);
                    }
                    self.update_history(ply, last_last_m, last_m, *m, bonus);
                }

                break;
            }
        }

        if excluded_move.is_none() {
            self.keystack.pop();

            self.write_tt(
                tt,
                &self.board[ply],
                ply,
                TtData {
                    m: best_move,
                    score: best as i16,
                    flags: if best >= beta {
                        TtFlags::Lower
                    } else if raised_alpha {
                        TtFlags::Exact
                    } else {
                        TtFlags::Upper
                    },
                    depth: depth as u8,
                },
            );

            if !self.board[ply].in_check()
                && !best_move.unwrap().is_capture()
                && (raised_alpha
                    || (best >= beta && best >= eval)
                    || (best <= alpha && best <= eval))
            {
                self.update_corrhist(ply, depth, best - eval);
            }
        }

        best
    }
}

pub struct Search {
    pool: rayon::ThreadPool,
    threads: Vec<Thread>,
    tt: Vec<TtEntry>,
    stop: Arc<AtomicBool>,
    stop_after: Option<Instant>,
}

impl Search {
    #[must_use]
    pub fn new(
        threads: usize,
    ) -> Self {
        let mut this = Self {
            pool: rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap(),
            threads: vec![],
            tt: vec![],
            stop: Arc::new(AtomicBool::new(false)),
            stop_after: None,
        };
        this.threads = vec![Thread {
                nodes: 0,
                qnodes: 0,
                seldepth: 0,
                board: vec![],
                pv: vec![],
                keystack: vec![],
                stop: Arc::clone(&this.stop),
                stop_after: None,
                node_limit: None,
                index: 0,
                history: [[[0; _]; _]; _],
                conthist: [[0; _]; _],
                corrhist_p: [[0; _]; _],
                path: vec![],
            }; threads];
        this
    }

    pub fn prepare(&mut self, board: &Board, stop_after: Option<Instant>, node_limit: Option<u64>, keystack: &[u64]) {
        self.stop_after = stop_after;
        self.stop.store(false, atomic::Ordering::Release);
        for (index, thread) in self.threads.iter_mut().enumerate() {
            thread.nodes = 0;
            thread.qnodes = 0;
            thread.seldepth = 0;
            thread.stop_after = stop_after;
            thread.node_limit = node_limit;
            thread.board = vec![board.clone()];
            thread.pv.clear();
            thread.keystack = keystack.to_vec();
            thread.index = index;
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn search(
        &mut self, depth: i32, alpha: i32, beta: i32, pv: &mut Vec<Move>,
    ) -> i32 {
        let scores = self.pool.install(|| {
            self.threads.par_iter_mut().map(|thread| {
                thread.search(depth, alpha, beta, 0, &self.tt, None)
            }).collect::<Vec<_>>()
        });

        *pv = self.threads[0].pv[0].clone();
        scores[0]
    }

    pub fn allocate_tt(&mut self, megabytes: usize) {
        let target_bytes = megabytes * 1024 * 1024;

        let mut size = 1_usize;
        loop {
            if size > target_bytes {
                break;
            }
            size *= 2;
        }
        size /= 2;
        size /= std::mem::size_of::<TtEntry>();

        self.tt = Vec::new();
        self.tt.resize_with(size, Default::default);
    }

    #[must_use]
    pub fn nodes(&self) -> u64 {
        self.threads.iter().map(|thread| thread.nodes).sum()
    }

    #[must_use]
    pub fn qnodes(&self) -> u64 {
        self.threads.iter().map(|thread| thread.qnodes).sum()
    }

    #[must_use]
    pub fn seldepth(&self) -> usize {
        self.threads.iter().map(|thread| thread.seldepth).max().unwrap()
    }
}
