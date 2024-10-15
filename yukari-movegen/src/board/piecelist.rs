use super::index::PieceIndex;
use crate::square::Square;

/// A mapping from `PieceIndex` to `Square`.
#[derive(Clone)]
#[repr(transparent)]
pub struct Piecelist([Option<Square>; 32]);

impl Piecelist {
    /// Create a new `Piecelist`.
    pub const fn new() -> Self {
        Self([None; 32])
    }

    /// Get the square associated with a piece.
    ///
    /// Panics if `piece_index` does not have a square, since `PieceIndex` implies a valid piece.
    pub fn get(&self, piece_index: PieceIndex) -> Square {
        let piece_index = usize::from(piece_index.into_inner());
        self.0[piece_index].unwrap_or(unsafe { Square::from_u8_unchecked(0) })
        //self.0[piece_index].expect("valid piece index has invalid square")
    }

    /// Add a piece to the board.
    ///
    /// Panics if `piece_index` has a valid square.
    pub fn add_piece(&mut self, piece_index: PieceIndex, square: Square) {
        let piece_index = usize::from(piece_index.into_inner());
        debug_assert!(
            self.0[piece_index].is_none(),
            "attempted to add piece to occupied piece index {piece_index:?}"
        );
        self.0[piece_index] = Some(square);
    }

    /// Remove a piece from the board.
    ///
    /// Panics if `piece_index` does not have a valid square, or if `square` does not match the internal square.
    pub fn remove_piece(&mut self, piece_index: PieceIndex, square: Square) {
        let piece_index = usize::from(piece_index.into_inner());
        match self.0[piece_index] {
            None => panic!("attempted to remove piece from empty square"),
            Some(square_index) => {
                debug_assert!(
                    square_index == square,
                    "attempted to remove wrong piece from square"
                );
                self.0[piece_index] = None;
            }
        }
    }

    /// Move a piece in the piecelist.
    pub fn move_piece(&mut self, piece_index: PieceIndex, square: Square) {
        let piece_index = usize::from(piece_index.into_inner());
        self.0[piece_index] = Some(square);
    }
}
