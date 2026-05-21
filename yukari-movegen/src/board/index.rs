use std::{
    convert::TryFrom,
    num::NonZeroU8,
    ops::{Index, IndexMut},
};

use crate::{
    Piece,
    board::{
        bitlist::BitlistArray,
        piecemask::Piecemask,
    },
    colour::Colour,
    square::Square,
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
        Self(unsafe { NonZeroU8::new_unchecked(x + 1) })
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
        if self.is_white() { Colour::White } else { Colour::Black }
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
        if index.is_white() { Self::White } else { Self::Black }
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
        debug_assert!(self[square].is_none(), "attempted to add piece to occupied square");
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

    /// Move a piece from `from_square` to `dest_square`
    pub fn move_piece(&mut self, piece_index: PieceIndex, from_square: Square, dest_square: Square) {
        self[from_square] = None;
        self[dest_square] = Some(piece_index);
    }

    /// Remap the array into ray-space for `square`.
    pub fn to_rays(&self, square: Square) -> PieceIndexRays {
        let mut rays = PieceIndexRays([None; 64]);
        let perm = square.ray_perm();

        for square in 0..64 {
            if let Some(perm_square) = perm[square] {
                rays.0[square] = self[perm_square];
            }
        }

        rays
    }

    pub fn to_bitlist_array(&self) -> BitlistArray {
        let mut array = BitlistArray::new();
        for square in 0..64 {
            let square = unsafe { Square::from_u8_unchecked(square) };
            if let Some(piece) = self[square] {
                array.add_piece(square, piece);
            }
        }
        array
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

/// A `Square` -> `PieceIndex` mapping in ray-space.
#[derive(Clone)]
#[repr(transparent)]
pub struct PieceIndexRays([Option<PieceIndex>; 64]);

impl Index<Square> for PieceIndexRays {
    type Output = Option<PieceIndex>;

    fn index(&self, index: Square) -> &Self::Output {
        &self.0[usize::from(index.into_inner())]
    }
}

impl IndexMut<Square> for PieceIndexRays {
    fn index_mut(&mut self, index: Square) -> &mut Self::Output {
        &mut self.0[usize::from(index.into_inner())]
    }
}

const HORSE: u8 = 0b000_0100;
const ORTH: u8 = 0b011_0000;
const DIAG: u8 = 0b010_1000;
const ORTH_NEAR: u8 = 0b111_0000;
const WPAWN_NEAR: u8 = 0b110_1001;
const BPAWN_NEAR: u8 = 0b110_1010;

static ATTACKER_LUT: [u8; 64] = [
    HORSE, ORTH_NEAR, ORTH, ORTH, ORTH, ORTH, ORTH, ORTH, // N
    HORSE, BPAWN_NEAR, DIAG, DIAG, DIAG, DIAG, DIAG, DIAG, // NE
    HORSE, ORTH_NEAR, ORTH, ORTH, ORTH, ORTH, ORTH, ORTH, // E
    HORSE, WPAWN_NEAR, DIAG, DIAG, DIAG, DIAG, DIAG, DIAG, // SE
    HORSE, ORTH_NEAR, ORTH, ORTH, ORTH, ORTH, ORTH, ORTH, // S
    HORSE, WPAWN_NEAR, DIAG, DIAG, DIAG, DIAG, DIAG, DIAG, // SW
    HORSE, ORTH_NEAR, ORTH, ORTH, ORTH, ORTH, ORTH, ORTH, // W
    HORSE, BPAWN_NEAR, DIAG, DIAG, DIAG, DIAG, DIAG, DIAG, // NW
];

static SLIDER_LUT: [u8; 64] = [
    0, ORTH, ORTH, ORTH, ORTH, ORTH, ORTH, ORTH, // N
    0, DIAG, DIAG, DIAG, DIAG, DIAG, DIAG, DIAG, // NE
    0, ORTH, ORTH, ORTH, ORTH, ORTH, ORTH, ORTH, // E
    0, DIAG, DIAG, DIAG, DIAG, DIAG, DIAG, DIAG, // SE
    0, ORTH, ORTH, ORTH, ORTH, ORTH, ORTH, ORTH, // S
    0, DIAG, DIAG, DIAG, DIAG, DIAG, DIAG, DIAG, // SW
    0, ORTH, ORTH, ORTH, ORTH, ORTH, ORTH, ORTH, // W
    0, DIAG, DIAG, DIAG, DIAG, DIAG, DIAG, DIAG, // NW
];

impl PieceIndexRays {
    pub fn to_piece_rays(&self, piecemasks: &Piecemask) -> PieceRays {
        let mut rays = PieceRays([None; 64]);

        for square in 0..64 {
            if let Some(index) = self.0[square] {
                rays.0[square] = piecemasks.piece(index);
            }
        }

        rays
    }

    pub fn attackers(&self, piecemasks: &Piecemask) -> RayMask {
        let mut mask = RayMask(0);
        for square in 0..64 {
            let Some(index) = self.0[square] else { continue };
            let Some(piece) = piecemasks.piece(index) else { unreachable!() };
            let colour = index.colour();
            let piece = piece as usize + if piece == Piece::Pawn { colour as usize } else { 1 };
            let piece = 1 << piece;
            let valid = (ATTACKER_LUT[square] & piece) == piece;
            mask.0 |= u64::from(valid) << square;
        }
        mask
    }

    pub fn sliders(&self, piecemasks: &Piecemask) -> RayMask {
        let mut mask = RayMask(0);
        for square in 0..64 {
            let Some(index) = self.0[square] else { continue };
            let Some(piece) = piecemasks.piece(index) else { unreachable!() };
            let colour = index.colour();
            let piece = piece as usize + if piece == Piece::Pawn { colour as usize } else { 1 };
            let piece = 1 << piece;
            let valid = (SLIDER_LUT[square] & piece) == piece;
            mask.0 |= u64::from(valid) << square;
        }
        mask
    }

    pub fn occupied(&self) -> RayMask {
        let mut mask = RayMask(0);
        for square in 0..64 {
            if self.0[square].is_some() {
                mask.0 |= 1_u64 << square;
            }
        }
        mask
    }

    pub fn pieces_of_colour(&self, colour: Colour) -> RayMask {
        let mut mask = RayMask(0);
        for square in 0..64 {
            if let Some(index) = self.0[square]
                && index.colour() == colour
            {
                mask.0 |= 1_u64 << square;
            }
        }
        mask
    }

    pub fn mask(&mut self, mask: RayMask) {
        for square in 0..64 {
            if !mask.nth(square) {
                self.0[square] = None;
            }
        }
    }

    pub fn ray_broadcast(&mut self) {
        for ray in 0..8 {
            let mut possible_index = None;
            for hop in 1..8 {
                if let Some(index) = self.0[8 * ray + hop] {
                    possible_index = Some(index);
                    break;
                }
            }
            for hop in 1..8 {
                self.0[8 * ray + hop] = possible_index;
            }
        }
    }

    pub fn rotate_180(&mut self) {
        for square in 0..32 {
            self.0.swap(square, 32 + square);
        }
    }

    pub fn to_mailbox(&self, square: Square) -> PieceIndexArray {
        let mut mailbox = PieceIndexArray([None; 64]);
        let bperm = square.ray_bperm();

        for square in 0..64 {
            if let Some(bperm_square) = bperm[square] {
                mailbox.0[square] = self[bperm_square];
            }
        }

        mailbox
    }
}

/// A `Square` -> `Piece` mapping in ray-space.
#[derive(Clone)]
#[repr(transparent)]
pub struct PieceRays([Option<Piece>; 64]);

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct RayMask(u64);

impl RayMask {
    pub const fn mask_to_nearest(self) -> Self {
        let o = self.0 | 0x8181_8181_8181_8181;
        Self(o ^ (o - 0x0303_0303_0303_0303))
    }

    pub const fn nearest(self) -> Self {
        Self(self.0 & self.mask_to_nearest().0)
    }

    pub const fn nth(self, index: usize) -> bool {
        (self.0 & (1_u64 << index)) != 0
    }
}
