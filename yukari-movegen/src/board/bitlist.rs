use super::index::PieceIndex;
use crate::{colour::Colour, square::Square};
use std::{
    fmt::Debug,
    iter::FusedIterator,
    ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Index, Not},
};

/// A set of 32 bits, each representing a piece.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct Bitlist(u32);

impl Bitlist {
    /// Create a new, empty Bitlist.
    pub const fn new() -> Self {
        Self(0)
    }

    /// Create a mask of the white-piece bits.
    pub const fn white() -> Self {
        Self(0x0000_FFFF)
    }

    /// Create a mask of the black-piece bits.
    pub const fn black() -> Self {
        Self(0xFFFF_0000)
    }

    /// Count the number of set bits in a bitlist.
    pub const fn count_ones(self) -> u32 {
        self.0.count_ones()
    }

    /// Create a mask corresponding to the bits of a given colour.
    pub const fn mask_from_colour(colour: Colour) -> Self {
        match colour {
            Colour::White => Self::white(),
            Colour::Black => Self::black(),
        }
    }

    /// Returns true if this `Bitlist` contains `other`.
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }

    /// Returns true if this `Bitlist` is empty.
    pub const fn empty(self) -> bool {
        self.0 == 0
    }

    /// Return the lowest set bit of a `Bitlist` as a `PieceIndex`, if it exists.
    pub const fn peek(self) -> Option<PieceIndex> {
        if self.0 == 0 {
            return None;
        }
        #[allow(clippy::cast_possible_truncation)]
        let bit = self.0.trailing_zeros() as u8;
        unsafe { Some(PieceIndex::new_unchecked(bit)) }
    }

    /// Return the lowest set bit of a `Bitlist` as a `PieceIndex`.
    pub const unsafe fn peek_nonzero(self) -> PieceIndex {
        if self.0 == 0 {
            std::hint::unreachable_unchecked();
        }
        #[allow(clippy::cast_possible_truncation)]
        let bit = self.0.trailing_zeros() as u8;
        PieceIndex::new_unchecked(bit)
    }

    /// Return the lowest set bit of a `Bitlist` as a `PieceIndex`, if it exists, and clear that bit.
    pub fn pop(&mut self) -> Option<PieceIndex> {
        let bit = self.peek()?;
        self.0 &= self.0.wrapping_sub(1);
        Some(bit)
    }

    // TODO: remove when traits can have const impls.
    pub const fn from_piece(index: PieceIndex) -> Self {
        Self(1_u32 << index.into_inner())
    }

    // TODO: remove when traits can have const impls.
    pub const fn and(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }

    // TODO: remove when traits can have const impls.
    pub const fn or(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }

    // TODO: remove when traits can have const impls.
    pub const fn invert(self) -> Self {
        Self(!self.0)
    }
}

impl From<PieceIndex> for Bitlist {
    fn from(index: PieceIndex) -> Self {
        Self(1_u32 << index.into_inner())
    }
}

impl From<u32> for Bitlist {
    fn from(index: u32) -> Self {
        Self(index)
    }
}

impl BitAnd for Bitlist {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl BitAndAssign for Bitlist {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl BitOr for Bitlist {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for Bitlist {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl Not for Bitlist {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self(!self.0)
    }
}

impl IntoIterator for Bitlist {
    type Item = PieceIndex;
    type IntoIter = BitlistIter;

    fn into_iter(self) -> Self::IntoIter {
        BitlistIter(self)
    }
}

/// Iterate over a `Bitlist`.
#[allow(clippy::module_name_repetitions)]
#[repr(transparent)]
pub struct BitlistIter(Bitlist);

impl Iterator for BitlistIter {
    type Item = PieceIndex;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.pop()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (
            self.0.count_ones() as usize,
            Some(self.0.count_ones() as usize),
        )
    }
}

impl ExactSizeIterator for BitlistIter {}
impl FusedIterator for BitlistIter {}

/// The main attack table array.
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, PartialEq, Eq)]
#[repr(transparent)]
pub struct BitlistArray([Bitlist; 64]);

impl Debug for BitlistArray {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for square in 0_u8..64 {
            // SAFETY: square is always in bounds from the for loop.
            let square = unsafe { Square::from_u8_unchecked(square) };

            if !self[square].empty() {
                writeln!(f, "{}: {:08x}", square, self[square].0)?;
            }
        }
        Ok(())
    }
}

impl Index<Square> for BitlistArray {
    type Output = Bitlist;

    fn index(&self, index: Square) -> &Self::Output {
        // The valid range of a `Square` is 0-63, matching the internal array, so this never panics.
        #[allow(clippy::indexing_slicing)]
        let result = &self.0[usize::from(index.into_inner())];
        result
    }
}

impl BitlistArray {
    /// Create a `BitlistArray`.
    pub const fn new() -> Self {
        Self([Bitlist::new(); 64])
    }

    /// Remove all attacks to a square.
    pub fn clear(&mut self, index: Square) {
        self.0[usize::from(index.into_inner())] = Bitlist::new();
    }

    /// Add an attack to a square.
    pub fn add_piece(&mut self, square: Square, piece: PieceIndex) {
        let index = usize::from(square.into_inner());
        let piece = Bitlist::from(piece);
        debug_assert!(
            !self.0[index].contains(piece),
            "attempted to add pre-existing piece attack on {}",
            square
        );
        self.0[index] |= piece;
    }

    /// Remove an attack from a square.
    pub fn remove_piece(&mut self, square: Square, piece: PieceIndex) {
        let index = usize::from(square.into_inner());
        let piece = Bitlist::from(piece);
        debug_assert!(
            self.0[index].contains(piece),
            "attempted to remove nonexistent piece attack on {}",
            square
        );
        self.0[index] &= !piece;
    }
}
