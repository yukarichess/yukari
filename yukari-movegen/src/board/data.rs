use std::simd::{cmp::SimdPartialEq, u8x64, u32x64};

use super::{
    bitlist::{Bitlist, BitlistArray},
    eval::Eval,
    index::{PieceIndex, PieceIndexArray},
    piecelist::Piecelist,
    piecemask::Piecemask,
    zobrist::Zobrist,
};
use crate::{
    File,
    colour::Colour,
    piece::Piece,
    square::{Direction, Square, Square16x8},
};

#[allow(clippy::module_name_repetitions)]
#[derive(Clone)]
pub struct BoardData {
    bitlist:      BitlistArray,
    piecelist:    Piecelist,
    index:        PieceIndexArray,
    piecemask:    Piecemask,
    /// Zobrist hash.
    hash_pawns:   u64,
    hash_knights: u64,
    hash_bishops: u64,
    hash_rooks:   u64,
    hash_queens:  u64,
    hash_kings:   u64,
    hash_other:   u64,
    /// Evaluation state.
    eval:         Eval,
}

impl Default for BoardData {
    fn default() -> Self {
        Self::new()
    }
}

impl BoardData {
    /// Create a new board.
    pub fn new() -> Self {
        Self {
            bitlist:      BitlistArray::new(),
            piecelist:    Piecelist::new(),
            index:        PieceIndexArray::new(),
            piecemask:    Piecemask::new(),
            hash_pawns:   0,
            hash_knights: 0,
            hash_bishops: 0,
            hash_rooks:   0,
            hash_queens:  0,
            hash_kings:   0,
            hash_other:   0,
            eval:         Eval::new(),
        }
    }

    /// Borrow the internal attack table.
    pub const fn attacks(&self) -> &BitlistArray {
        &self.bitlist
    }

    /// Borrow the internal piece mask.
    pub const fn piecemask(&self) -> &Piecemask {
        &self.piecemask
    }

    /// Return the piece index on a square, if any.
    pub fn piece_index(&self, square: Square) -> Option<PieceIndex> {
        self.index[square]
    }

    /// Return the attacks to a square by a colour.
    pub fn attacks_to(&self, square: Square, colour: Colour) -> Bitlist {
        self.bitlist[square] & Bitlist::mask_from_colour(colour)
    }

    /// Return the square a piece resides on.
    pub fn square_of_piece(&self, bit: PieceIndex) -> Square {
        self.piecelist.get(bit)
    }

    /// True if the square has a piece on it.
    pub fn has_piece(&self, square: Square) -> bool {
        self.index[square].is_some()
    }

    /// Return the square of the king of a given colour.
    pub fn king_square(&self, colour: Colour) -> Square {
        let king_index = unsafe { (self.piecemask.kings() & Bitlist::mask_from_colour(colour)).peek_nonzero() };
        self.square_of_piece(king_index)
    }

    /// Given a piece index, return its piece type.
    pub const fn piece_from_bit(&self, bit: PieceIndex) -> Piece {
        if let Some(piece) = self.piecemask.piece(bit) {
            piece
        } else {
            panic!("piece index corresponds to invalid piece");
        }
    }

    /// Given a square, return the piece type of it, if any.
    pub fn piece_from_square(&self, square: Square) -> Option<Piece> {
        self.piecemask.piece(self.index[square]?)
    }

    /// Given a square, return the colour of the piece on it, if any.
    pub fn colour_from_square(&self, square: Square) -> Option<Colour> {
        Some(Colour::from(self.index[square]?))
    }

    /// Zobrist hash of this position.
    pub const fn hash(&self) -> u64 {
        self.hash_pawns
            ^ self.hash_knights
            ^ self.hash_bishops
            ^ self.hash_rooks
            ^ self.hash_queens
            ^ self.hash_kings
            ^ self.hash_other
    }

    /// Pawn-only Zobrist hash of this position.
    pub const fn hash_pawns(&self) -> u64 {
        self.hash_pawns
    }

