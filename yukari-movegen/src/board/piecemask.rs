use super::{bitlist::Bitlist, index::PieceIndex};
use crate::{colour::Colour, piece::Piece};

#[derive(Clone)]
pub struct Piecemask {
    pbq: Bitlist,
    nbk: Bitlist,
    rqk: Bitlist,
}

impl Piecemask {
    pub const fn new() -> Self {
        Self {
            pbq: Bitlist::new(),
            nbk: Bitlist::new(),
            rqk: Bitlist::new(),
        }
    }

    pub const fn empty(&self) -> Bitlist {
        self.pbq.or(self.nbk).or(self.rqk).invert()
    }

    pub const fn occupied(&self) -> Bitlist {
        self.empty().invert()
    }

    pub const fn pawns(&self) -> Bitlist {
        self.pbq.and(self.nbk.invert()).and(self.rqk.invert())
    }

    pub const fn knights(&self) -> Bitlist {
        self.pbq.invert().and(self.nbk).and(self.rqk.invert())
    }

    pub const fn bishops(&self) -> Bitlist {
        self.pbq.and(self.nbk)
    }

    pub const fn rooks(&self) -> Bitlist {
        self.pbq.invert().and(self.nbk.invert()).and(self.rqk)
    }

    pub const fn queens(&self) -> Bitlist {
        self.pbq.and(self.rqk)
    }

    pub const fn kings(&self) -> Bitlist {
        self.nbk.and(self.rqk)
    }

    pub const fn white(&self) -> Bitlist {
        self.occupied().and(Bitlist::white())
    }

    pub const fn black(&self) -> Bitlist {
        self.occupied().and(Bitlist::black())
    }

    pub const fn pieces_of_colour(&self, colour: Colour) -> Bitlist {
        match colour {
            Colour::White => self.white(),
            Colour::Black => self.black(),
        }
    }

    pub const fn piece(&self, index: PieceIndex) -> Option<Piece> {
        const PIECES: [Option<Piece>; 8] = [None, Some(Piece::Pawn), Some(Piece::Knight), Some(Piece::Bishop), Some(Piece::Rook), Some(Piece::Queen), Some(Piece::King), None];
        let pbq = self.pbq.contains(Bitlist::from_piece(index)) as usize;
        let nbk = self.nbk.contains(Bitlist::from_piece(index)) as usize;
        let rqk = self.rqk.contains(Bitlist::from_piece(index)) as usize;
        let index = (rqk << 2) | (nbk << 1) | pbq;
        PIECES[index]
    }

    /// Add a piece to a `Piecemask`.
    ///
    /// Panics if adding a piece would give `colour` more than 16 pieces.
    pub fn add_piece(&mut self, piece: Piece, colour: Colour) -> PieceIndex {
        // SAFETY: a standard chess board has 32 pieces, of which 16 are white and 16 are black.
        // Here we have a 32-bit integer, of which 16 bits are white and 16 are black.
        // Thus, any position where one side has more than 16 pieces is by the rules of chess impossible to reach,
        // and thus every time this gets called there will be at least one empty bit.
        let piece_index =
            unsafe { (self.empty() & Bitlist::mask_from_colour(colour)).peek_nonzero() };
        let yes = Bitlist::from(piece_index);
        let no = Bitlist::new();

        let (pbq, nbk, rqk) = match piece {
            Piece::Pawn => (yes, no, no),
            Piece::Knight => (no, yes, no),
            Piece::Bishop => (yes, yes, no),
            Piece::Rook => (no, no, yes),
            Piece::Queen => (yes, no, yes),
            Piece::King => (no, yes, yes),
        };

        self.pbq |= pbq;
        self.nbk |= nbk;
        self.rqk |= rqk;

        piece_index
    }

    /// Remove a piece from a Piecemask.
    ///
    /// Panics if `piece_index` is not a valid piece.
    pub fn remove_piece(&mut self, piece_index: PieceIndex) {
        debug_assert!(
            self.occupied().contains(piece_index.into()),
            "attempted to remove invalid piece"
        );
        self.pbq &= !Bitlist::from(piece_index);
        self.nbk &= !Bitlist::from(piece_index);
        self.rqk &= !Bitlist::from(piece_index);
    }
}
