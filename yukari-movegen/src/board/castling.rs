use crate::{
    chessmove::MoveType,
    colour::Colour,
    square::{File, Rank, Square},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CastlingRights {
    availability:   u8,
    castling_files: [Square; 4],
}

impl CastlingRights {
    pub const fn new() -> Self {
        Self { availability: 0, castling_files: [Square::H1, Square::A1, Square::H8, Square::A8] }
    }

    pub const fn kingside_index(colour: Colour) -> usize {
        2 * colour as usize
    }

    pub const fn queenside_index(colour: Colour) -> usize {
        (2 * colour as usize) + 1
    }

    pub const fn back_rank(colour: Colour) -> Rank {
        match colour {
            Colour::White => Rank::One,
            Colour::Black => Rank::Eight,
        }
    }

    pub fn castling_destinations(colour: Colour, kind: MoveType) -> (Square, Square) {
        let back = Self::back_rank(colour);
        if kind == MoveType::KingsideCastle {
            (Square::from_rank_file(back, File::G), Square::from_rank_file(back, File::F))
        } else {
            (Square::from_rank_file(back, File::C), Square::from_rank_file(back, File::D))
        }
    }

    pub const fn availability(self) -> u8 {
        self.availability
    }

    pub const fn set(&mut self, index: usize, square: Square) {
        self.availability |= 1 << index;
        self.castling_files[index] = square;
    }

    pub const fn rook_square(self, index: usize) -> Option<Square> {
        if self.availability & (1 << index) == 0 { None } else { Some(self.castling_files[index]) }
    }

    pub const fn king_moved(&mut self, colour: Colour) {
        self.availability &= !(0b11 << (2 * colour as usize));
    }

    pub fn update_square(&mut self, square: Square, colour: Colour) {
        let mut mask_update = 0;
        for index in [Self::kingside_index(colour), Self::queenside_index(colour)] {
            if self.castling_files[index] == square {
                mask_update |= 1 << index;
            }
        }
        self.availability &= !mask_update;
    }
}
