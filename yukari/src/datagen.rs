use std::{
    io::Write,
    sync::Mutex,
    time::{Duration, Instant},
};

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rand::seq::IteratorRandom;
use rand::Rng;
use tinyvec::ArrayVec;
use yukari_movegen::{Board, Colour, Move, MoveType, Piece, Square};

use crate::search;

const VERDICT_REJECT_CP: i16 = 1000;

const WIN_ADJ_CP: i16 = 400;
const WIN_ADJ_PLIES: i32 = 6;

const DRAW_ADJ_CP: i16 = 10;
const DRAW_ADJ_PLIES: i32 = 16;
const DRAW_ADJ_MIN_POSITIONS: usize = 40;

const ROLLOUT_NODE_CAP: u64 = 5_000;
const ROLLOUT_TIME: Duration = Duration::from_millis(250);

const VERDICT_DEPTH: i32 = 10;
const VERDICT_TIME: Duration = Duration::from_secs(2);

#[derive(Clone, Copy)]
#[repr(u8)]
enum MarlinWdl {
    BlackWin = 0,
    Draw = 1,
    WhiteWin = 2,
}

#[repr(C)]
struct MarlinFormat {
    occupancy:       u64,
    pieces:          [u8; 16], // [u4; 32]
    stm_ep_square:   u8,
    halfmove_clock:  u8,
    fullmove_number: u16,
    eval:            i16,
    wdl:             MarlinWdl,
    _extra:          u8,
}

impl From<&Board> for MarlinFormat {
    fn from(board: &Board) -> Self {
        let mut this = Self {
            occupancy:       0,
            pieces:          [0; 16],
            stm_ep_square:   0,
            halfmove_clock:  0,
            fullmove_number: 0,
            eval:            0,
            wdl:             MarlinWdl::Draw,
            _extra:          0,
        };
        for sq in 0..64 {
            let square = unsafe { Square::from_u8_unchecked(sq) };
            let Some(piece_idx) = board.data().piece_index(square) else { continue };

            let piece = board.piece_from_bit(piece_idx);
            let castle = board.castle();
            let unmoved_rook = piece == Piece::Rook && (0..4).any(|idx| castle.rook_square(idx) == Some(square));

            // marlinformat uses piece code 6 ("unmoved rook") to mark castling rights.
            let code = if unmoved_rook { 6 } else { piece as u8 };
            let mut nibble = code | ((piece_idx.colour() as u8) << 3);

            let piece_count = this.occupancy.count_ones() as usize;
            if piece_count % 2 == 1 {
                nibble <<= 4;
            }
            this.pieces[piece_count / 2] |= nibble;
            this.occupancy |= 1_u64 << sq;
        }

        this.stm_ep_square = ((board.side() as u8) << 7) | (board.ep().map_or(64, yukari_movegen::Square::into_inner));

        this
    }
}

impl MarlinFormat {
    pub fn write(&self, f: &mut impl Write) {
        f.write_all(&self.occupancy.to_le_bytes()).unwrap();
        f.write_all(&self.pieces).unwrap();
        f.write_all(&[self.stm_ep_square, self.halfmove_clock]).unwrap();
        f.write_all(&self.fullmove_number.to_le_bytes()).unwrap();
        f.write_all(&self.eval.to_le_bytes()).unwrap();
        f.write_all(&[self.wdl as u8, self._extra]).unwrap();
    }
}

#[repr(transparent)]
struct ViriMove(u16);

impl From<Move> for ViriMove {
    fn from(m: Move) -> Self {
        let (from, dest) = (u16::from(m.from().into_inner()), u16::from(m.dest().into_inner()));
        let prom = match m.promotion_piece() {
            None => 0,
            Some(Piece::Knight) => 0,
            Some(Piece::Bishop) => 1,
            Some(Piece::Rook) => 2,
            Some(Piece::Queen) => 3,
            Some(_) => unreachable!("invalid promotion piece"),
        };
        // viriformat flag bits: 0=normal, 1=en passant, 2=castle, 3=promotion.
        let flags = match m.kind() {
            MoveType::EnPassant => 1,
            MoveType::KingsideCastle | MoveType::QueensideCastle => 2,
            MoveType::PromotionKnight
            | MoveType::PromotionBishop
            | MoveType::PromotionRook
            | MoveType::PromotionQueen
            | MoveType::CapturePromotionKnight
            | MoveType::CapturePromotionBishop
            | MoveType::CapturePromotionRook
            | MoveType::CapturePromotionQueen => 3,
            _ => 0,
        };

        Self(from | (dest << 6) | (prom << 12) | (flags << 14))
    }
}

