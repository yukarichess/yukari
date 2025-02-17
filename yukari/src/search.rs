use std::{
    cmp::Ordering,
    sync::atomic::AtomicU64,
    time::{Duration, Instant},
};

use tinyvec::ArrayVec;
use yukari_movegen::{Board, Move, Piece};

use crate::output;

const MATE_VALUE: i32 = 10_000;

#[derive(Clone)]
pub struct SearchParams {
    pub rfp_margin_base: i32,
    pub rfp_margin_mul: i32,
    pub razor_margin_mul: i32,
    pub lmr_base: f32,
    pub lmr_mul: f32,
    pub hist_bonus_base: i32,
    pub hist_bonus_mul: i32,
    pub hist_pen_base: i32,
    pub hist_pen_mul: i32,
}

impl Default for SearchParams {
    fn default() -> Self {
        Self {
            rfp_margin_base: 0,
            rfp_margin_mul: 37,
            razor_margin_mul: 250,
            lmr_base: 1.0,
            lmr_mul: 0.5,
            hist_bonus_base: 250,
            hist_bonus_mul: 300,
            hist_pen_base: 250,
            hist_pen_mul: 300,
        }
    }
}

// TODO: when 50-move rule is implemented, this can be limited to searching from the last irreversible move.
#[must_use]
pub fn is_repetition_draw(keystack: &[u64], hash: u64) -> bool {
    keystack.iter().filter(|key| **key == hash).count() >= 3
}

#[derive(Copy, Clone, Default)]
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

pub fn allocate_tt(megabytes: usize) -> Vec<TtEntry> {
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

    let mut tt: Vec<TtEntry> = Vec::new();
    tt.resize_with(size, Default::default);
    println!("# Allocated {} bytes of hash", size * std::mem::size_of::<TtEntry>());
    tt
}

#[derive(PartialEq, Eq, Copy, Clone, Debug, Default)]
enum MoveOrder {
    #[default]
    TtMove,
    GoodCapture(Piece, Piece),
    Quiet(i16),
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
            (MoveOrder::GoodCapture(a_mvv, a_lva), MoveOrder::GoodCapture(b_mvv, b_lva)) => b_mvv.cmp(a_mvv).then_with(|| a_lva.cmp(b_lva)),
            (MoveOrder::GoodCapture(_, _), _) => Ordering::Less,
            (_, MoveOrder::GoodCapture(_, _)) => Ordering::Greater,

            // Quiets sort above bad captures; ties broken by highest history score.
            (MoveOrder::Quiet(a), MoveOrder::Quiet(b)) => b.cmp(a),
            (MoveOrder::Quiet(_), _) => Ordering::Less,
            (_, MoveOrder::Quiet(_)) => Ordering::Greater,

            // Bad captures; ties broken by highest MVV/LVA score.
            (MoveOrder::BadCapture(a_mvv, a_lva), MoveOrder::BadCapture(b_mvv, b_lva)) => b_mvv.cmp(a_mvv).then_with(|| a_lva.cmp(b_lva)),
        }
    }
}

impl MoveOrder {
    pub fn classify(board: &Board, history: &[[i16; 64]; 64], tt_move: Option<Move>, m: Move) -> Self {
        if let Some(tt_move) = tt_move {
            if tt_move == m {
                return Self::TtMove;
            }
        }

        if m.is_capture() {
            let dest_piece = board.piece_from_square(m.dest).unwrap_or(Piece::Pawn);
            let from_piece = board.piece_from_square(m.from).unwrap();
            if (dest_piece >= from_piece) || board.static_exchange_evaluation(m) >= 0 {
                return Self::GoodCapture(dest_piece, from_piece);
            } else {
                return Self::BadCapture(dest_piece, from_piece);
            }
        }

        let score = history[m.from.into_inner() as usize][m.dest.into_inner() as usize];
        Self::Quiet(score)
    }
}

pub struct Search<'a> {
    nodes: u64,
    qnodes: u64,
    zw_nodes: u64,
    zw_qnodes: u64,
    nullmove_attempts: u64,
    nullmove_success: u64,
    beta_cutoff_index: u64,
    beta_cutoffs: u64,
    q_beta_cutoff_index: u64,
    q_beta_cutoffs: u64,
    start: Instant,
    stop_after: Option<Instant>,
    history: &'a mut [[i16; 64]; 64],
    tt: &'a [TtEntry],
    corrhist: &'a mut [[i32; 16384]; 2],
    params: &'a SearchParams,
}