    /// Non-pawn Zobrist hash of this position.
    pub fn hash_nonpawn(&self, colour: Colour) -> u64 {
        let mut hash = 0;
        for king in self.piecemask.kings() {
            let square = self.square_of_piece(king);
            if king.colour() != colour {
                continue;
            }
            Zobrist::add_piece(colour, Piece::King, square, &mut hash);
        }
        for queen in self.piecemask.queens() {
            let square = self.square_of_piece(queen);
            if queen.colour() != colour {
                continue;
            }
            Zobrist::add_piece(colour, Piece::Queen, square, &mut hash);
        }
        for rook in self.piecemask.rooks() {
            let square = self.square_of_piece(rook);
            if rook.colour() != colour {
                continue;
            }
            Zobrist::add_piece(colour, Piece::Rook, square, &mut hash);
        }
        for bishop in self.piecemask.bishops() {
            let square = self.square_of_piece(bishop);
            if bishop.colour() != colour {
                continue;
            }
            Zobrist::add_piece(colour, Piece::Bishop, square, &mut hash);
        }
        for knight in self.piecemask.knights() {
            let square = self.square_of_piece(knight);
            if knight.colour() != colour {
                continue;
            }
            Zobrist::add_piece(colour, Piece::Knight, square, &mut hash);
        }
        hash
    }

    /// (king, bishop, knight)-only Zobrist hash of this position.
    pub const fn hash_kbn(&self) -> u64 {
        self.hash_knights ^ self.hash_bishops ^ self.hash_kings
    }

    /// (king, queen, rook)-only Zobrist hash of this position.
    pub const fn hash_kqr(&self) -> u64 {
        self.hash_rooks ^ self.hash_queens ^ self.hash_kings
    }

    /// Add a `Piece` to a `Square`.
    pub fn add_piece(&mut self, piece: Piece, colour: Colour, square: Square, update: bool) {
        let piece_index = self.piecemask.add_piece(piece, colour);
        self.piecelist.add_piece(piece_index, square);
        self.index.add_piece(piece_index, square);
        let piece = self.piece_from_bit(piece_index);
        let hash = match piece {
            Piece::Pawn => &mut self.hash_pawns,
            Piece::Knight => &mut self.hash_knights,
            Piece::Bishop => &mut self.hash_bishops,
            Piece::Rook => &mut self.hash_rooks,
            Piece::Queen => &mut self.hash_queens,
            Piece::King => &mut self.hash_kings,
        };
        Zobrist::add_piece(colour, piece, square, hash);

        if update {
            let white_king = self.king_square(Colour::White);
            let black_king = self.king_square(Colour::Black);
            self.eval.add_piece(piece, square, colour, white_king, black_king);

            self.add_attacks(square, piece_index, piece);
            self.update_sliders(square, false, None);
            // fixup: add threats to new square
            for attack in self.bitlist[square] & !Bitlist::from_piece(piece_index) {
                self.eval.add_threat(
                    self.piece_from_bit(attack),
                    self.square_of_piece(attack),
                    Some(piece),
                    square,
                    attack.colour(),
                    Some(piece_index.colour()),
                    white_king,
                    black_king,
                );
            }
        }
    }

