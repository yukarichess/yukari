use crate::{
    piece::Piece,
    square::{File, Rank, Square},
};
use std::fmt::Display;

#[derive(Copy, Clone, Default, PartialEq)]
pub struct Move {
    pub from: Square,
    pub dest: Square,
    pub kind: MoveType,
    pub prom: Option<Piece>,
}

impl Display for Move {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let from_file: u8 = b'a' + u8::from(File::from(self.from));
        let from_rank: u8 = b'1' + u8::from(Rank::from(self.from));
        let dest_file: u8 = b'a' + u8::from(File::from(self.dest));
        let dest_rank: u8 = b'1' + u8::from(Rank::from(self.dest));
        write!(
            f,
            "{}{}{}{}",
            from_file as char, from_rank as char, dest_file as char, dest_rank as char
        )?;

        if let Some(prom) = self.prom {
            static PROMOTE_CHAR: [char; 6] = ['p', 'n', 'b', 'r', 'q', 'k'];
            write!(f, "{}", PROMOTE_CHAR[prom as usize])?;
        }

        Ok(())
    }
}

impl Move {
    /// Create a new Move.
    #[must_use]
    pub const fn new(
        from: Square,
        dest: Square,
        kind: MoveType,
        promotion_piece: Option<Piece>,
    ) -> Self {
        //assert!(dest != from);
        Self {
            from,
            dest,
            kind,
            prom: promotion_piece,
        }
    }

    #[must_use]
    pub const fn is_capture(&self) -> bool {
        matches!(
            self.kind,
            MoveType::Capture | MoveType::CapturePromotion | MoveType::EnPassant
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MoveType {
    Normal,
    Capture,
    Castle,
    DoublePush,
    EnPassant,
    Promotion,
    CapturePromotion,
}

impl Default for MoveType {
    fn default() -> Self {
        Self::Normal
    }
}
