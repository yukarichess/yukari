use std::{fmt::{Debug, Display}, num::NonZeroU16};

use crate::{
    piece::Piece,
    square::{File, Rank, Square},
};

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Move(NonZeroU16);

const _NICHE_OPTIMISED: () = assert!(std::mem::size_of::<Move>() == std::mem::size_of::<Option<Move>>());

impl Display for Move {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let from_file: u8 = b'a' + u8::from(File::from(self.from()));
        let from_rank: u8 = b'1' + u8::from(Rank::from(self.from()));
        let dest_file: u8 = b'a' + u8::from(File::from(self.dest()));
        let dest_rank: u8 = b'1' + u8::from(Rank::from(self.dest()));
        write!(f, "{}{}{}{}", from_file as char, from_rank as char, dest_file as char, dest_rank as char)?;

        if let Some(prom) = self.promotion_piece() {
            static PROMOTE_CHAR: [char; 6] = ['p', 'n', 'b', 'r', 'q', 'k'];
            write!(f, "{}", PROMOTE_CHAR[prom as usize])?;
        }

        Ok(())
    }
}

impl Debug for Move {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let from_file: u8 = b'a' + u8::from(File::from(self.from()));
        let from_rank: u8 = b'1' + u8::from(Rank::from(self.from()));
        let dest_file: u8 = b'a' + u8::from(File::from(self.dest()));
        let dest_rank: u8 = b'1' + u8::from(Rank::from(self.dest()));
        write!(f, "{}{}{}{}", from_file as char, from_rank as char, dest_file as char, dest_rank as char)?;

        if let Some(prom) = self.promotion_piece() {
            static PROMOTE_CHAR: [char; 6] = ['p', 'n', 'b', 'r', 'q', 'k'];
            write!(f, "{}", PROMOTE_CHAR[prom as usize])?;
        }

        Ok(())
    }
}

impl Default for Move {
    fn default() -> Self {
        Self(NonZeroU16::new(0xFFFF).unwrap())
    }
}

impl Move {
    /// Create a new Move.
    #[must_use]
    pub const fn new(from: Square, dest: Square, kind: MoveType) -> Self {
        let from = from.into_inner() as u16;
        let dest = dest.into_inner() as u16;
        let kind = kind as u16;
        let value = (kind << 12) | (dest << 6) | from;
        Self(NonZeroU16::new(value).unwrap())
    }

    #[must_use]
    pub const fn is_capture(&self) -> bool {
        self.kind().is_capture()
    }

    #[must_use]
    pub const fn is_promotion(&self) -> bool {
        self.kind().is_promotion()
    }

    #[must_use]
    pub const fn promotion_piece(&self) -> Option<Piece> {
        self.kind().promotion_piece()
    }

    #[must_use]
    pub const fn from(self) -> Square {
        unsafe { Square::from_u8_unchecked((self.0.get() & 0x003F) as u8) }
    }

    #[must_use]
    pub const fn dest(self) -> Square {
        unsafe { Square::from_u8_unchecked(((self.0.get() & 0x0FC0) >> 6) as u8) }
    }

    #[must_use]
    pub const fn kind(self) -> MoveType {
        unsafe { std::mem::transmute::<u8, MoveType>(((self.0.get() & 0xF000) >> 12) as u8) }
    }
}

// [[CITE]]: https://87flowers.com/chess-moveflags/
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MoveType {
    Normal,
    DoublePush,
    QueensideCastle,
    KingsideCastle,
    PromotionKnight,
    PromotionBishop,
    PromotionRook,
    PromotionQueen,
    Capture,
    EnPassant,
    _Unused1,
    _Unused2,
    CapturePromotionKnight,
    CapturePromotionBishop,
    CapturePromotionRook,
    CapturePromotionQueen
}

impl MoveType {
    #[must_use]
    pub const fn is_capture(self) -> bool {
        let this = self as u8;
        (this & 0b1000) != 0
    }

    #[must_use]
    pub const fn is_promotion(self) -> bool {
        let this = self as u8;
        (this & 0b0100) != 0
    }

    #[must_use]
    pub const fn promotion_piece(self) -> Option<Piece> {
        match self {
            MoveType::Normal => None,
            MoveType::DoublePush => None,
            MoveType::QueensideCastle => None,
            MoveType::KingsideCastle => None,
            MoveType::PromotionKnight => Some(Piece::Knight),
            MoveType::PromotionBishop => Some(Piece::Bishop),
            MoveType::PromotionRook => Some(Piece::Rook),
            MoveType::PromotionQueen => Some(Piece::Queen),
            MoveType::Capture => None,
            MoveType::EnPassant => None,
            MoveType::_Unused1 => None,
            MoveType::_Unused2 => None,
            MoveType::CapturePromotionKnight => Some(Piece::Knight),
            MoveType::CapturePromotionBishop => Some(Piece::Bishop),
            MoveType::CapturePromotionRook => Some(Piece::Rook),
            MoveType::CapturePromotionQueen => Some(Piece::Queen),
        }
    }
}

impl Default for MoveType {
    fn default() -> Self {
        Self::Normal
    }
}