    /// Remove a piece from a square.
    pub fn remove_piece(&mut self, piece_index: PieceIndex, update: bool) {
        let square = self.square_of_piece(piece_index);
        let piece = self.piece_from_bit(piece_index);
        self.piecemask.remove_piece(piece_index);
        self.piecelist.remove_piece(piece_index, square);
        self.index.remove_piece(piece_index, square);
        let hash = match piece {
            Piece::Pawn => &mut self.hash_pawns,
            Piece::Knight => &mut self.hash_knights,
            Piece::Bishop => &mut self.hash_bishops,
            Piece::Rook => &mut self.hash_rooks,
            Piece::Queen => &mut self.hash_queens,
            Piece::King => &mut self.hash_kings,
        };
        Zobrist::remove_piece(piece_index.colour(), piece, square, hash);

        let white_king = self.king_square(Colour::White);
        let black_king = self.king_square(Colour::Black);
        self.eval.remove_piece(piece, square, piece_index.colour(), white_king, black_king);

        if update {
            self.remove_attacks(square, piece_index, piece);
            self.update_sliders(square, true, None);
            // fixup: clear threats to old square
            for attack in self.bitlist[square] & !Bitlist::from_piece(piece_index) {
                self.eval.remove_threat(
                    self.piece_from_bit(attack),
                    self.square_of_piece(attack),
                    Some(piece),
                    square,
                    attack.colour(),
                    Some(piece_index.colour()),
                    white_king,
                    black_king,
                );
            }
        }
    }

    pub fn rebuild_accumulators(&mut self) {
        let white_king = self.king_square(Colour::White);
        let black_king = self.king_square(Colour::Black);
        //println!("===");
        self.eval = Eval::new();
        for square in 0..64 {
            let square = unsafe { Square::from_u8_unchecked(square) };
            let Some(square_piece_index) = self.index[square] else { continue };

            for attack in self.bitlist[square] {
                self.eval.add_threat(
                    self.piece_from_bit(attack),
                    self.square_of_piece(attack),
                    Some(self.piece_from_bit(square_piece_index)),
                    square,
                    attack.colour(),
                    self.colour_from_square(square),
                    white_king,
                    black_king,
                );
            }

            self.eval.add_piece(
                self.piece_from_bit(square_piece_index),
                square,
                square_piece_index.colour(),
                white_king,
                black_king,
            );
        }
    }

    pub fn verify_accumulators(&self) -> bool {
        let mut clone = self.clone();
        clone.rebuild_accumulators();
        self.eval == clone.eval
    }

    fn move_piece_rebuild_accumulator(&mut self, from_square: Square, to_square: Square) {
        let piece_index = self.index[to_square].expect("attempted to move piece from empty square");
        let piece = self.piece_from_bit(piece_index);

        let white_king = self.king_square(Colour::White);
        let black_king = self.king_square(Colour::Black);

        // we need to rebuild the accumulator ;~;
        self.eval.reset_colour(piece_index.colour());
        for square in 0..64 {
            let square = unsafe { Square::from_u8_unchecked(square) };
            let Some(square_piece_index) = self.index[square] else { continue };

            for attack in self.bitlist[square] {
                self.eval.add_threat_for_acc(
                    self.piece_from_bit(attack),
                    self.square_of_piece(attack),
                    self.piece_from_bit(square_piece_index),
                    square,
                    attack.colour(),
                    self.colour_from_square(square),
                    white_king,
                    black_king,
                    piece_index.is_white(),
                );
            }
            self.eval.add_piece_for_acc(
                self.piece_from_bit(square_piece_index),
                square,
                square_piece_index.colour(),
                white_king,
                black_king,
                piece_index.is_white(),
            );
        }

        self.eval
            .remove_piece_for_acc(piece, from_square, piece_index.colour(), white_king, black_king, !piece_index.is_white());
        self.eval
            .add_piece_for_acc(piece, to_square, piece_index.colour(), white_king, black_king, !piece_index.is_white());
        // fixup: clear threats to old square
        for attack in self.bitlist[from_square] & !Bitlist::from_piece(piece_index) {
            self.eval.remove_threat_for_acc(
                self.piece_from_bit(attack),
                self.square_of_piece(attack),
                piece,
                from_square,
                attack.colour(),
                self.colour_from_square(to_square),
                white_king,
                black_king,
                !piece_index.is_white(),
            );
        }

        // fixup: add threats to new square
        for attack in self.bitlist[to_square] & !Bitlist::from_piece(piece_index) {
            self.eval.add_threat_for_acc(
                self.piece_from_bit(attack),
                self.square_of_piece(attack),
                piece,
                to_square,
                attack.colour(),
                self.colour_from_square(to_square),
                white_king,
                black_king,
                !piece_index.is_white(),
            );
        }
    }