struct ViriFormat {
    position: MarlinFormat,
    moves:    Vec<(ViriMove, i16)>,
}

impl ViriFormat {
    pub fn new(board: &Board) -> Self {
        Self { position: MarlinFormat::from(board), moves: Vec::new() }
    }

    pub fn push(&mut self, m: Move, score: i16) {
        self.moves.push((ViriMove::from(m), score));
    }

    /// Serialize the game into a single viriformat record: marlinformat
    /// header, then `(move, score)` pairs, terminated by a four-byte zero
    /// sentinel. Returned buffer is ready to be written verbatim.
    pub fn finish(mut self, result: MarlinWdl) -> Vec<u8> {
        self.position.wdl = result;

        let mut buf = Vec::with_capacity(32 + self.moves.len() * 4 + 4);

        // marlinformat header
        self.position.write(&mut buf);

        // move/score stream
        for (m, score) in self.moves {
            buf.extend_from_slice(&m.0.to_le_bytes());
            buf.extend_from_slice(&score.to_le_bytes());
        }

        // end-of-game sentinel
        buf.extend_from_slice(&[0, 0, 0, 0]);
        buf
    }
}

pub struct DataGen<'a, T: Write> {
    f:      &'a Mutex<T>,
    search: search::Search,
    rng:    rand::rngs::ThreadRng,
    pv:     Vec<Move>,
}

