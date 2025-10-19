use std::{cmp::Ordering, sync::atomic::AtomicU64, time::Instant};

use tinyvec::ArrayVec;
use yukari_movegen::{Board, Colour, Move, Piece};

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
    pub lmp_base: i32,
    pub lmp_mul: i32,
    pub lmp_pow: u32,
    pub see_pruning_capture: f32,
    pub see_pruning_quiet: f32,
}

impl Default for SearchParams {
    fn default() -> Self {
        Self {
            rfp_margin_base: 3,
            rfp_margin_mul: 36,
            razor_margin_mul: 246,
            lmr_base: 1.018_642_9,
            lmr_mul: 0.521_101_53,
            hist_bonus_base: 260,
            hist_bonus_mul: 303,
            hist_pen_base: 251,
            hist_pen_mul: 298,
            lmp_base: 5,
            lmp_mul: 1,
            lmp_pow: 2,
            see_pruning_capture: 0.489_161_5,
            see_pruning_quiet: 0.006_385_347,
        }
    }
}

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
    tt
}

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
        board: &Board, history: &[[[i16; 64]; 64]; 12], conthist: &[[i16; 2 * 6 * 64]; 2 * 6 * 64], tt_move: Option<Move>,
        last_last_m: Option<(Piece, Move)>, last_m: Option<(Piece, Move)>, m: Move,
    ) -> Self {
        if let Some(tt_move) = tt_move
            && tt_move == m
        {
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

pub struct Search<'a> {
    nodes: u64,
    qnodes: u64,
    zw_nodes: u64,
    zw_qnodes: u64,
    seldepth: i32,
    nullmove_attempts: u64,
    nullmove_success: u64,
    beta_cutoff_index: u64,
    beta_cutoffs: u64,
    q_beta_cutoff_index: u64,
    q_beta_cutoffs: u64,
    hash_probes: u64,
    hash_hits: u64,
    hash_cutoffs: u64,
    stop_after: Option<Instant>,
    history: &'a mut [[[i16; 64]; 64]; 12],
    tt: &'a [TtEntry],
    corrhist_p: &'a mut [[i32; 16384]; 2],
    corrhist_kbn: &'a mut [[i32; 16384]; 2],
    corrhist_kqr: &'a mut [[i32; 16384]; 2],
    conthist: &'a mut [[i16; 2 * 6 * 64]; 2 * 6 * 64],
    path: ArrayVec<[Option<(Piece, Move)>; 64]>,
    eval: ArrayVec<[Option<i32>; 64]>,
    params: &'a SearchParams,
}

impl<'a> Search<'a> {
    #[must_use]
    pub fn new(
        stop_after: Option<Instant>, tt: &'a [TtEntry], history: &'a mut [[[i16; 64]; 64]; 12],
        corrhist_p: &'a mut [[i32; 16384]; 2], corrhist_kbn: &'a mut [[i32; 16384]; 2], corrhist_kqr: &'a mut [[i32; 16384]; 2], conthist: &'a mut [[i16; 2 * 6 * 64]; 2 * 6 * 64], params: &'a SearchParams,
    ) -> Self {
        Self {
            nodes: 0,
            qnodes: 0,
            zw_nodes: 0,
            zw_qnodes: 0,
            seldepth: 0,
            nullmove_attempts: 0,
            nullmove_success: 0,
            beta_cutoff_index: 0,
            beta_cutoffs: 0,
            q_beta_cutoff_index: 0,
            q_beta_cutoffs: 0,
            hash_probes: 0,
            hash_hits: 0,
            hash_cutoffs: 0,
            stop_after,
            history,
            tt,
            corrhist_p,
            corrhist_kbn,
            corrhist_kqr,
            conthist,
            path: ArrayVec::new(),
            eval: ArrayVec::new(),
            params,
        }
    }

    fn update_corrhist(&mut self, board: &Board, depth: i32, diff: i32) {
        const CORRHIST_GRAIN: i32 = 256;
        const CORRHIST_WEIGHT_SCALE: i32 = 256;
        const CORRHIST_MAX: i32 = 256 * 32;

        let diff = diff * CORRHIST_GRAIN;
        let weight = 16.min(depth + 1);

        // pawns
        let entry_p = &mut self.corrhist_p[board.side() as usize][board.data().hash_pawns() as usize & 16383];

        *entry_p = ((*entry_p * (CORRHIST_WEIGHT_SCALE - weight) + diff * weight) / CORRHIST_WEIGHT_SCALE)
            .clamp(-CORRHIST_MAX, CORRHIST_MAX);

        // kings, bishops, knights
        let entry_kbn = &mut self.corrhist_kbn[board.side() as usize][board.data().hash_kbn() as usize & 16383];

        *entry_kbn = ((*entry_kbn * (CORRHIST_WEIGHT_SCALE - weight) + diff * weight) / CORRHIST_WEIGHT_SCALE)
            .clamp(-CORRHIST_MAX, CORRHIST_MAX);

        // kings, queens, rooks
        let entry_kqr = &mut self.corrhist_kqr[board.side() as usize][board.data().hash_kqr() as usize & 16383];

        *entry_kqr = ((*entry_kqr * (CORRHIST_WEIGHT_SCALE - weight) + diff * weight) / CORRHIST_WEIGHT_SCALE)
            .clamp(-CORRHIST_MAX, CORRHIST_MAX);
    }

    fn eval_with_corrhist(&self, board: &Board, eval: i32) -> i32 {
        const CORRHIST_GRAIN: i32 = 256;
        let entry_p = self.corrhist_p[board.side() as usize][board.hash_pawns() as usize & 16383] / CORRHIST_GRAIN;
        let entry_kbn = self.corrhist_kbn[board.side() as usize][board.data().hash_kbn() as usize & 16383] / CORRHIST_GRAIN;
        let entry_kqr = self.corrhist_kqr[board.side() as usize][board.data().hash_kqr() as usize & 16383] / CORRHIST_GRAIN;
        (eval + entry_p + entry_kbn + entry_kqr).clamp(-MATE_VALUE + 1, MATE_VALUE - 1)
    }

    fn update_history(
        &mut self, board: &Board, last_last_m: Option<(Piece, Move)>, last_m: Option<(Piece, Move)>, m: Move, bonus: i32,
    ) {
        const HISTORY_MAX: i32 = 16384;
        let bonus = bonus.clamp(-HISTORY_MAX, HISTORY_MAX);
        {
            let coloured_piece = 6 * usize::from(board.side() == Colour::Black) + board.piece_from_square(m.from).unwrap() as usize;
            let history = &mut self.history[coloured_piece][m.from.into_inner() as usize][m.dest.into_inner() as usize];
            let bonus = bonus - i32::from(*history) * bonus.abs() / HISTORY_MAX;
            *history += bonus as i16;
        }
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

    fn quiesce(&mut self, board: &Board, mut alpha: i32, beta: i32, pv: &mut ArrayVec<[Move; 64]>, ply: i32) -> i32 {
        let expected_pvnode = alpha != beta - 1;
        let mut best_score = self.eval_with_corrhist(board, board.eval(board.side()));

        pv.set_len(0);

        self.seldepth = self.seldepth.max(ply);

        // Emergency bailout
        if ply == 63 {
            return best_score;
        }

        if best_score >= beta {
            return best_score;
        }
        alpha = alpha.max(best_score);

        if let Some(entry) = self.probe_tt(board, 0)
            && !expected_pvnode
        {
            let score = i32::from(entry.score);
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

        let mut index = 0;
        board.generate_captures_incremental(|m| {
            self.qnodes += 1;
            if !expected_pvnode {
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
        &mut self, board: &Board, mut depth: i32, mut alpha: i32, beta: i32, pv: &mut ArrayVec<[Move; 64]>, ply: i32,
        keystack: &mut Vec<u64>, excluded_move: Option<Move>, expected_cutnode: bool,
    ) -> i32 {
        let expected_pvnode = alpha != beta - 1;

        self.seldepth = self.seldepth.max(ply);

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

        // Check extension
        if board.in_check() {
            depth += 1;
        }

        if depth <= 0 {
            return self.quiesce(board, alpha, beta, pv, ply);
        }

        pv.set_len(0);

        self.hash_probes += 1;
        let tt_entry = self.probe_tt(board, ply);
        if let Some(entry) = tt_entry {
            self.hash_hits += 1;
            if excluded_move.is_none() && !expected_pvnode && i32::from(entry.depth) >= depth {
                let score = i32::from(entry.score);
                match entry.flags {
                    TtFlags::Exact => {
                        self.hash_cutoffs += 1;
                        return score;
                    }
                    TtFlags::Upper => {
                        if score <= alpha {
                            self.hash_cutoffs += 1;
                            return score;
                        }
                    }
                    TtFlags::Lower => {
                        if score >= beta {
                            self.hash_cutoffs += 1;
                            return score;
                        }
                    }
                }
            }
        }

        if excluded_move.is_none()
            && expected_pvnode
            && (tt_entry.is_none() || i32::from(tt_entry.unwrap().depth) + 3 < depth)
            && depth >= 3
        {
            // internal iterative reduction
            depth -= 1;
        }

        // Improving metric: are we doing better than we were two plies ago?
        let eval_int = self.eval_with_corrhist(board, board.eval(board.side()));
        let mut improving = false;
        if !board.in_check() {
            let last_eval = *self.eval.iter().rev().nth(1).unwrap_or(&None);
            improving = last_eval.is_some_and(|last_eval| eval_int > last_eval);
        }

        // Reverse futility pruning: is the static eval so good we can prune?
        let rfp_margin = self.params.rfp_margin_base + self.params.rfp_margin_mul * depth;
        let rfp_depth = if improving { 5 } else { 4 };
        if excluded_move.is_none() && !expected_pvnode && !board.in_check() && depth <= rfp_depth && eval_int - rfp_margin >= beta
        {
            return eval_int - rfp_margin;
        }

        // Razoring: is the static eval so low we can prune, and not improved by a quiescence search?
        let razor_margin = self.params.razor_margin_mul * depth;
        if excluded_move.is_none()
            && !expected_pvnode
            && !board.in_check()
            && depth <= 3
            && alpha.abs() < 2000
            && eval_int + razor_margin <= alpha
        {
            let score = self.quiesce(board, alpha, alpha + 1, pv, ply);
            if score <= alpha {
                return score;
            }
        }

        // Null-move pruning: can we skip a turn and still come off sufficiently winning we can prune?
        let reduction = if depth > 10 {
            5
        } else if depth > 6 {
            4
        } else {
            3
        } + ((eval_int - beta) / 200).max(0)
            + i32::from(improving);
        if excluded_move.is_none() && !expected_pvnode && !board.in_check() && depth >= 2 && eval_int >= beta {
            keystack.push(board.hash());
            let board = board.make_null();
            let mut child_pv = ArrayVec::new();
            self.path.push(None);
            let score = -self.search(&board, depth - 1 - reduction, -beta, -beta + 1, &mut child_pv, ply + 1, keystack, None, !expected_cutnode);
            self.path.pop();
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

        // Is this a singular search where we have excluded the only legal move?
        if moves.len() == 1 && excluded_move.is_some() {
            return alpha;
        }

        let mut moves = {
            let tt_move = tt_entry.and_then(|e| e.m);
            let last_move = *self.path.last().unwrap_or(&None);
            let last_last_move = *self.path.iter().rev().nth(1).unwrap_or(&None);
            moves
                .into_iter()
                .map(|m| (m, MoveOrder::classify(board, self.history, self.conthist, tt_move, last_last_move, last_move, m)))
                .collect::<ArrayVec<[(Move, MoveOrder); 256]>>()
        };
        moves.sort_by_key(|(_, order)| *order);

        let mut best_move = None;
        let mut best_score = i32::MIN;
        let mut raised_lower_bound = false;

        // Push the move to check for repetition draws
        if excluded_move.is_none() {
            keystack.push(board.hash());
            if board.in_check() {
                self.eval.push(None);
            } else {
                self.eval.push(Some(eval_int));
            }
        }

        for (movecount, (m, _)) in moves.into_iter().enumerate() {
            if let Some(excluded_move) = excluded_move
                && excluded_move == m
            {
                continue;
            }

            self.nodes += 1;
            if !expected_pvnode {
                self.zw_nodes += 1;
            }

            // SEE Pruning
            if !board.in_check() && (2..=5).contains(&depth) && movecount > 1 && best_score > -MATE_VALUE + 500 {
                let threshold = if m.is_capture() {
                    -(depth as f32 * self.params.see_pruning_capture) as i32
                } else {
                    -(depth as f32 * self.params.see_pruning_quiet) as i32
                };
                if board.static_exchange_evaluation(m) < threshold {
                    continue;
                }
            }

            // Late Move Pruning
            let lmp_threshold = self.params.lmp_base + (((self.params.lmp_mul * depth).pow(self.params.lmp_pow)) >> i32::from(!improving));
            if !board.in_check() && !m.is_capture() && depth <= 3 && movecount >= lmp_threshold as usize && best_score > -MATE_VALUE + 500 {
                continue;
            }

            let mut extension = 0;
            let mut reduction = 1;

            // Singular extension: is the TT move uniquely good?
            if let Some(tt_entry) = tt_entry
                && excluded_move.is_none()
                && ply > 0
                && Some(m) == tt_entry.m
            {

                if depth >= 7
                    && matches!(tt_entry.flags, TtFlags::Exact | TtFlags::Lower)
                    && tt_entry.score.abs() < 9500
                {
                    let singular_beta = (i32::from(tt_entry.score) - depth * 2).max(-MATE_VALUE + 1);
                    let singular_depth = (depth - 1) / 2;
                    let score = self.search(board, singular_depth, singular_beta - 1, singular_beta, pv, ply, keystack, Some(m), expected_cutnode);

                    // Multicut: Another move failed high, so this position is very good; prune.
                    if score >= singular_beta && singular_beta >= beta {
                        keystack.pop();
                        self.eval.pop();
                        return singular_beta;
                    }

                    if score < singular_beta {
                        // The TT move seems uniquely good; extend.
                        extension += 1;
                    } else if i32::from(tt_entry.score) >= beta {
                        extension -= 1;
                    }
                } else if depth <= 7 && !board.in_check() && eval_int <= alpha - 26 && tt_entry.flags == TtFlags::Lower {
                    // Low-depth singular extension
                    extension += 1;
                }
            }

            self.path.push(Some((board.piece_from_square(m.from).unwrap(), m)));

            // Late Move Reduction
            if depth >= 3 && movecount >= 4 && !m.is_capture() {
                let depth = (depth as f32).ln();
                let movecount = (movecount as f32).ln();
                reduction += (depth * movecount).mul_add(self.params.lmr_mul, self.params.lmr_base) as i32;
                reduction -= i32::from(expected_pvnode);
                // credit: adam
            }

            let mut child_pv = ArrayVec::new();
            let child_board = board.make(m);
            let mut score = 0;

            if movecount > 0 {
                score = -self.search(
                    &child_board,
                    depth - reduction + extension,
                    -alpha - 1,
                    -alpha,
                    &mut child_pv,
                    ply + 1,
                    keystack,
                    None,
                    reduction > 1 || !expected_cutnode,
                );
            }
            if movecount > 0 && reduction > 1 && score > alpha {
                reduction = 1;
                score = -self.search(
                    &child_board,
                    depth - reduction + extension,
                    -alpha - 1,
                    -alpha,
                    &mut child_pv,
                    ply + 1,
                    keystack,
                    None,
                    !expected_cutnode,
                );
            }
            if movecount == 0 || expected_pvnode && score > alpha {
                reduction = 1;
                score = -self.search(
                    &child_board,
                    depth - reduction + extension,
                    -beta,
                    -alpha,
                    &mut child_pv,
                    ply + 1,
                    keystack,
                    None,
                    false
                );
            }

            self.path.pop();

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

            if self.nodes.trailing_zeros() >= 10
                && let Some(time) = self.stop_after
                && Instant::now() >= time
            {
                if excluded_move.is_none() {
                    keystack.pop();
                    self.eval.pop();
                }
                return best_score;
            }

            if score >= beta {
                let bonus = self.params.hist_bonus_mul * depth - self.params.hist_bonus_base;
                let penalty = self.params.hist_pen_mul * depth - self.params.hist_pen_base;
                let last_move = *self.path.last().unwrap_or(&None);
                let last_last_move = *self.path.iter().rev().nth(1).unwrap_or(&None);
                if !m.is_capture() {
                    for (m, _) in moves.into_iter().take(movecount) {
                        if m.is_capture() {
                            continue;
                        }
                        self.update_history(board, last_last_move, last_move, m, -penalty);
                    }
                    self.update_history(board, last_last_move, last_move, m, bonus);
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
            }
        }

        if excluded_move.is_none() {
            keystack.pop();
            self.eval.pop();
        }

        if excluded_move.is_none() {
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
        }

        best_score
    }

    #[allow(clippy::too_many_arguments)]
    pub fn search_root(
        &mut self, board: &Board, depth: i32, lower_bound: i32, upper_bound: i32, pv: &mut ArrayVec<[Move; 64]>,
        keystack: &mut Vec<u64>,
    ) -> i32 {
        self.seldepth = 0;
        let score = self.search(board, depth, lower_bound, upper_bound, pv, 0, keystack, None, false);
        assert_eq!(self.path.len(), 0);
        assert_eq!(self.eval.len(), 0);
        score
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

    #[must_use]
    pub fn seldepth(&self) -> i32 {
        self.seldepth
    }

    #[must_use]
    pub fn tt_hit_rate(&self) -> f64 {
        100.0 * (self.hash_hits as f64) / (self.hash_probes as f64)
    }

    #[must_use]
    pub fn tt_cutoff_rate(&self) -> f64 {
        100.0 * (self.hash_cutoffs as f64) / (self.hash_probes as f64)
    }
}
