use std::ops::Not;

/// A piece colour.
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Colour {
    /// White pieces.
    White,
    /// Black pieces.
    Black,
}

impl From<Colour> for usize {
    #[inline]
    fn from(colour: Colour) -> Self {
        match colour {
            Colour::White => 0,
            Colour::Black => 1,
        }
    }
}

impl Not for Colour {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            Self::White => Self::Black,
            Self::Black => Self::White,
        }
    }
}
