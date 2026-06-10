use crate::{
    chessmove::MoveType,
    colour::Colour,
    square::{File, Rank, Square},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CastlingRights {
    raw: u16,
}

impl CastlingRights {
    pub const fn new() -> Self {
        Self { raw: 0 }
    }

    pub const fn kingside_index(colour: Colour) -> usize {
        2 * colour as usize
    }

    pub const fn queenside_index(colour: Colour) -> usize {
        2 * colour as usize + 1
    }

    pub const fn back_rank(colour: Colour) -> Rank {
        match colour {
            Colour::White => Rank::One,
            Colour::Black => Rank::Eight,
        }
    }

    pub fn castling_destinations(colour: Colour, kind: MoveType) -> (Square, Square) {
        let back = Self::back_rank(colour);
        if matches!(kind, MoveType::KingsideCastle) {
            (Square::from_rank_file(back, File::G), Square::from_rank_file(back, File::F))
        } else {
            (Square::from_rank_file(back, File::C), Square::from_rank_file(back, File::D))
        }
    }

    pub const fn availability(self) -> u8 {
        (self.raw & 0xF) as u8
    }

    pub fn set(&mut self, index: usize, file: File) {
        let shift = 4 + 3 * index;
        self.raw &= !(0b111 << shift);
        self.raw |= (u16::from(u8::from(file)) << shift) | (1 << index);
    }

    pub fn rook_square(self, index: usize) -> Option<Square> {
        if self.raw & (1 << index) == 0 {
            return None;
        }
        let file = File::try_from(((self.raw >> (4 + 3 * index)) & 0b111) as u8).unwrap();
        let rank = if index < 2 { Rank::One } else { Rank::Eight };
        Some(Square::from_rank_file(rank, file))
    }

    pub const fn king_moved(&mut self, colour: Colour) {
        self.raw &= !(0b11 << (2 * colour as usize));
    }

    pub fn update_square(&mut self, square: Square, colour: Colour) {
        for index in [Self::kingside_index(colour), Self::queenside_index(colour)] {
            if self.rook_square(index) == Some(square) {
                self.raw &= !(1 << index);
            }
        }
    }
}
