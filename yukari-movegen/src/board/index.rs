use crate::{colour::Colour, square::Square};
use std::{
    convert::TryFrom,
    num::NonZeroU8,
    ops::{Index, IndexMut},
};

#[allow(clippy::module_name_repetitions)]
#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
#[repr(transparent)]
pub struct PieceIndex(NonZeroU8);

impl PieceIndex {
    /// # Safety
    /// `x` must be in the range 0-31.
    #[must_use]
    pub const unsafe fn new_unchecked(x: u8) -> Self {
        Self(NonZeroU8::new_unchecked(x + 1))
    }

    #[must_use]
    pub const fn into_inner(self) -> u8 {
        (self.0.get() - 1) & 31
    }

    #[must_use]
    pub const fn is_white(self) -> bool {
        self.into_inner() <= 15
    }

    #[must_use]
    pub const fn is_black(self) -> bool {
        self.into_inner() >= 16
    }

    #[must_use]
    pub const fn colour(self) -> Colour {
        if self.is_white() {
            Colour::White
        } else {
            Colour::Black
        }
    }
}

impl TryFrom<u8> for PieceIndex {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if value > 31 {
            return Err(());
        }

        // SAFETY: value + 1 is always non-zero.
        Ok(Self(unsafe { NonZeroU8::new_unchecked(value + 1) }))
    }
}

impl From<PieceIndex> for Colour {
    fn from(index: PieceIndex) -> Self {
        if index.is_white() {
            Self::White
        } else {
            Self::Black
        }
    }
}

/// A `Square` -> `PieceIndex` mapping.
#[derive(Clone)]
#[repr(transparent)]
pub struct PieceIndexArray([Option<PieceIndex>; 64]);

impl PieceIndexArray {
    /// Create a new `PieceIndexArray`.
    pub const fn new() -> Self {
        Self([None; 64])
    }

    /// Add a `PieceIndex` to a `Square`. Panics if the square is occupied.
    pub fn add_piece(&mut self, piece_index: PieceIndex, square: Square) {
        debug_assert!(
            self[square].is_none(),
            "attempted to add piece to occupied square"
        );
        self[square] = Some(piece_index);
    }

    /// Remove a `PieceIndex` from a `Square`. Panics if the square is empty or contains a different `PieceIndex`.
    pub fn remove_piece(&mut self, _piece_index: PieceIndex, square: Square) {
        self[square] = None;
        /*match self[square] {
            None => panic!("attempted to remove piece from empty square"),
            Some(square_index) => {
                debug_assert!(
                    square_index == piece_index,
                    "attempted to remove wrong piece from square"
                );
                self[square] = None;
            }
        }*/
    }

    /// Move a piece from
    pub fn move_piece(
        &mut self,
        piece_index: PieceIndex,
        from_square: Square,
        dest_square: Square,
    ) {
        self[from_square] = None;
        self[dest_square] = Some(piece_index);
    }
}

impl Index<Square> for PieceIndexArray {
    type Output = Option<PieceIndex>;

    fn index(&self, index: Square) -> &Self::Output {
        &self.0[usize::from(index.into_inner())]
    }
}

impl IndexMut<Square> for PieceIndexArray {
    fn index_mut(&mut self, index: Square) -> &mut Self::Output {
        &mut self.0[usize::from(index.into_inner())]
    }
}
