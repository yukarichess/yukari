use std::{
    io::Write,
    sync::Mutex,
    time::{Duration, Instant},
};

use rand::seq::IteratorRandom;
use tinyvec::ArrayVec;
use yukari_movegen::{Board, Colour, File, Move, MoveType, Piece, Rank, Square};

use crate::search;

#[derive(Clone, Copy)]
#[repr(u8)]
enum MarlinWdl {
    BlackWin = 0,
    Draw = 1,
    WhiteWin = 2,
}

#[repr(C)]
struct MarlinFormat {
    occupancy: u64,
    pieces: [u8; 16], // [u4; 32]
    stm_ep_square: u8,
    halfmove_clock: u8,
    fullmove_number: u16,
    eval: i16,
    wdl: MarlinWdl,
    _extra: u8,
}

impl From<Board> for MarlinFormat {
    fn from(board: Board) -> Self {
        let a1 = Square::from_rank_file(Rank::One, File::A);
        let a8 = Square::from_rank_file(Rank::Eight, File::A);
        let h1 = Square::from_rank_file(Rank::One, File::H);
        let h8 = Square::from_rank_file(Rank::Eight, File::H);

        let mut this = Self {
            occupancy: 0,
            pieces: [0; 16],
            stm_ep_square: 0,
            halfmove_clock: 0,
            fullmove_number: 0,
            eval: 0,
            wdl: MarlinWdl::Draw,
            _extra: 0,
        };
        for sq in 0..64 {
            let square = unsafe { Square::from_u8_unchecked(sq) };
            let Some(piece) = board.data().piece_index(square) else { continue };

            let mut piece = if board.piece_from_bit(piece) == Piece::Rook {
                if (board.castle().0 && square == h1)
                    || (board.castle().1 && square == a1)
                    || (board.castle().2 && square == h8)
                    || (board.castle().3 && square == a8)
                {
                    // "unmoved rook" to represent castling rights.
                    (6_u8) | ((piece.colour() as u8) << 3)
                } else {
                    (board.piece_from_bit(piece) as u8) | ((piece.colour() as u8) << 3)
                }
            } else {
                (board.piece_from_bit(piece) as u8) | ((piece.colour() as u8) << 3)
            };
            let piece_count = this.occupancy.count_ones() as usize;
            if piece_count % 2 == 1 {
                piece <<= 4;
            }
            this.pieces[piece_count / 2] |= piece;
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
        let (from, dest) = if matches!(m.kind(), MoveType::KingsideCastle | MoveType::QueensideCastle) {
            // convert from yukari's "king two squares" castling to viridithas' "king takes rook" castling.
            let rank = Rank::from(m.dest());
            let file = File::from(m.dest());
            let from = u16::from(m.from().into_inner());
            let dest = match (rank, file) {
                (Rank::One, File::G) => u16::from(Square::from_rank_file(Rank::One, File::H).into_inner()),
                (Rank::One, File::C) => u16::from(Square::from_rank_file(Rank::One, File::A).into_inner()),
                (Rank::Eight, File::G) => u16::from(Square::from_rank_file(Rank::Eight, File::H).into_inner()),
                (Rank::Eight, File::C) => u16::from(Square::from_rank_file(Rank::Eight, File::A).into_inner()),
                _ => panic!("unrecognised castling to-square"),
            };
            (from, dest)
        } else {
            (u16::from(m.from().into_inner()), u16::from(m.dest().into_inner()))
        };
        let prom = match m.promotion_piece() {
            None => 0,
            Some(Piece::Knight) => 0,
            Some(Piece::Bishop) => 1,
            Some(Piece::Rook) => 2,
            Some(Piece::Queen) => 3,
            Some(_) => unreachable!("invalid promotion piece"),
        };
        let flags = match m.kind() {
            yukari_movegen::MoveType::Normal => 0,
            yukari_movegen::MoveType::Capture => 0,
            yukari_movegen::MoveType::KingsideCastle => 2,
            yukari_movegen::MoveType::QueensideCastle => 2,
            yukari_movegen::MoveType::DoublePush => 0,
            yukari_movegen::MoveType::EnPassant => 1,
            yukari_movegen::MoveType::PromotionKnight => 3,
            yukari_movegen::MoveType::PromotionBishop => 3,
            yukari_movegen::MoveType::PromotionRook => 3,
            yukari_movegen::MoveType::PromotionQueen => 3,
            yukari_movegen::MoveType::CapturePromotionKnight => 3,
            yukari_movegen::MoveType::CapturePromotionBishop => 3,
            yukari_movegen::MoveType::CapturePromotionRook => 3,
            yukari_movegen::MoveType::CapturePromotionQueen => 3,
            yukari_movegen::MoveType::_Unused1 => 0,
            yukari_movegen::MoveType::_Unused2 => 0,
        };

        Self(from | (dest << 6) | (prom << 12) | (flags << 14))
    }
}

struct ViriFormat {
    position: MarlinFormat,
    moves: Vec<(ViriMove, i16)>,
}

impl ViriFormat {
    pub fn new(board: Board) -> Self {
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
    f: &'a Mutex<T>,
    search: search::Search,
    rng: rand::rngs::ThreadRng,
    positions: usize,
}

impl<'a, T: Write> DataGen<'a, T> {
    pub fn new(f: &'a Mutex<T>) -> DataGen<'a, T> {
        let mut this = Self {
            f,
            search: search::Search::new(1),
            rng: rand::rng(),
            positions: 0,
        };
        this.search.allocate_tt(16);
        this
    }

    pub fn play(&mut self, mut games: usize) -> usize {
        while games > 0 {
            self.positions = 0;
            if self.play_game() {
                games -= 1;
            }
        }
        self.positions
    }

    /// Serialize `game` locally, then hand the bytes to the shared writer
    /// in a single `write_all`. Keeps the mutex critical section down to
    /// one syscall's worth of work rather than O(moves).
    fn emit(&self, game: ViriFormat, wdl: MarlinWdl) {
        let buf = game.finish(wdl);
        self.f.lock().unwrap().write_all(&buf).unwrap();
    }

    fn search(&mut self, board: Board, keystack: &[u64], node_limit: bool) -> Option<(Move, i16)> {
        let start = Instant::now();
        let stop_after = start + Duration::from_secs_f32(if node_limit { 0.25 } else { 2.0 });
        let mut pv = Vec::new();
        let mut score = 0;

        self.search.prepare(&board, Some(stop_after), None, keystack);

        for depth in 0..=63 {
            pv.clear();
            score = self.search.search(depth, -i32::MAX, i32::MAX, &mut pv);
            if node_limit && (self.search.nodes() + self.search.qnodes()) > 5_000 {
                break;
            }
            if !node_limit && depth == 10 {
                break;
            }
        }
        if pv.is_empty() {
            return None;
        }
        Some((pv[0], score.clamp(-10_000, 10_000) as i16))
    }

    fn play_game(&mut self) -> bool {
        let mut yukari_board = Board::startpos();
        let mut cc_board = cozy_chess::Board::startpos();
        let mut keystack = vec![yukari_board.hash()];

        // Opening: eight random moves.
        for _ in 0..8 {
            let mut moves = ArrayVec::new();
            yukari_board.generate(&mut moves);
            let Some(&m) = moves.iter().choose(&mut self.rng) else {
                // checkmate in the opening, maybe?
                return false;
            };
            yukari_board = yukari_board.make(m);
            keystack.push(yukari_board.hash());
            let m_str = format!("{m}");
            let Ok(cc_m) = cozy_chess::util::parse_uci_move(&cc_board, &m_str) else {
                eprintln!("cozy-chess considers move {m} on board {cc_board} to be invalid!");
                return false;
            };
            let Ok(()) = cc_board.try_play(cc_m) else {
                eprintln!("cozy-chess considers move {m} on board {cc_board} to be illegal!");
                return false;
            };
        }

        // Check: the "opening" must not be excessively lopsided.
        let mut game = {
            let Some((_, score)) = self.search(yukari_board.clone(), &keystack, false) else {
                // checkmate???
                return false;
            };
            if score.abs() >= 1000 {
                return false;
            }
            ViriFormat::new(yukari_board.clone())
        };

        let mut draw_adj_counter = 0;
        let mut win_adj_counter = 0;
        let mut win_adj_white = true;

        // Rollout: "soft 5k nodes" until game end.
        loop {
            // Game ended?
            match cc_board.status() {
                cozy_chess::GameStatus::Ongoing => {
                    // cozy-chess doesn't track insufficient material, so check ourselves.
                    if yukari_board.insufficient_material() {
                        self.emit(game, MarlinWdl::Draw);
                        return true;
                    }
                    // Threefold repetition.
                    let yukari_reps = keystack.iter().filter(|key| **key == yukari_board.hash()).count();
                    if yukari_reps == 3 {
                        self.emit(game, MarlinWdl::Draw);
                        return true;
                    }
                }
                cozy_chess::GameStatus::Drawn => {
                    self.emit(game, MarlinWdl::Draw);
                    return true;
                }
                cozy_chess::GameStatus::Won => {
                    let wdl = if yukari_board.side() == Colour::White { MarlinWdl::BlackWin } else { MarlinWdl::WhiteWin };
                    self.emit(game, wdl);
                    return true;
                }
            }

            // Can we adjudicate?
            if win_adj_counter >= 6 {
                let wdl = if win_adj_white { MarlinWdl::WhiteWin } else { MarlinWdl::BlackWin };
                self.emit(game, wdl);
                return true;
            }

            if draw_adj_counter >= 16 && self.positions >= 40 {
                self.emit(game, MarlinWdl::Draw);
                return true;
            }

            let Some((m, score)) = self.search(yukari_board.clone(), &keystack, true) else {
                eprintln!("search did not find a move on board {yukari_board}");
                return false;
            };
            let m_str = format!("{m}");
            let Ok(cc_m) = cozy_chess::util::parse_uci_move(&cc_board, &m_str) else {
                eprintln!("cozy-chess considers move {m} on board {cc_board} to be invalid!");
                return false;
            };
            let Ok(()) = cc_board.try_play(cc_m) else {
                eprintln!("cozy-chess considers move {m} on board {cc_board} to be illegal!");
                return false;
            };

            let score = if yukari_board.side() == Colour::Black { -score } else { score };
            game.push(m, score);
            yukari_board = yukari_board.make(m);
            keystack.push(yukari_board.hash());
            self.positions += 1;

            if score.abs() <= 10 {
                draw_adj_counter += 1;
            } else {
                draw_adj_counter = 0;
            }

            if score.abs() >= 400 {
                win_adj_counter += 1;
                win_adj_white = score >= 400;
            } else {
                win_adj_counter = 0;
                win_adj_white = false;
            }
        }
    }
}