    /// Move a piece from a square to another square.
    pub fn move_piece(&mut self, from_square: Square, to_square: Square) {
        let piece_index = self.index[from_square].expect("attempted to move piece from empty square");
        let piece = self.piece_from_bit(piece_index);

        self.remove_attacks(from_square, piece_index, piece);
        self.update_sliders(from_square, true, None);

        self.piecelist.move_piece(piece_index, to_square);
        self.index.move_piece(piece_index, from_square, to_square);
        let hash = match piece {
            Piece::Pawn => &mut self.hash_pawns,
            Piece::Knight => &mut self.hash_knights,
            Piece::Bishop => &mut self.hash_bishops,
            Piece::Rook => &mut self.hash_rooks,
            Piece::Queen => &mut self.hash_queens,
            Piece::King => &mut self.hash_kings,
        };
        Zobrist::move_piece(piece_index.colour(), piece, from_square, to_square, hash);

        self.add_attacks(to_square, piece_index, piece);
        self.update_sliders(to_square, false, Some(from_square));

        let white_king = self.king_square(Colour::White);
        let black_king = self.king_square(Colour::Black);

        let from_file = File::from(from_square);
        let to_file = File::from(to_square);
        if crate::board::eval::HORIZONTAL_MIRROR
            && piece == Piece::King
            && ((from_file >= File::E && to_file <= File::D) || (from_file <= File::D && to_file >= File::E))
        {
            self.move_piece_rebuild_accumulator(from_square, to_square);
            return;
        }

        self.eval
            .move_piece(piece, from_square, to_square, piece_index.colour(), white_king, black_king);

        // fixup: clear threats to old square
        for attack in self.bitlist[from_square] & !Bitlist::from_piece(piece_index) {
            self.eval.remove_threat(
                self.piece_from_bit(attack),
                self.square_of_piece(attack),
                Some(piece),
                from_square,
                attack.colour(),
                self.colour_from_square(to_square),
                white_king,
                black_king,
            );
        }

        // fixup: add threats to new square
        for attack in self.bitlist[to_square] & !Bitlist::from_piece(piece_index) {
            self.eval.add_threat(
                self.piece_from_bit(attack),
                self.square_of_piece(attack),
                Some(piece),
                to_square,
                attack.colour(),
                self.colour_from_square(to_square),
                white_king,
                black_king,
            );
        }

        debug_assert!(!self.bitlist[to_square].contains(piece_index.into()), "piece on {to_square} cannot attack itself");
    }

    /// Set the en-passant square.
    pub fn set_ep(&mut self, old: Option<Square>, new: Option<Square>) {
        Zobrist::set_ep(old, new, &mut self.hash_other);
    }

    /// Add castling rights.
    pub fn add_castling(&mut self, kind: usize) {
        Zobrist::add_castling(kind, &mut self.hash_other);
    }

    /// Remove castling rights.
    pub fn remove_castling(&mut self, kind: usize) {
        Zobrist::remove_castling(kind, &mut self.hash_other);
    }

    /// Toggle side to move.
    pub fn toggle_side(&mut self) {
        Zobrist::toggle_side(&mut self.hash_other);
    }

    /// Evaluation from the perspective of `colour`.
    pub fn eval(&self, colour: Colour) -> i32 {
        self.eval.get(self.piecemask().occupied().count_ones() as u8, colour)
    }

    /// Rebuild the attack set for the board.
    pub fn rebuild_attacks(&mut self) {
        for square in 0_u8..64 {
            // SAFETY: index is always in bounds.
            let index = unsafe { Square::from_u8_unchecked(square) };
            self.bitlist.clear(index);
        }

        for square in 0_u8..64 {
            // SAFETY: square is always in bounds.
            let square = unsafe { Square::from_u8_unchecked(square) };
            if let Some(bit) = self.index[square] {
                let piece = self.piece_from_bit(bit);
                self.add_attacks(square, bit, piece);
            }
        }
    }

