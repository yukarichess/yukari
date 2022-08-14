use super::{
    bitlist::{Bitlist, BitlistArray},
    index::{PieceIndex, PieceIndexArray},
    piecelist::Piecelist,
    piecemask::Piecemask,
};
use crate::{
    colour::Colour,
    piece::Piece,
    square::{Direction, Square, Square16x8},
};

#[allow(clippy::module_name_repetitions)]
#[derive(Clone)]
pub struct BoardData {
    bitlist: BitlistArray,
    piecelist: Piecelist,
    index: PieceIndexArray,
    piecemask: Piecemask,
}

impl BoardData {
    /// Create a new board.
    pub const fn new() -> Self {
        Self {
            bitlist: BitlistArray::new(),
            piecelist: Piecelist::new(),
            index: PieceIndexArray::new(),
            piecemask: Piecemask::new(),
        }
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

    /// Return a bitlist of all pawns.
    pub const fn pawns(&self) -> Bitlist {
        self.piecemask.pawns()
    }

    /// Return a bitlist of all knights.
    pub const fn knights(&self) -> Bitlist {
        self.piecemask.knights()
    }

    /// Return a bitlist of all bishops.
    pub const fn bishops(&self) -> Bitlist {
        self.piecemask.bishops()
    }

    /// Return a bitlist of all rooks.
    pub const fn rooks(&self) -> Bitlist {
        self.piecemask.rooks()
    }

    /// Return a bitlist of all queens.
    pub const fn queens(&self) -> Bitlist {
        self.piecemask.queens()
    }

    /// Return a bitlist of all kings.
    pub const fn kings(&self) -> Bitlist {
        self.piecemask.kings()
    }

    /// Return a bitlist of all pieces.
    pub const fn pieces(&self) -> Bitlist {
        self.piecemask.occupied()
    }

    /// Return a bitlist of all pieces of a given colour.
    pub const fn pieces_of_colour(&self, colour: Colour) -> Bitlist {
        self.piecemask.pieces_of_colour(colour)
    }

    /// Return the square of the king of a given colour.
    pub fn king_square(&self, colour: Colour) -> Square {
        let king_index =
            unsafe { (self.kings() & Bitlist::mask_from_colour(colour)).peek_nonzero() };
        self.square_of_piece(king_index)
    }

    /// Given a piece index, return its piece type.
    pub fn piece_from_bit(&self, bit: PieceIndex) -> Piece {
        self.piecemask
            .piece(bit)
            .expect("piece index corresponds to invalid piece")
    }

    /// Given a square, return the piece type of it, if any.
    pub fn piece_from_square(&self, square: Square) -> Option<Piece> {
        self.piecemask.piece(self.index[square]?)
    }

    /// Given a square, return the colour of the piece on it, if any.
    pub fn colour_from_square(&self, square: Square) -> Option<Colour> {
        Some(Colour::from(self.index[square]?))
    }

    /// Add a `Piece` to a `Square`.
    pub fn add_piece(&mut self, piece: Piece, colour: Colour, square: Square, update: bool) {
        let piece_index = self.piecemask.add_piece(piece, colour);
        self.piecelist.add_piece(piece_index, square);
        self.index.add_piece(piece_index, square);

        if update {
            self.update_attacks(square, piece_index, piece, true, None);
            self.update_sliders(square, false);
        }
    }

    /// Remove a piece from a square.
    pub fn remove_piece(&mut self, piece_index: PieceIndex, update: bool) {
        let square = self.square_of_piece(piece_index);
        let piece = self.piece_from_bit(piece_index);
        self.piecemask.remove_piece(piece_index);
        self.piecelist.remove_piece(piece_index, square);
        self.index.remove_piece(piece_index, square);

        if update {
            self.update_attacks(square, piece_index, piece, false, None);
            self.update_sliders(square, true);
        }
    }

    /// Move a piece from a square to another square.
    pub fn move_piece(&mut self, from_square: Square, to_square: Square) {
        let piece_index =
            self.index[from_square].expect("attempted to move piece from empty square");
        let piece = self.piece_from_bit(piece_index);
        let slide_dir = from_square.direction(to_square).and_then(|dir| {
            if matches!(piece, Piece::Bishop | Piece::Rook | Piece::Queen) {
                Some(dir)
            } else {
                None
            }
        });

        self.update_attacks(from_square, piece_index, piece, false, slide_dir);
        self.update_sliders(from_square, true);
        if slide_dir.is_some() {
            self.bitlist.add_piece(from_square, piece_index);
        }

        self.piecelist.move_piece(piece_index, to_square);
        self.index.move_piece(piece_index, from_square, to_square);

        if slide_dir.is_some() {
            self.bitlist.remove_piece(to_square, piece_index);
        }
        self.update_attacks(to_square, piece_index, piece, true, slide_dir);
        self.update_sliders(to_square, false);

        debug_assert!(
            !self.bitlist[to_square].contains(piece_index.into()),
            "piece on {} cannot attack itself",
            to_square
        );
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
                self.update_attacks(square, bit, piece, true, None);
            }
        }
    }

    /// Add or remove attacks for a square.
    fn update_attacks(
        &mut self,
        square: Square,
        bit: PieceIndex,
        piece: Piece,
        add: bool,
        skip_dir: Option<Direction>,
    ) {
        let update = |bitlist: &mut BitlistArray, dest: Square| {
            if add {
                debug_assert!(dest != square);
                bitlist.add_piece(dest, bit);
            } else {
                bitlist.remove_piece(dest, bit);
            }
        };

        let slide = |bitlist: &mut BitlistArray, index: &PieceIndexArray, dir: Direction| {
            if let Some(skip_dir) = skip_dir {
                if skip_dir == dir || skip_dir == dir.opposite() {
                    return;
                }
            }

            let mut sq = square.travel(dir);

            let mut iters = 0;
            while let Some(square) = sq {
                update(bitlist, square);
                sq = square.travel(dir).filter(|_| index[square].is_none());
                iters += 1;
                if iters > 6 {
                    break;
                }
            }
        };

        let leap = |b: &mut BitlistArray, dir: Direction| {
            if let Some(dest) = square.travel(dir) {
                update(b, dest);
            }
        };

        debug_assert!(
            !self.bitlist[square].contains(bit.into()),
            "{:?} on {} cannot attack itself",
            self.piece_from_square(square),
            square
        );

        match piece {
            Piece::Pawn => {
                if bit.is_white() {
                    leap(&mut self.bitlist, Direction::NorthEast);
                    leap(&mut self.bitlist, Direction::NorthWest);
                } else {
                    leap(&mut self.bitlist, Direction::SouthEast);
                    leap(&mut self.bitlist, Direction::SouthWest);
                }
            }
            Piece::Knight => {
                leap(&mut self.bitlist, Direction::NorthNorthEast);
                leap(&mut self.bitlist, Direction::EastNorthEast);
                leap(&mut self.bitlist, Direction::EastSouthEast);
                leap(&mut self.bitlist, Direction::SouthSouthEast);
                leap(&mut self.bitlist, Direction::SouthSouthWest);
                leap(&mut self.bitlist, Direction::WestSouthWest);
                leap(&mut self.bitlist, Direction::WestNorthWest);
                leap(&mut self.bitlist, Direction::NorthNorthWest);
            }
            Piece::King => {
                leap(&mut self.bitlist, Direction::North);
                leap(&mut self.bitlist, Direction::NorthEast);
                leap(&mut self.bitlist, Direction::East);
                leap(&mut self.bitlist, Direction::SouthEast);
                leap(&mut self.bitlist, Direction::South);
                leap(&mut self.bitlist, Direction::SouthWest);
                leap(&mut self.bitlist, Direction::West);
                leap(&mut self.bitlist, Direction::NorthWest);
            }
            Piece::Bishop => {
                slide(&mut self.bitlist, &self.index, Direction::NorthEast);
                slide(&mut self.bitlist, &self.index, Direction::SouthEast);
                slide(&mut self.bitlist, &self.index, Direction::SouthWest);
                slide(&mut self.bitlist, &self.index, Direction::NorthWest);
            }
            Piece::Rook => {
                slide(&mut self.bitlist, &self.index, Direction::North);
                slide(&mut self.bitlist, &self.index, Direction::East);
                slide(&mut self.bitlist, &self.index, Direction::South);
                slide(&mut self.bitlist, &self.index, Direction::West);
            }
            Piece::Queen => {
                slide(&mut self.bitlist, &self.index, Direction::North);
                slide(&mut self.bitlist, &self.index, Direction::East);
                slide(&mut self.bitlist, &self.index, Direction::South);
                slide(&mut self.bitlist, &self.index, Direction::West);
                slide(&mut self.bitlist, &self.index, Direction::NorthEast);
                slide(&mut self.bitlist, &self.index, Direction::SouthEast);
                slide(&mut self.bitlist, &self.index, Direction::SouthWest);
                slide(&mut self.bitlist, &self.index, Direction::NorthWest);
            }
        }

        debug_assert!(
            !self.bitlist[square].contains(bit.into()),
            "{:?} on {} cannot attack itself",
            self.piece_from_square(square),
            square
        );
    }

    /// Extend or remove slider attacks to a square.
    fn update_sliders(&mut self, square: Square, add: bool) {
        let sliders = self.bitlist[square]
            & (self.piecemask.bishops() | self.piecemask.rooks() | self.piecemask.queens());

        let square = Square16x8::from_square(square);
        for piece in sliders {
            let attacker = Square16x8::from_square(self.square_of_piece(piece));
            if let Some(direction) = attacker.direction(square) {
                for dest in square.ray_attacks(direction) {
                    if add {
                        self.bitlist.add_piece(dest, piece);
                    } else {
                        self.bitlist.remove_piece(dest, piece);
                    }

                    if self.index[dest].is_some() {
                        break;
                    }
                }
            }
        }
    }
}