impl<'a> Search<'a> {
    #[must_use]
    pub fn new(
        start: Instant, stop_after: Option<Instant>, tt: &'a [TtEntry], history: &'a mut [[i16; 64]; 64],
        corrhist: &'a mut [[i32; 16384]; 2], params: &'a SearchParams,
    ) -> Self {
        Self {
            nodes: 0,
            qnodes: 0,
            zw_nodes: 0,
            zw_qnodes: 0,
            nullmove_attempts: 0,
            nullmove_success: 0,
            beta_cutoff_index: 0,
            beta_cutoffs: 0,
            q_beta_cutoff_index: 0,
            q_beta_cutoffs: 0,
            start,
            stop_after,
            history,
            tt,
            corrhist,
            params,
        }
    }

    fn update_corrhist(&mut self, board: &Board, depth: i32, diff: i32) {
        const CORRHIST_GRAIN: i32 = 256;
        const CORRHIST_WEIGHT_SCALE: i32 = 256;
        const CORRHIST_MAX: i32 = 256 * 32;
        let entry = &mut self.corrhist[board.side() as usize][board.hash_pawns() as usize & 16383];
        let diff = diff * CORRHIST_GRAIN;
        let weight = 16.min(depth + 1);

        *entry = ((*entry * (CORRHIST_WEIGHT_SCALE - weight) + diff * weight) / CORRHIST_WEIGHT_SCALE)
            .clamp(-CORRHIST_MAX, CORRHIST_MAX);
    }

    fn eval_with_corrhist(&self, board: &Board, eval: i32) -> i32 {
        const CORRHIST_GRAIN: i32 = 256;
        let entry = &self.corrhist[board.side() as usize][board.hash_pawns() as usize & 16383];
        (eval + entry / CORRHIST_GRAIN).clamp(-MATE_VALUE + 1, MATE_VALUE - 1)
    }

    fn update_history(&mut self, m: Move, bonus: i32) {
        const HISTORY_MAX: i32 = 16384;
        let bonus = bonus.clamp(-HISTORY_MAX, HISTORY_MAX);
        let history = &mut self.history[m.from.into_inner() as usize][m.dest.into_inner() as usize];
        let bonus = bonus - (*history as i32) * bonus.abs() / HISTORY_MAX;
        *history += bonus as i16;
    }