    fn remove_attacks(&mut self, square: Square, bit: PieceIndex, piece: Piece) {
        let white_king = self.king_square(Colour::White);
        let black_king = self.king_square(Colour::Black);

        // SIMD versions of internal data structures.
        let mut bitlist = unsafe { std::mem::transmute::<BitlistArray, u32x64>(self.bitlist.clone()) };
        let index = unsafe { std::mem::transmute::<PieceIndexArray, u8x64>(self.index.clone()) };

        let bit_vector = u32x64::splat(1_u32 << bit.into_inner());

        let occupied = index.simd_ne(u8x64::splat(0));
        let mask_of_attacks = (bitlist & bit_vector).simd_ne(u32x64::splat(0)).cast::<i8>();
        let mut threats = (occupied & mask_of_attacks).to_bitmask();

        while threats != 0 {
            let dest = threats.trailing_zeros();
            let dest = unsafe { Square::from_u8_unchecked(dest as u8) };
            threats &= threats - 1;
            self.eval.remove_threat(
                piece,
                square,
                self.index[dest].and_then(|index| self.piecemask.piece(index)),
                dest,
                bit.colour(),
                self.index[dest].map(PieceIndex::colour),
                white_king,
                black_king,
            );
        }

        bitlist &= !bit_vector;
        self.bitlist = unsafe { std::mem::transmute::<u32x64, BitlistArray>(bitlist) };
    }

    /// Add attacks for a square.
    fn add_attacks(&mut self, square: Square, bit: PieceIndex, piece: Piece) {
        let white_king = self.king_square(Colour::White);
        let black_king = self.king_square(Colour::Black);

        let update = |bitlist: &mut BitlistArray, index: &PieceIndexArray, piecemask: &Piecemask, eval: &mut Eval, dest: Square| {
            debug_assert!(dest != square);
            bitlist.add_piece(dest, bit);
            eval.add_threat(
                piece,
                square,
                index[dest].and_then(|index| piecemask.piece(index)),
                dest,
                bit.colour(),
                index[dest].map(PieceIndex::colour),
                white_king,
                black_king,
            );
        };

        let slide =
            |bitlist: &mut BitlistArray, piecemask: &Piecemask, eval: &mut Eval, index: &PieceIndexArray, dir: Direction| {
                let mut sq = square.travel(dir);
                while let Some(square) = sq {
                    update(bitlist, index, piecemask, eval, square);
                    sq = square.travel(dir).filter(|_| index[square].is_none());
                }
            };

        let leap = |bitlist: &mut BitlistArray, piecemask: &Piecemask, eval: &mut Eval, index: &PieceIndexArray, dir: Direction| {
            if let Some(dest) = square.travel(dir) {
                update(bitlist, index, piecemask, eval, dest);
            }
        };

        debug_assert!(
            !self.bitlist[square].contains(bit.into()),
            "{:?} on {square} cannot attack itself",
            self.piece_from_square(square)
        );

        if piece == Piece::Pawn {
            if bit.is_white() {
                leap(&mut self.bitlist, &self.piecemask, &mut self.eval, &self.index, Direction::NorthEast);
                leap(&mut self.bitlist, &self.piecemask, &mut self.eval, &self.index, Direction::NorthWest);
            } else {
                leap(&mut self.bitlist, &self.piecemask, &mut self.eval, &self.index, Direction::SouthEast);
                leap(&mut self.bitlist, &self.piecemask, &mut self.eval, &self.index, Direction::SouthWest);
            }
            return;
        }
        if piece == Piece::Knight {
            const DIRS: [Direction; 8] = [
                Direction::NorthNorthEast,
                Direction::EastNorthEast,
                Direction::EastSouthEast,
                Direction::SouthSouthEast,
                Direction::SouthSouthWest,
                Direction::WestSouthWest,
                Direction::WestNorthWest,
                Direction::NorthNorthWest,
            ];
            for dir in DIRS {
                leap(&mut self.bitlist, &self.piecemask, &mut self.eval, &self.index, dir);
            }
            return;
        }
        if piece == Piece::King {
            const DIRS: [Direction; 8] = [
                Direction::North,
                Direction::NorthEast,
                Direction::East,
                Direction::SouthEast,
                Direction::South,
                Direction::SouthWest,
                Direction::West,
                Direction::NorthWest,
            ];
            for dir in DIRS {
                leap(&mut self.bitlist, &self.piecemask, &mut self.eval, &self.index, dir);
            }
            return;
        }
        if piece == Piece::Bishop || piece == Piece::Queen {
            slide(&mut self.bitlist, &self.piecemask, &mut self.eval, &self.index, Direction::NorthEast);
            slide(&mut self.bitlist, &self.piecemask, &mut self.eval, &self.index, Direction::SouthEast);
            slide(&mut self.bitlist, &self.piecemask, &mut self.eval, &self.index, Direction::SouthWest);
            slide(&mut self.bitlist, &self.piecemask, &mut self.eval, &self.index, Direction::NorthWest);
            if piece == Piece::Bishop {
                return;
            }
        }
        if piece == Piece::Rook || piece == Piece::Queen {
            slide(&mut self.bitlist, &self.piecemask, &mut self.eval, &self.index, Direction::North);
            slide(&mut self.bitlist, &self.piecemask, &mut self.eval, &self.index, Direction::East);
            slide(&mut self.bitlist, &self.piecemask, &mut self.eval, &self.index, Direction::South);
            slide(&mut self.bitlist, &self.piecemask, &mut self.eval, &self.index, Direction::West);
            return;
        }

        debug_assert!(
            !self.bitlist[square].contains(bit.into()),
            "{:?} on {square} cannot attack itself",
            self.piece_from_square(square)
        );
    }

