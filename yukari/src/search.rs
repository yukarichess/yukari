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
        board: &Board, m: Move,
    ) -> Self {
        Self::Quiet(0)
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

    pub fn search(&mut self, depth: i32, mut alpha: i32, beta: i32, ply: usize) -> i32 {
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
            moves
                .into_iter()
                .map(|m| (m, MoveOrder::classify(&self.board[ply], m)))
                .collect::<ArrayVec<[(Move, MoveOrder); 256]>>()
        };
        moves.sort_by_key(|(_, order)| *order);

        self.keystack.push(self.board[ply].hash());

        let mut best = i32::MIN;

        for (m, _) in &moves {
            self.nodes += 1;

            if self.board.len() <= ply + 1 {
                self.board.push(self.board[ply].make(*m));
            } else {
                self.board[ply+1] = self.board[ply].make(*m);
            }

            let score = -self.search(depth - 1, -beta, -alpha, ply + 1);

            if score > best {
                best = score;

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
            }

            if score >= beta {
                self.keystack.pop();
                return score;
            }
        }

        self.keystack.pop();

        best
    }
}

#[derive(Clone)]
pub struct Search {
    threads: Vec<Thread>,
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
            thread.search(depth, alpha, beta, 0)
        }).collect::<Vec<_>>();

        *pv = self.threads[0].pv[0].clone();
        scores[0]
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
