use std::{cmp::Ordering, sync::{Arc, Weak, atomic::{self, AtomicBool, AtomicU64}}, time::Instant};

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
        board: &Board, tt_move: Option<Move>, history: &[[[i16; 64]; 64]; 12], m: Move,
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
        let score = i32::from(history[coloured_piece][m.from.into_inner() as usize][m.dest.into_inner() as usize]);
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
    history: [[[i16; 64]; 64]; 12]
}

impl Thread {
    pub fn quiesce(&mut self, mut alpha: i32, beta: i32, ply: usize) -> i32 {
        if self.pv.len() <= ply {
            self.pv.push(Vec::new());
        } else {
            self.pv[ply].clear();
        }

        self.seldepth = self.seldepth.max(ply);

        let mut best = self.board[ply].eval(self.board[ply].side());
        if best >= beta {
            return best;
        }
        alpha = alpha.max(best);

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

            let score = -self.quiesce(-beta, -alpha, ply + 1);

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
        &mut self, ply: usize, m: Move, bonus: i32,
    ) {
        const HISTORY_MAX: i32 = 16384;
        let board = &self.board[ply];
        let bonus = bonus.clamp(-HISTORY_MAX, HISTORY_MAX);
        {
            let coloured_piece = 6 * usize::from(board.side() == Colour::Black) + board.piece_from_square(m.from).unwrap() as usize;
            let history = &mut self.history[coloured_piece][m.from.into_inner() as usize][m.dest.into_inner() as usize];
            let bonus = bonus - i32::from(*history) * bonus.abs() / HISTORY_MAX;
            *history += bonus as i16;
        }
    }

    pub fn search(&mut self, depth: i32, mut alpha: i32, beta: i32, ply: usize, tt: &[TtEntry]) -> i32 {
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
            return self.quiesce(alpha, beta, ply);
        }

        let tt_entry = self.probe_tt(tt, &self.board[ply], ply);
        if let Some(entry) = tt_entry && !expected_pvnode && i32::from(entry.depth) >= depth {
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

        let eval = self.board[ply].eval(self.board[ply].side());
        let rfp_margin = 45 * depth;
        if !self.board[ply].in_check() && depth <= 8 && eval - rfp_margin >= beta {
            return eval - rfp_margin;
        }

        let razor_margin = 250 * depth;
        if !self.board[ply].in_check() && depth == 1 && alpha.abs() < 2000 && eval + razor_margin <= alpha {
            let score = self.quiesce(alpha, alpha + 1, ply);
            if score <= alpha {
                return score;
            }
        }

        let (try_probcut_beta, try_probcut_alpha, a, b, sigma, s) = match depth {
            1 => (true, true, 1.033_601_6, 5.614_562, 56.246_075, 0),
            2 => (true, false, 1.039_230_8, 8.608_924, 65.645_85, 0),
            3 => (true, false, 1.033_900_7, -1.201_043, 54.260_3, 1),
            4 => (true, false, 1.041_830_2, 1.444_724_8, 66.839_806, 1),
            //8 => (true, 1.0417771,   1.3685266,  94.86876, 4),
            _ => (false, false, 0.0, 0.0, 0.0, 0),
        };
        if !self.board[ply].in_check() && alpha >= -1000 && beta <= 1000 && try_probcut_beta {
            let bound = ((beta as f32 + sigma - b) / a).round() as i32;
            let score = self.search(s, bound - 1, bound, ply, tt);
            if score >= bound {
                return beta;
            }
        }

        if !self.board[ply].in_check() && alpha >= -1000 && beta <= 1000 && try_probcut_alpha {
            let bound = ((alpha as f32 - sigma - b) / a).round() as i32;
            let score = self.search(s, bound, bound + 1, ply, tt);
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

        let mut moves = {
            let tt_move = tt_entry.and_then(|e| e.m);
            moves
                .into_iter()
                .map(|m| (m, MoveOrder::classify(&self.board[ply], tt_move, &self.history, m)))
                .collect::<ArrayVec<[(Move, MoveOrder); 256]>>()
        };
        moves.sort_by_key(|(_, order)| *order);

        self.keystack.push(self.board[ply].hash());

        let mut best = i32::MIN;
        let mut best_move = None;
        let mut raised_alpha = false;

        for (movecount, (m, _)) in moves.iter().enumerate() {
            self.nodes += 1;

            self.prefetch_tt(tt, &self.board[ply], *m);

            if self.board.len() <= ply + 1 {
                self.board.push(self.board[ply].make(*m));
            } else {
                self.board[ply+1] = self.board[ply].make(*m);
            }

            let mut score;
            if movecount == 0 {
                score = -self.search(depth - 1, -beta, -alpha, ply + 1, tt);
            } else {
                score = -self.search(depth - 1, -alpha - 1, -alpha, ply + 1, tt);
                if score > alpha && score < beta {
                    score = -self.search(depth - 1, -beta, -alpha, ply + 1, tt);
                }
            }

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
                self.keystack.pop();
                return best;
            }

            if score > alpha {
                alpha = score;
                raised_alpha = true;
            }

            if score >= beta {
                let bonus = 250 * depth - 300;
                if !m.is_capture() {
                    for (m, _) in moves.into_iter().take(movecount) {
                        if m.is_capture() {
                            continue;
                        }
                        self.update_history(ply, m, -bonus);
                    }
                    self.update_history(ply, *m, bonus);
                }

                break;
            }
        }

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

        best
    }
}

pub struct Search {
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
        let scores = self.threads.par_iter_mut().map(|thread| {
            thread.search(depth, alpha, beta, 0, &self.tt)
        }).collect::<Vec<_>>();

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