    fn quiesce(&mut self, board: &Board, mut alpha: i32, beta: i32, pv: &mut ArrayVec<[Move; 64]>, ply: i32) -> i32 {
        let mut best_score = self.eval_with_corrhist(board, board.eval(board.side()));

        pv.set_len(0);

        // Emergency bailout
        if ply == 63 {
            return best_score;
        }

        if best_score >= beta {
            return best_score;
        }
        alpha = alpha.max(best_score);

        if let Some(entry) = self.probe_tt(board, 0) {
            if alpha == beta - 1 {
                let score = entry.score as i32;
                match entry.flags {
                    TtFlags::Exact => return score,
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
        }

        let mut index = 0;
        board.generate_captures_incremental(|m| {
            self.qnodes += 1;
            if alpha == beta - 1 {
                self.zw_qnodes += 1;
            }

            let board = board.make(m);
            let mut child_pv = ArrayVec::new();
            let score = -self.quiesce(&board, -beta, -alpha, &mut child_pv, ply + 1);

            best_score = best_score.max(score);

            if score >= beta {
                self.q_beta_cutoff_index += index;
                self.q_beta_cutoffs += 1;
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

            index += 1;

            true
        });

        best_score
    }

    fn probe_tt(&self, board: &Board, ply: i32) -> Option<TtData> {
        let entry = (board.hash() & ((self.tt.len() - 1) as u64)) as usize;
        let entry = &self.tt[entry];
        let entry_key = entry.key.load(std::sync::atomic::Ordering::Relaxed);
        let entry_data = entry.data.load(std::sync::atomic::Ordering::Relaxed);
        let mut entry: TtData = unsafe { std::mem::transmute(entry_data) };

        if entry_key ^ entry_data == board.hash() {
            if entry.score as i32 >= MATE_VALUE - 500 {
                entry.score -= ply as i16;
            }
            if entry.score as i32 <= -MATE_VALUE + 500 {
                entry.score += ply as i16;
            }
            return Some(entry);
        }
        None
    }

    fn write_tt(&self, board: &Board, ply: i32, mut data: TtData) {
        let entry = (board.hash() & ((self.tt.len() - 1) as u64)) as usize;
        let entry = &self.tt[entry];
        if i32::from(data.score) >= MATE_VALUE - 500 {
            data.score += ply as i16;
        }
        if i32::from(data.score) <= -MATE_VALUE + 500 {
            data.score -= ply as i16;
        }
        let data = unsafe { std::mem::transmute::<TtData, u64>(data) };
        entry.key.store(board.hash() ^ data, std::sync::atomic::Ordering::Relaxed);
        entry.data.store(data, std::sync::atomic::Ordering::Relaxed);
    }

    #[allow(clippy::too_many_arguments)]
    fn search(
        &mut self, board: &Board, mut depth: i32, mut alpha: i32, beta: i32, output: &mut dyn output::Output,
        pv: &mut ArrayVec<[Move; 64]>, ply: i32, keystack: &mut Vec<u64>,
    ) -> i32 {
        // Emergency bailout
        if ply == 63 {
            pv.set_len(0);
            return self.eval_with_corrhist(board, board.eval(board.side()));
        }

        // Draw by insufficient material
        if board.insufficient_material() && ply > 0 {
            pv.set_len(0);
            return 0;
        }

        // Is this a repetition draw?
        if is_repetition_draw(keystack, board.hash()) && ply > 0 {
            pv.set_len(0);
            return 0;
        }

        let mut root_reduction = 0;

        // Check extension
        if board.in_check() {
            depth += 1;
            root_reduction += -1;
        }

        if depth <= 0 {
            return self.quiesce(board, alpha, beta, pv, ply);
        }

        pv.set_len(0);

        let tt_entry = self.probe_tt(board, ply);
        if let Some(entry) = tt_entry {
            if alpha == beta - 1 && entry.depth as i32 >= depth {
                let score = entry.score as i32;
                match entry.flags {
                    TtFlags::Exact => return score,
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
        }
        
        if alpha != beta - 1 && tt_entry.is_none() && depth >= 3 {
            // internal iterative reduction
            depth -= 1;
            root_reduction += 1;
        }

        let eval_int = self.eval_with_corrhist(board, board.eval(board.side()));

        let rfp_margin = self.params.rfp_margin_base + self.params.rfp_margin_mul * depth;
        if !board.in_check() && depth <= 4 && eval_int - rfp_margin >= beta {
            return eval_int - rfp_margin;
        }

        let razor_margin = self.params.razor_margin_mul * depth;
        if !board.in_check() && depth <= 3 && alpha.abs() < 2000 && eval_int + razor_margin <= alpha {
            let score = self.quiesce(board, alpha, alpha + 1, pv, ply);
            if score <= alpha {
                return score;
            }
        }

        let reduction = if depth > 6 { 4 } else { 3 } + ((eval_int - beta) / 200).max(0);

        if !board.in_check() && depth >= 2 && eval_int >= beta {
            keystack.push(board.hash());
            let board = board.make_null();
            let mut child_pv = ArrayVec::new();
            let score = -self.search(
                &board,
                depth - 1 - reduction,
                -beta,
                -beta + 1,
                output,
                &mut child_pv,
                ply + 1,
                keystack,
            );
            keystack.pop();

            self.nullmove_attempts += 1;

            if score >= beta {
                self.nullmove_success += 1;
                return score;
            }
        }

        let mut moves = ArrayVec::new();
        board.generate(&mut moves);

        // Is this checkmate or stalemate?
        if moves.is_empty() {
            pv.set_len(0);
            if board.in_check() {
                return -MATE_VALUE + ply;
            }
            return 0;
        }

        let mut moves = moves.into_iter().map(|m| (m, MoveOrder::classify(board, self.history, tt_entry.and_then(|e| e.m), m))).collect::<ArrayVec<[(Move, MoveOrder); 256]>>();
        moves.sort_by_key(|(_, order)| *order);

        let mut best_move = None;
        let mut best_score = i32::MIN;
        let mut raised_lower_bound = false;

        // Push the move to check for repetition draws
        keystack.push(board.hash());

        for (movecount, (m, _)) in moves.into_iter().enumerate() {
            self.nodes += 1;
            if alpha == beta - 1 {
                self.zw_nodes += 1;
            }

            if ply == 0 {
                let now = Instant::now();
                if now >= self.start + Duration::from_secs(2) {
                    output.new_move(board, depth + root_reduction, now.duration_since(self.start), self.nodes() + self.qnodes(), m);
                }
            }

            // SEE Pruning
            if !board.in_check() && depth == 2 && movecount > 1 && best_score > -MATE_VALUE + 500 {
                let see = board.static_exchange_evaluation(m);
                if m.is_capture() && see < -1 {
                    continue;
                }
                if !m.is_capture() && see < 0 {
                    continue;
                }
            }
            
            let lmp_threshold = 1.max(if depth == 1 { (3 * moves.len()) / 4 } else { (7 * moves.len()) / 8 });
            if !board.in_check() && !m.is_capture() && depth <= 2 && movecount >= lmp_threshold && best_score > -MATE_VALUE + 500 {
                continue;
            }

            let mut reduction = 1;

            if depth >= 3 && movecount >= 4 && !board.in_check() && !m.is_capture() {
                let depth = (depth as f32).ln();
                let i = (movecount as f32).ln();
                reduction += (depth * i).mul_add(self.params.lmr_mul, self.params.lmr_base) as i32;
                reduction -= i32::from(alpha != beta - 1);
                // credit: adam
            }

            let mut child_pv = ArrayVec::new();
            let child_board = board.make(m);
            let mut score = 0;

            if movecount > 0 {
                score = -self.search(
                    &child_board,
                    depth - reduction,
                    -alpha - 1,
                    -alpha,
                    output,
                    &mut child_pv,
                    ply + 1,
                    keystack,
                );
            }
            if movecount > 0 && reduction > 1 && score > alpha {
                reduction = 1;
                score = -self.search(
                    &child_board,
                    depth - reduction,
                    -alpha - 1,
                    -alpha,
                    output,
                    &mut child_pv,
                    ply + 1,
                    keystack,
                );
            }
            if movecount == 0 || alpha != beta - 1 && score > alpha {
                reduction = 1;
                score = -self.search(
                    &child_board,
                    depth - reduction,
                    -beta,
                    -alpha,
                    output,
                    &mut child_pv,
                    ply + 1,
                    keystack,
                );
            }

            if score > best_score {
                best_move = Some(m);
                best_score = score;

                // Ensure we have *a move* even when failing high/low at root.
                if ply == 0 {
                    pv.set_len(0);
                    pv.push(m);
                    for m in child_pv {
                        pv.push(m);
                    }
                }
            }

            if self.nodes.trailing_zeros() >= 10 {
                if let Some(time) = self.stop_after {
                    if Instant::now() >= time {
                        keystack.pop();
                        return best_score;
                    }
                }
            }

            if score >= beta {
                let bonus = self.params.hist_bonus_mul * depth - self.params.hist_bonus_base;
                let penalty = self.params.hist_pen_mul * depth - self.params.hist_pen_base;
                if !m.is_capture() {
                    for (m, _) in moves.into_iter().take(movecount) {
                        if m.is_capture() {
                            continue;
                        }
                        self.update_history(m, -penalty);
                    }
                    self.update_history(m, bonus);
                }

                self.beta_cutoff_index += movecount as u64;
                self.beta_cutoffs += 1;

                break;
            }

            if score > alpha {
                alpha = score;
                pv.set_len(0);
                pv.push(m);
                for m in child_pv {
                    pv.push(m);
                }
                raised_lower_bound = true;

                if ply == 0 {
                    let now = Instant::now();
                    if now >= self.start + Duration::from_secs(2) {
                        output.new_pv(
                            board,
                            depth + root_reduction,
                            score,
                            now.duration_since(self.start),
                            self.nodes() + self.qnodes(),
                            pv,
                        );
                    }
                }
            }
        }

        keystack.pop();

        self.write_tt(
            board,
            ply,
            TtData {
                m: best_move,
                score: best_score as i16,
                flags: if best_score >= beta {
                    TtFlags::Lower
                } else if raised_lower_bound {
                    TtFlags::Exact
                } else {
                    TtFlags::Upper
                },
                depth: depth as u8,
            },
        );

        if !board.in_check()
            && !best_move.unwrap().is_capture()
            && (raised_lower_bound
                || (best_score >= beta && best_score >= eval_int)
                || (best_score <= alpha && best_score <= eval_int))
        {
            self.update_corrhist(board, depth, best_score - eval_int);
        }

        best_score
    }

    #[allow(clippy::too_many_arguments)]
    pub fn search_root(
        &mut self, board: &Board, depth: i32, lower_bound: i32, upper_bound: i32, output: &mut dyn output::Output,
        pv: &mut ArrayVec<[Move; 64]>, keystack: &mut Vec<u64>,
    ) -> i32 {
        self.search(board, depth, lower_bound, upper_bound, output, pv, 0, keystack)
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

    #[must_use]
    pub fn beta_cutoff_index(&self) -> f64 {
        (self.beta_cutoff_index as f64) / (self.beta_cutoffs as f64)
    }

    #[must_use]
    pub fn q_beta_cutoff_index(&self) -> f64 {
        (self.q_beta_cutoff_index as f64) / (self.q_beta_cutoffs as f64)
    }

    #[must_use]
    pub fn zw_nodes(&self) -> f64 {
        100.0 * (self.zw_nodes as f64) / (self.nodes as f64)
    }

    #[must_use]
    pub fn zw_qnodes(&self) -> f64 {
        100.0 * (self.zw_qnodes as f64) / (self.qnodes as f64)
    }
}