impl<'a, T: Write> DataGen<'a, T> {
    pub fn new(f: &'a Mutex<T>) -> DataGen<'a, T> {
        let mut this = Self { f, search: search::Search::new(1), rng: rand::rng(), pv: Vec::new() };
        this.search.allocate_tt(16);
        this
    }

    /// Play one accepted game and return its position count.
    /// Retries internally on rejected openings (early mate, lopsided verdict).
    pub fn play_one(&mut self) -> usize {
        loop {
            if let Some(positions) = self.play_game() {
                return positions;
            }
        }
    }

    /// Serialize `game` locally, then hand the bytes to the shared writer
    /// in a single `write_all`. Keeps the mutex critical section down to
    /// one syscall's worth of work rather than O(moves).
    fn emit(&self, game: ViriFormat, wdl: MarlinWdl) {
        let buf = game.finish(wdl);
        self.f.lock().unwrap().write_all(&buf).unwrap();
    }

    fn search_rollout(&mut self, board: &Board, keystack: &[u64]) -> Option<(Move, i16)> {
        self.iterate(board, keystack, ROLLOUT_TIME, 63, Some(ROLLOUT_NODE_CAP))
    }

    /// Used once to vet the random opening.
    fn search_verdict(&mut self, board: &Board, keystack: &[u64]) -> Option<(Move, i16)> {
        self.iterate(board, keystack, VERDICT_TIME, VERDICT_DEPTH, None)
    }

    /// Iterative-deepening driver shared by rollout and verdict searches.
    /// Breaks between iterations once `node_cap` is reached so the returned
    /// PV is always from a completed depth.
    fn iterate(
        &mut self, board: &Board, keystack: &[u64], time_cap: Duration, max_depth: i32, node_cap: Option<u64>,
    ) -> Option<(Move, i16)> {
        const ASPIRATION_MIN_DEPTH: i32 = 5;
        const ASPIRATION_WINDOW: i32 = 30;

        let stop_after = Instant::now() + time_cap;
        let mut score = 0;
        self.pv.clear();

        self.search.prepare(board, Some(stop_after), None, keystack);

        for depth in 0..=max_depth {
            // Aspirate around the previous score once deep enough;
            // if it falls outside the window, fall back to a full re-search.
            let (alpha, beta) = if depth >= ASPIRATION_MIN_DEPTH {
                (score - ASPIRATION_WINDOW, score + ASPIRATION_WINDOW)
            } else {
                (-i32::MAX, i32::MAX)
            };

            self.pv.clear();
            let mut s = self.search.search(depth, alpha, beta, &mut self.pv);
            if s <= alpha || s >= beta {
                self.pv.clear();
                s = self.search.search(depth, -i32::MAX, i32::MAX, &mut self.pv);
            }
            score = s;

            if let Some(cap) = node_cap
                && self.search.nodes() + self.search.qnodes() > cap
            {
                break;
            }
        }
        if self.pv.is_empty() {
            return None;
        }
        Some((self.pv[0], score.clamp(-10_000, 10_000) as i16))
    }

    fn play_game(&mut self) -> Option<usize> {
        let mut yukari_board = Board::dfrc(self.rng.random_range(0..960), self.rng.random_range(0..960));
        let mut keystack = vec![yukari_board.hash()];

        // Opening: eight random moves.
        for _ in 0..8 {
            let mut moves = ArrayVec::new();
            yukari_board.generate(&mut moves);
            let Some(&m) = moves.iter().choose(&mut self.rng) else {
                // checkmate in the opening, maybe?
                return None;
            };
            yukari_board = yukari_board.make(m);
            keystack.push(yukari_board.hash());
        }

        // Check: the "opening" must not be excessively lopsided.
        let mut game = {
            let Some((_, score)) = self.search_verdict(&yukari_board, &keystack) else {
                // checkmate???
                return None;
            };
            if score.abs() >= VERDICT_REJECT_CP {
                return None;
            }
            ViriFormat::new(&yukari_board)
        };

        let mut positions = 0_usize;
        let mut draw_adj_counter = 0;
        let mut win_adj_counter = 0;
        let mut win_adj_white = true;

        // Rollout: "soft 5k nodes" until game end.
        loop {
            // Mate or stalemate: no legal moves.
            let mut moves = ArrayVec::new();
            yukari_board.generate(&mut moves);
            if moves.is_empty() {
                let wdl = if yukari_board.in_check() {
                    // Side to move is mated.
                    if yukari_board.side() == Colour::White { MarlinWdl::BlackWin } else { MarlinWdl::WhiteWin }
                } else {
                    MarlinWdl::Draw
                };
                self.emit(game, wdl);
                return Some(positions);
            }

            if yukari_board.insufficient_material() {
                self.emit(game, MarlinWdl::Draw);
                return Some(positions);
            }

            // Threefold repetition.
            let hash = yukari_board.hash();
            let reps = keystack.iter().filter(|k| **k == hash).take(3).count();
            if reps == 3 {
                self.emit(game, MarlinWdl::Draw);
                return Some(positions);
            }

            // Fifty-move rule.
            if yukari_board.fifty() == 100 {
                self.emit(game, MarlinWdl::Draw);
                return Some(positions);
            }

            // Can we adjudicate?
            if win_adj_counter >= WIN_ADJ_PLIES {
                let wdl = if win_adj_white { MarlinWdl::WhiteWin } else { MarlinWdl::BlackWin };
                self.emit(game, wdl);
                return Some(positions);
            }

            if draw_adj_counter >= DRAW_ADJ_PLIES && positions >= DRAW_ADJ_MIN_POSITIONS {
                self.emit(game, MarlinWdl::Draw);
                return Some(positions);
            }

            let Some((m, score)) = self.search_rollout(&yukari_board, &keystack) else {
                eprintln!("search did not find a move on board {yukari_board}");
                return None;
            };

            let score = if yukari_board.side() == Colour::Black { -score } else { score };
            game.push(m, score);
            yukari_board = yukari_board.make(m);
            keystack.push(yukari_board.hash());
            positions += 1;

            if score.abs() <= DRAW_ADJ_CP {
                draw_adj_counter += 1;
            } else {
                draw_adj_counter = 0;
            }

            // Require a consistent winner across WIN_ADJ_PLIES;
            // a sign flip restarts the count on the new side.
            if score >= WIN_ADJ_CP {
                if win_adj_white {
                    win_adj_counter += 1;
                } else {
                    win_adj_counter = 1;
                    win_adj_white = true;
                }
            } else if score <= -WIN_ADJ_CP {
                if !win_adj_white {
                    win_adj_counter += 1;
                } else {
                    win_adj_counter = 1;
                    win_adj_white = false;
                }
            } else {
                win_adj_counter = 0;
            }
        }
    }
}