    /// Extend or remove slider attacks to a square.
    fn update_sliders(&mut self, square: Square, add: bool, from_square: Option<Square>) {
        let white_king = self.king_square(Colour::White);
        let black_king = self.king_square(Colour::Black);
        let sliders = self.bitlist[square] & (self.piecemask.bishops() | self.piecemask.rooks() | self.piecemask.queens());

        let square16x8 = Square16x8::from_square(square);
        for piece in sliders {
            let attacker = Square16x8::from_square(self.square_of_piece(piece));
            let Some(direction) = attacker.direction(square16x8) else {
                continue;
            };
            for dest in square16x8.ray_attacks(direction) {
                if add {
                    self.bitlist.add_piece(dest, piece);
                    self.eval.add_threat(
                        self.piece_from_bit(piece),
                        self.square_of_piece(piece),
                        self.piece_from_square(dest),
                        dest,
                        piece.colour(),
                        self.colour_from_square(dest),
                        white_king,
                        black_king,
                    );
                } else {
                    let to_colour = from_square.map_or_else(
                        || self.colour_from_square(dest),
                        |from_square| {
                            if from_square == dest { self.colour_from_square(square) } else { self.colour_from_square(dest) }
                        },
                    );
                    let to_piece = from_square.map_or_else(
                        || self.piece_from_square(dest),
                        |from_square| {
                            if from_square == dest { self.piece_from_square(square) } else { self.piece_from_square(dest) }
                        },
                    );
                    self.bitlist.remove_piece(dest, piece);
                    self.eval.remove_threat(
                        self.piece_from_bit(piece),
                        self.square_of_piece(piece),
                        to_piece,
                        dest,
                        piece.colour(),
                        to_colour,
                        white_king,
                        black_king,
                    );
                }

                if self.index[dest].is_some() {
                    break;
                }
            }
        }
    }
}
