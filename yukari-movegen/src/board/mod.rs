use crate::{
    chessmove::{Move, MoveType},
    colour::Colour,
    piece::Piece,
    square::{Direction, File, Rank, Square, Square16x8},
};
use std::{
    convert::{TryFrom, TryInto},
    ffi::CString,
    fmt::Display,
};

use rand::{thread_rng, Rng};
use tinyvec::ArrayVec;

mod bitlist;
mod data;
mod index;
mod piecelist;
mod piecemask;

use bitlist::Bitlist;
use data::BoardData;
pub use index::PieceIndex;

/// Pin information in a board.
pub struct PinInfo {
    pub pins: [Option<Direction>; 32],
    pub enpassant_pinned: Bitlist,
}

impl PinInfo {
    pub const fn new() -> Self {
        Self {
            pins: [None; 32],
            enpassant_pinned: Bitlist::new(),
        }
    }
}

impl Default for PinInfo {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct Zobrist {
    pub piece: [[[u64; 64]; 6]; 2],
    pub side: u64,
    pub ep: [u64; 8],
    pub castling: [u64; 4],
}

impl Zobrist {
    #[must_use]
    pub fn new() -> Self {
        let mut rng = thread_rng();

        let mut piece = [[[0_u64; 64]; 6]; 2];
        let mut ep = [0; 8];
        let mut castling = [0; 4];

        for side in &mut piece {
            for piece_kind in side.iter_mut() {
                for square in piece_kind.iter_mut() {
                    *square = rng.gen();
                }
            }
        }

        let side = rng.gen();

        for file in &mut ep {
            *file = rng.gen();
        }

        for castle_flag in &mut castling {
            *castle_flag = rng.gen();
        }

        Self {
            piece,
            side,
            ep,
            castling,
        }
    }
}

impl Default for Zobrist {
    fn default() -> Self {
        Self::new()
    }
}

/// A chess position.
#[derive(Clone)]
pub struct Board {
    /// The chess board representation.
    data: data::BoardData,
    /// The side to move.
    side: Colour,
    /// Castling rights, if any.
    castle: (bool, bool, bool, bool),
    /// En-passant square, if any.
    ep: Option<Square>,
    /// Zobrist hash.
    hash: u64,
}

impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for Board {
    #[allow(clippy::missing_inline_in_public_items)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for i in 0_u8..64_u8 {
            let j = i ^ 56_u8;

            if let (Some(piece), Some(colour)) = (
                self.data
                    .piece_from_square(j.try_into().expect("square somehow out of bounds")),
                self.data
                    .colour_from_square(j.try_into().expect("square somehow out of bounds")),
            ) {
                let c = match piece {
                    Piece::Pawn => 'P',
                    Piece::Knight => 'N',
                    Piece::Bishop => 'B',
                    Piece::Rook => 'R',
                    Piece::Queen => 'Q',
                    Piece::King => 'K',
                };

                let c = match colour {
                    Colour::White => c.to_ascii_uppercase(),
                    Colour::Black => c.to_ascii_lowercase(),
                };

                write!(f, "{} ", c)?;
            } else {
                write!(f, ". ")?;
            }

            if j & 7 == 7 {
                writeln!(f)?;
            }
        }
        if self.side == Colour::White {
            writeln!(f, "White to move.")?;
        } else {
            writeln!(f, "Black to move.")?;
        }
        if self.castle.0 {
            write!(f, "K")?;
        }
        if self.castle.1 {
            write!(f, "Q")?;
        }
        if self.castle.2 {
            write!(f, "k")?;
        }
        if self.castle.3 {
            write!(f, "q")?;
        }
        writeln!(f)?;
        if let Some(ep) = self.ep {
            writeln!(f, "{}", ep)?;
        } else {
            writeln!(f, "-")?;
        }

        Ok(())
    }
}

impl Board {
    /// Create a new empty board.
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self {
            side: Colour::White,
            castle: (false, false, false, false),
            ep: None,
            data: BoardData::new(),
            hash: 0,
        }
    }

    #[allow(clippy::missing_panics_doc)]
    #[must_use]
    pub fn startpos(zobrist: &Zobrist) -> Self {
        Self::from_fen(
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            zobrist,
        )
        .unwrap()
    }

    /// Check if this board is illegal by seeing if the enemy king is attacked by friendly pieces.
    /// If it is, it implies the move the enemy made left them in check, which is illegal.
    #[must_use]
    #[inline]
    pub fn illegal(&self) -> bool {
        let king_index =
            unsafe { (self.data.kings() & self.data.pieces_of_colour(!self.side)).peek_nonzero() };
        let king_square = self.data.square_of_piece(king_index);
        !self.data.attacks_to(king_square, self.side).empty()
    }

    /// Parse a position in Forsyth-Edwards Notation into a board.
    #[must_use]
    pub fn from_fen(fen: &str, zobrist: &Zobrist) -> Option<Self> {
        let fen = CString::new(fen).expect("FEN is not ASCII");
        let fen = fen.as_bytes();
        Self::from_fen_bytes(fen, zobrist)
    }

    /// Parse a position in Forsyth-Edwards Notation into a board.
    ///
    /// # Panics
    /// Panics when invalid FEN is input.
    #[must_use]
    pub fn from_fen_bytes(fen: &[u8], zobrist: &Zobrist) -> Option<Self> {
        let mut b = Self::new();

        let mut idx = 0_usize;
        let mut c = fen[idx];

        for rank in (0..=7).rev() {
            let mut file = 0;
            while file <= 7 {
                if (b'1'..=b'8').contains(&c) {
                    let length = c - b'0';
                    let mut i = 0;
                    while i < length {
                        file += 1;
                        i += 1;
                    }
                } else {
                    let piece = match c.to_ascii_lowercase() {
                        b'k' => Piece::King,
                        b'q' => Piece::Queen,
                        b'r' => Piece::Rook,
                        b'b' => Piece::Bishop,
                        b'n' => Piece::Knight,
                        b'p' => Piece::Pawn,
                        _ => return None,
                    };

                    let colour = if c.is_ascii_uppercase() {
                        Colour::White
                    } else {
                        Colour::Black
                    };

                    let square =
                        Square::from_rank_file(rank.try_into().unwrap(), file.try_into().unwrap());

                    b.data.add_piece(piece, colour, square, false);

                    file += 1;
                }
                idx += 1;
                c = fen[idx];
            }
            if rank > 0 {
                idx += 1;
                c = fen[idx];
            }
        }
        idx += 1;
        c = fen[idx];
        b.side = match c {
            b'w' => Colour::White,
            b'b' => Colour::Black,
            _ => return None,
        };
        idx += 2;
        c = fen[idx];
        b.castle = (false, false, false, false);
        if c == b'-' {
            idx += 1;
        } else {
            if c == b'K' {
                b.castle.0 = true;
                idx += 1;
                c = fen[idx];
            }
            if c == b'Q' {
                b.castle.1 = true;
                idx += 1;
                c = fen[idx];
            }
            if c == b'k' {
                b.castle.2 = true;
                idx += 1;
                c = fen[idx];
            }
            if c == b'q' {
                b.castle.3 = true;
                idx += 1;
            }
        }
        idx += 1;
        c = fen[idx];
        if c == b'-' {
            b.ep = None;
        } else {
            let file = File::try_from(c - b'a').unwrap();
            idx += 1;
            c = fen[idx];
            let rank = Rank::try_from(c - b'1').unwrap();
            b.ep = Some(Square::from_rank_file(rank, file));
        }

        b.recalculate_hash(zobrist);
        b.data.rebuild_attacks();

        Some(b)
    }

    fn set_ep(&mut self, zobrist: &Zobrist, ep: Option<Square>) {
        if let Some(ep) = self.ep {
            self.hash ^= zobrist.ep[File::from(ep) as usize];
        }
        self.ep = ep;
        if let Some(ep) = self.ep {
            self.hash ^= zobrist.ep[File::from(ep) as usize];
        }
    }

    /// Make a move on the board.
    ///
    /// # Panics
    /// Panics when Lofty hasn't implemented necessary code.
    #[inline]
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn make(&self, m: Move, zobrist: &Zobrist) -> Self {
        let mut b = self.clone();
        match m.kind {
            MoveType::Normal => {
                let piece = b.piece_from_square(m.from).unwrap() as usize;
                b.data.move_piece(m.from, m.dest);
                b.hash ^= zobrist.piece[b.side as usize][piece][m.from.into_inner() as usize]
                    ^ zobrist.piece[b.side as usize][piece][m.dest.into_inner() as usize];
                b.set_ep(zobrist, None);
            }
            MoveType::DoublePush => {
                let piece = b.piece_from_square(m.from).unwrap() as usize;
                b.data.move_piece(m.from, m.dest);
                b.hash ^= zobrist.piece[b.side as usize][piece][m.from.into_inner() as usize]
                    ^ zobrist.piece[b.side as usize][piece][m.dest.into_inner() as usize];
                b.set_ep(zobrist, m.from.relative_north(b.side));
            }
            MoveType::Capture => {
                let piece_index = b
                    .data
                    .piece_index(m.dest)
                    .expect("attempted to capture an empty square");
                let moving_piece = b.piece_from_square(m.from).unwrap() as usize;
                let captured_piece = b.piece_from_square(m.dest).unwrap() as usize;
                b.data.remove_piece(piece_index, true);
                b.data.move_piece(m.from, m.dest);
                b.hash ^= zobrist.piece[b.side as usize][moving_piece]
                    [m.from.into_inner() as usize]
                    ^ zobrist.piece[b.side as usize][moving_piece][m.dest.into_inner() as usize]
                    ^ zobrist.piece[!b.side as usize][captured_piece][m.dest.into_inner() as usize];
                b.set_ep(zobrist, None);
            }
            MoveType::Castle => {
                if m.dest > m.from {
                    let rook_from = m.dest.east().unwrap();
                    let rook_to = m.dest.west().unwrap();
                    b.data.move_piece(rook_from, rook_to);
                    b.hash ^= zobrist.piece[b.side as usize][Piece::Rook as usize]
                        [rook_from.into_inner() as usize]
                        ^ zobrist.piece[b.side as usize][Piece::Rook as usize]
                            [rook_to.into_inner() as usize];
                } else {
                    let rook_from = m.dest.west().unwrap().west().unwrap();
                    let rook_to = m.dest.east().unwrap();
                    b.data.move_piece(rook_from, rook_to);
                    b.hash ^= zobrist.piece[b.side as usize][Piece::Rook as usize]
                        [rook_from.into_inner() as usize]
                        ^ zobrist.piece[b.side as usize][Piece::Rook as usize]
                            [rook_to.into_inner() as usize];
                }
                b.data.move_piece(m.from, m.dest);
                b.hash ^= zobrist.piece[b.side as usize][Piece::King as usize]
                    [m.from.into_inner() as usize]
                    ^ zobrist.piece[b.side as usize][Piece::King as usize]
                        [m.dest.into_inner() as usize];
                b.set_ep(zobrist, None);
            }
            MoveType::EnPassant => {
                let target_square = b.ep.unwrap().relative_south(b.side).unwrap();
                let target_piece = b.data.piece_index(target_square).unwrap();
                b.data.remove_piece(target_piece, true);
                b.data.move_piece(m.from, m.dest);
                b.hash ^= zobrist.piece[b.side as usize][Piece::Pawn as usize]
                    [m.from.into_inner() as usize]
                    ^ zobrist.piece[b.side as usize][Piece::Pawn as usize]
                        [m.dest.into_inner() as usize]
                    ^ zobrist.piece[!b.side as usize][Piece::Pawn as usize]
                        [target_square.into_inner() as usize];
                b.set_ep(zobrist, None);
            }
            MoveType::Promotion => {
                let piece_index = b.data.piece_index(m.from).unwrap();
                b.data.remove_piece(piece_index, true);
                b.data.add_piece(m.prom.unwrap(), b.side, m.dest, true);
                b.hash ^= zobrist.piece[b.side as usize][Piece::Pawn as usize]
                    [m.from.into_inner() as usize]
                    ^ zobrist.piece[b.side as usize][m.prom.unwrap() as usize]
                        [m.dest.into_inner() as usize];
                b.set_ep(zobrist, None);
            }
            MoveType::CapturePromotion => {
                let source_piece = b.data.piece_index(m.from).unwrap();
                let target_piece = b.data.piece_index(m.dest).unwrap();
                let captured_piece = b.piece_from_square(m.dest).unwrap() as usize;
                b.data.remove_piece(source_piece, true);
                b.data.remove_piece(target_piece, true);
                b.data.add_piece(m.prom.unwrap(), b.side, m.dest, true);
                b.hash ^= zobrist.piece[b.side as usize][Piece::Pawn as usize]
                    [m.from.into_inner() as usize]
                    ^ zobrist.piece[b.side as usize][m.prom.unwrap() as usize]
                        [m.dest.into_inner() as usize]
                    ^ zobrist.piece[!b.side as usize][captured_piece][m.dest.into_inner() as usize];
                b.set_ep(zobrist, None);
            }
        }

        let a1 = Square::from_rank_file(Rank::One, File::A);
        let a8 = Square::from_rank_file(Rank::Eight, File::A);
        let e1 = Square::from_rank_file(Rank::One, File::E);
        let e8 = Square::from_rank_file(Rank::Eight, File::E);
        let h1 = Square::from_rank_file(Rank::One, File::H);
        let h8 = Square::from_rank_file(Rank::Eight, File::H);

        if m.from == e1 {
            if b.castle.0 {
                b.castle.0 = false;
                b.hash ^= zobrist.castling[0];
            }
            if b.castle.1 {
                b.castle.1 = false;
                b.hash ^= zobrist.castling[1];
            }
        }

        if m.from == e8 {
            if b.castle.2 {
                b.castle.2 = false;
                b.hash ^= zobrist.castling[2];
            }
            if b.castle.3 {
                b.castle.3 = false;
                b.hash ^= zobrist.castling[3];
            }
        }

        if (m.from == h1 || m.dest == h1) && b.castle.0 {
            b.castle.0 = false;
            b.hash ^= zobrist.castling[0];
        }

        if (m.from == a1 || m.dest == a1) && b.castle.1 {
            b.castle.1 = false;
            b.hash ^= zobrist.castling[1];
        }

        if (m.from == h8 || m.dest == h8) && b.castle.2 {
            b.castle.2 = false;
            b.hash ^= zobrist.castling[2];
        }

        if (m.from == a8 || m.dest == a8) && b.castle.3 {
            b.castle.3 = false;
            b.hash ^= zobrist.castling[3];
        }

        b.side = !b.side;
        b.hash ^= zobrist.side;
        b
    }

    fn try_push_move(
        &self,
        v: &mut ArrayVec<[Move; 256]>,
        from: Square,
        dest: Square,
        kind: MoveType,
        promotion_piece: Option<Piece>,
        pininfo: &PinInfo,
    ) {
        if let Some(dir) = pininfo.pins[self.data.piece_index(from).unwrap().into_inner() as usize]
        {
            if let Some(move_dir) = from.direction(dest) {
                // Pinned slider can only move along pin ray.
                if dir != move_dir && dir != move_dir.opposite() {
                    return;
                }
            } else {
                // Pinned knight can't move.
                return;
            }
        }
        v.push(Move::new(from, dest, kind, promotion_piece));
    }

    /// Find pinned pieces and handle them specially.
    ///
    /// # Panics
    /// Panics when Lofty has written shitty code.
    #[must_use]
    pub fn discover_pinned_pieces(&self) -> PinInfo {
        let mut info = PinInfo::new();

        let sliders = self.data.bishops() | self.data.rooks() | self.data.queens();
        let king_index =
            unsafe { (self.data.kings() & Bitlist::mask_from_colour(self.side)).peek_nonzero() };
        let king_square = self.data.square_of_piece(king_index);
        let king_square_16x8 = Square16x8::from_square(king_square);

        for possible_pinner in self.data.pieces_of_colour(!self.side).and(sliders) {
            let pinner_square = self.data.square_of_piece(possible_pinner);
            let pinner_square_16x8 = Square16x8::from_square(pinner_square);
            let pinner_type = self.data.piece_from_bit(possible_pinner);
            let pinner_king_dir = match pinner_square_16x8.direction(king_square_16x8) {
                Some(dir) => dir,
                None => continue,
            };

            if !pinner_king_dir.valid_for_slider(pinner_type) {
                continue;
            }

            let mut friendly_blocker = None;
            let mut enemy_blocker = None;
            for square in pinner_square_16x8.ray_attacks(pinner_king_dir) {
                if square == king_square {
                    break;
                }

                if let Some(piece_index) = self.data.piece_index(square) {
                    if self.data.colour_from_square(square) == Some(!self.side) {
                        match enemy_blocker {
                            Some(_) => {
                                friendly_blocker = None;
                                enemy_blocker = None;
                                break;
                            }
                            None => {
                                enemy_blocker = Some(piece_index);
                            }
                        }
                    } else {
                        match friendly_blocker {
                            Some(_) => {
                                friendly_blocker = None;
                                enemy_blocker = None;
                                break;
                            }
                            None => {
                                friendly_blocker = Some(piece_index);
                            }
                        }
                    }
                }
            }

            match (friendly_blocker, enemy_blocker) {
                // There are no friendly blockers: skip.
                (None, _) => continue,
                // There is one friendly blocker: it is pinned.
                (Some(blocker), None) => {
                    info.pins[blocker.into_inner() as usize] = Some(pinner_king_dir);
                }
                // There is one friendly blocker and one enemy blocker: it *may* be pinned for en-passant purposes
                (Some(friendly_blocker), Some(enemy_blocker)) => {
                    // If at least one of the blockers is a piece, we don't need to worry about en-passant.
                    if self.data.piece_from_bit(friendly_blocker) != Piece::Pawn
                        || self.data.piece_from_bit(enemy_blocker) != Piece::Pawn
                        || (pinner_king_dir != Direction::East
                            && pinner_king_dir != Direction::West)
                    {
                        continue;
                    }

                    // Alas, we do have to care.
                    info.enpassant_pinned |= Bitlist::from(friendly_blocker);
                }
            }
        }

        info
    }

    /// Generate en-passant pawn moves.
    fn generate_pawn_enpassant(&self, v: &mut ArrayVec<[Move; 256]>, pininfo: &PinInfo) {
        if let Some(ep) = self.ep {
            for capturer in self
                .data
                .attacks_to(ep, self.side)
                .and(self.data.pawns())
                .and(!pininfo.enpassant_pinned)
            {
                let from = self.data.square_of_piece(capturer);
                self.try_push_move(v, from, ep, MoveType::EnPassant, None, pininfo);
            }
        }
    }

    /// Generate pawn-specific quiet moves.
    fn generate_pawn_quiet(&self, v: &mut ArrayVec<[Move; 256]>, from: Square, pininfo: &PinInfo) {
        let north = from.relative_north(self.side);
        if let Some(dest) = north {
            // Pawn single pushes.
            if !self.data.has_piece(dest) {
                if Rank::from(dest).is_relative_eighth(self.side) {
                    self.try_push_move(
                        v,
                        from,
                        dest,
                        MoveType::Promotion,
                        Some(Piece::Queen),
                        pininfo,
                    );
                    self.try_push_move(
                        v,
                        from,
                        dest,
                        MoveType::Promotion,
                        Some(Piece::Knight),
                        pininfo,
                    );
                    self.try_push_move(
                        v,
                        from,
                        dest,
                        MoveType::Promotion,
                        Some(Piece::Rook),
                        pininfo,
                    );
                    self.try_push_move(
                        v,
                        from,
                        dest,
                        MoveType::Promotion,
                        Some(Piece::Bishop),
                        pininfo,
                    );
                } else {
                    self.try_push_move(v, from, dest, MoveType::Normal, None, pininfo);
                }

                // Pawn double pushes.
                let north2 = dest.relative_north(self.side);
                if let Some(dest) = north2 {
                    if Rank::from(dest).is_relative_fourth(self.side) && !self.data.has_piece(dest)
                    {
                        self.try_push_move(v, from, dest, MoveType::DoublePush, None, pininfo);
                    }
                }
            }
        }
    }

    /// Generate moves when in check by a single piece.
    #[allow(clippy::too_many_lines)]
    fn generate_single_check(&self, v: &mut ArrayVec<[Move; 256]>) {
        #[allow(clippy::unwrap_used)]
        let king_index =
            unsafe { (self.data.kings() & Bitlist::mask_from_colour(self.side)).peek_nonzero() };
        let king_square = self.data.square_of_piece(king_index);
        let king_square_16x8 = Square16x8::from_square(king_square);
        let attacker_bit = self.data.attacks_to(king_square, !self.side);
        let attacker_index = unsafe { attacker_bit.peek_nonzero() };
        let attacker_piece = self.data.piece_from_bit(attacker_index);
        let attacker_square = self.data.square_of_piece(attacker_index);
        let attacker_direction = attacker_square.direction(king_square);

        let pininfo = self.discover_pinned_pieces();

        let add_pawn_block = |v: &mut ArrayVec<[Move; 256]>, from, dest, kind| {
            if let Some(colour) = self.data.colour_from_square(from) {
                if colour == self.side {
                    self.try_push_move(v, from, dest, kind, None, &pininfo);
                }
            }
        };

        let add_pawn_blocks = |v: &mut ArrayVec<[Move; 256]>, dest: Square| {
            if let Some(from) = dest.relative_south(self.side) {
                match self.data.piece_from_square(from) {
                    Some(Piece::Pawn) => add_pawn_block(v, from, dest, MoveType::Normal),
                    Some(_) => {}
                    None => {
                        if Rank::from(dest).is_relative_fourth(self.side) {
                            if let Some(from) = from.relative_south(self.side) {
                                if self.data.piece_from_square(from) == Some(Piece::Pawn) {
                                    add_pawn_block(v, from, dest, MoveType::DoublePush);
                                }
                            }
                        }
                    }
                }
            }
        };

        // Can we capture the attacker?
        for capturer in self.data.attacks_to(attacker_square, self.side) {
            let from = self.data.square_of_piece(capturer);
            if self.data.piece_from_bit(capturer) == Piece::King
                && !self.data.attacks_to(attacker_square, !self.side).empty()
            {
                continue;
            }
            if self.data.piece_from_bit(capturer) == Piece::Pawn
                && Rank::from(attacker_square).is_relative_eighth(self.side)
            {
                self.try_push_move(
                    v,
                    from,
                    attacker_square,
                    MoveType::CapturePromotion,
                    Some(Piece::Queen),
                    &pininfo,
                );
                self.try_push_move(
                    v,
                    from,
                    attacker_square,
                    MoveType::CapturePromotion,
                    Some(Piece::Knight),
                    &pininfo,
                );
                self.try_push_move(
                    v,
                    from,
                    attacker_square,
                    MoveType::CapturePromotion,
                    Some(Piece::Rook),
                    &pininfo,
                );
                self.try_push_move(
                    v,
                    from,
                    attacker_square,
                    MoveType::CapturePromotion,
                    Some(Piece::Bishop),
                    &pininfo,
                );
            } else {
                self.try_push_move(v, from, attacker_square, MoveType::Capture, None, &pininfo);
            }
        }

        if let Some(ep) = self.ep {
            if let Some(ep_south) = ep.relative_south(self.side) {
                if ep_south == attacker_square && attacker_piece == Piece::Pawn {
                    for capturer in self.data.attacks_to(ep, self.side)
                        & self.data.pawns()
                        & !pininfo.enpassant_pinned
                    {
                        self.try_push_move(
                            v,
                            self.data.square_of_piece(capturer),
                            ep,
                            MoveType::EnPassant,
                            None,
                            &pininfo,
                        );
                    }
                }
            }
        }

        // Can we block the check?
        if let Piece::Bishop | Piece::Rook | Piece::Queen = attacker_piece {
            let direction = king_square.direction(attacker_square).unwrap();
            for dest in king_square_16x8.ray_attacks(direction) {
                if dest == attacker_square {
                    break;
                }

                // Piece moves.
                for attacker in self
                    .data
                    .attacks_to(dest, self.side)
                    .and(!self.data.pawns())
                    .and(!self.data.kings())
                {
                    self.try_push_move(
                        v,
                        self.data.square_of_piece(attacker),
                        dest,
                        MoveType::Normal,
                        None,
                        &pininfo,
                    );
                }

                // Pawn moves.
                add_pawn_blocks(v, dest);
            }
        }

        // Can we move the king?
        for square in king_square.king_attacks() {
            let kind = if self.data.has_piece(square) {
                if square == attacker_square
                    || self.data.colour_from_square(square) == Some(self.side)
                {
                    // Own-piece captures are illegal, captures of the attacker are handled elsewhere.
                    continue;
                }
                MoveType::Capture
            } else {
                MoveType::Normal
            };

            if !self.data.attacks_to(square, !self.side).empty() {
                // Moving into check is illegal.
                continue;
            }
            if let Some(attacker_direction) = attacker_direction {
                // Slider attacks x-ray through the king to attack that square.
                if let Some(xray_square) = king_square.travel(attacker_direction) {
                    if matches!(attacker_piece, Piece::Bishop | Piece::Rook | Piece::Queen)
                        && xray_square == square
                    {
                        continue;
                    }
                }
            }

            v.push(Move::new(king_square, square, kind, None));
        }
    }

    fn generate_double_check(&self, v: &mut ArrayVec<[Move; 256]>) {
        #[allow(clippy::unwrap_used)]
        let king_index =
            unsafe { (self.data.kings() & Bitlist::mask_from_colour(self.side)).peek_nonzero() };
        let king_square = self.data.square_of_piece(king_index);
        let mut attacker_bits = self.data.attacks_to(king_square, !self.side);
        let attacker1_index = attacker_bits.pop().unwrap();
        let attacker1_piece = self.data.piece_from_bit(attacker1_index);
        let attacker1_square = self.data.square_of_piece(attacker1_index);
        let attacker1_direction = attacker1_square.direction(king_square);
        let attacker2_index = attacker_bits.pop().unwrap();
        let attacker2_piece = self.data.piece_from_bit(attacker2_index);
        let attacker2_square = self.data.square_of_piece(attacker2_index);
        let attacker2_direction = attacker2_square.direction(king_square);

        // Can we move the king?
        for square in king_square.king_attacks() {
            let kind = if self.data.has_piece(square) {
                if self.data.colour_from_square(square) == Some(self.side) {
                    // Own-piece captures are illegal.
                    continue;
                }
                MoveType::Capture
            } else {
                MoveType::Normal
            };

            if !self.data.attacks_to(square, !self.side).empty() {
                // Moving into check is illegal.
                continue;
            }

            // Slider attacks x-ray through the king to attack that square.
            if let Some(attacker1_direction) = attacker1_direction {
                if let Some(xray_square) = king_square.travel(attacker1_direction) {
                    if matches!(attacker1_piece, Piece::Bishop | Piece::Rook | Piece::Queen)
                        && xray_square == square
                    {
                        continue;
                    }
                }
            }

            if let Some(attacker2_direction) = attacker2_direction {
                if let Some(xray_square) = king_square.travel(attacker2_direction) {
                    if matches!(attacker2_piece, Piece::Bishop | Piece::Rook | Piece::Queen)
                        && xray_square == square
                    {
                        continue;
                    }
                }
            }

            v.push(Move::new(king_square, square, kind, None));
        }
    }

    pub fn generate_captures(&self, v: &mut ArrayVec<[Move; 256]>) {
        let pininfo = self.discover_pinned_pieces();

        let mut find_attackers = |dest: Square| {
            let attacks = self.data.attacks_to(dest, self.side);
            for capturer in attacks & self.data.pawns() {
                let from = self.data.square_of_piece(capturer);
                if Rank::from(dest).is_relative_eighth(self.side) {
                    self.try_push_move(
                        v,
                        from,
                        dest,
                        MoveType::CapturePromotion,
                        Some(Piece::Queen),
                        &pininfo,
                    );
                    self.try_push_move(
                        v,
                        from,
                        dest,
                        MoveType::CapturePromotion,
                        Some(Piece::Knight),
                        &pininfo,
                    );
                    self.try_push_move(
                        v,
                        from,
                        dest,
                        MoveType::CapturePromotion,
                        Some(Piece::Rook),
                        &pininfo,
                    );
                    self.try_push_move(
                        v,
                        from,
                        dest,
                        MoveType::CapturePromotion,
                        Some(Piece::Bishop),
                        &pininfo,
                    );
                } else {
                    self.try_push_move(v, from, dest, MoveType::Capture, None, &pininfo);
                }
            }
            for capturer in attacks & self.data.knights() {
                let from = self.data.square_of_piece(capturer);
                self.try_push_move(v, from, dest, MoveType::Capture, None, &pininfo);
            }
            for capturer in attacks & self.data.bishops() {
                let from = self.data.square_of_piece(capturer);
                self.try_push_move(v, from, dest, MoveType::Capture, None, &pininfo);
            }
            for capturer in attacks & self.data.rooks() {
                let from = self.data.square_of_piece(capturer);
                self.try_push_move(v, from, dest, MoveType::Capture, None, &pininfo);
            }
            for capturer in attacks & self.data.queens() {
                let from = self.data.square_of_piece(capturer);
                self.try_push_move(v, from, dest, MoveType::Capture, None, &pininfo);
            }
            for capturer in attacks & self.data.kings() {
                let from = self.data.square_of_piece(capturer);
                if !self.data.attacks_to(dest, !self.side).empty() {
                    // Moving into check is illegal.
                    continue;
                }
                self.try_push_move(v, from, dest, MoveType::Capture, None, &pininfo);
            }
        };

        for victim in self.data.pieces_of_colour(!self.side) & self.data.queens() {
            find_attackers(self.square_of_piece(victim));
        }
        for victim in self.data.pieces_of_colour(!self.side) & self.data.rooks() {
            find_attackers(self.square_of_piece(victim));
        }
        for victim in self.data.pieces_of_colour(!self.side) & self.data.bishops() {
            find_attackers(self.square_of_piece(victim));
        }
        for victim in self.data.pieces_of_colour(!self.side) & self.data.knights() {
            find_attackers(self.square_of_piece(victim));
        }
        for victim in self.data.pieces_of_colour(!self.side) & self.data.pawns() {
            find_attackers(self.square_of_piece(victim));
        }

        self.generate_pawn_enpassant(v, &pininfo);
    }

    #[allow(clippy::missing_panics_doc, clippy::too_many_lines)]
    pub fn generate_captures_incremental<F: FnMut(Move) -> bool>(&self, mut f: F) {
        let pininfo = self.discover_pinned_pieces();

        let mut minor_mask = Bitlist::new();
        let mut rook_mask = Bitlist::new();
        let mut queen_mask = Bitlist::new();

        let mut try_move = |from: Square,
                            dest: Square,
                            kind: MoveType,
                            promotion_piece: Option<Piece>,
                            pininfo: &PinInfo| {
            if let Some(dir) =
                pininfo.pins[self.data.piece_index(from).unwrap().into_inner() as usize]
            {
                if let Some(move_dir) = from.direction(dest) {
                    // Pinned slider can only move along pin ray.
                    if dir == move_dir || dir == move_dir.opposite() {
                        return f(Move::new(from, dest, kind, promotion_piece));
                    }
                }
                // Pinned knight can't move.
                return true;
            }
            f(Move::new(from, dest, kind, promotion_piece))
        };

        let mut find_attackers = |dest: Square,
                                  victim_type: Piece,
                                  minor_mask: Bitlist,
                                  rook_mask: Bitlist,
                                  queen_mask: Bitlist|
         -> bool {
            let attacks = self.data.attacks_to(dest, self.side);
            for capturer in attacks & self.data.pawns() {
                let from = self.data.square_of_piece(capturer);
                if Rank::from(dest).is_relative_eighth(self.side) {
                    if !try_move(
                        from,
                        dest,
                        MoveType::CapturePromotion,
                        Some(Piece::Queen),
                        &pininfo,
                    ) {
                        return false;
                    }
                    if !try_move(
                        from,
                        dest,
                        MoveType::CapturePromotion,
                        Some(Piece::Knight),
                        &pininfo,
                    ) {
                        return false;
                    }
                    if !try_move(
                        from,
                        dest,
                        MoveType::CapturePromotion,
                        Some(Piece::Rook),
                        &pininfo,
                    ) {
                        return false;
                    }
                    if !try_move(
                        from,
                        dest,
                        MoveType::CapturePromotion,
                        Some(Piece::Bishop),
                        &pininfo,
                    ) {
                        return false;
                    }
                } else if !try_move(from, dest, MoveType::Capture, None, &pininfo) {
                    return false;
                }
            }
            for capturer in attacks & (self.data.knights() | self.data.bishops()) {
                let from = self.data.square_of_piece(capturer);
                if victim_type < Piece::Bishop
                    && !(self.data.attacks_to(dest, !self.side) & minor_mask).empty()
                {
                    // This is a bad capture.
                    continue;
                }
                if !try_move(from, dest, MoveType::Capture, None, &pininfo) {
                    return false;
                }
            }
            for capturer in attacks & self.data.rooks() {
                let from = self.data.square_of_piece(capturer);
                if victim_type < Piece::Rook
                    && !(self.data.attacks_to(dest, !self.side) & rook_mask).empty()
                {
                    // This is a bad capture.
                    continue;
                }
                if !try_move(from, dest, MoveType::Capture, None, &pininfo) {
                    return false;
                }
            }
            for capturer in attacks & self.data.queens() {
                let from = self.data.square_of_piece(capturer);
                if victim_type < Piece::Queen
                    && !(self.data.attacks_to(dest, !self.side) & queen_mask).empty()
                {
                    // This is a bad capture.
                    continue;
                }
                if !try_move(from, dest, MoveType::Capture, None, &pininfo) {
                    return false;
                }
            }
            for capturer in attacks & self.data.kings() {
                let from = self.data.square_of_piece(capturer);
                if !self.data.attacks_to(dest, !self.side).empty() {
                    // Moving into check is illegal.
                    continue;
                }
                if !try_move(from, dest, MoveType::Capture, None, &pininfo) {
                    return false;
                }
            }
            true
        };

        minor_mask |= self.data.pieces_of_colour(!self.side) & self.data.pawns();
        rook_mask |= self.data.pieces_of_colour(!self.side) & self.data.pawns();
        queen_mask |= self.data.pieces_of_colour(!self.side) & self.data.pawns();

        for victim in self.data.pieces_of_colour(!self.side) & self.data.queens() {
            if !find_attackers(
                self.square_of_piece(victim),
                Piece::Queen,
                minor_mask,
                rook_mask,
                queen_mask,
            ) {
                return;
            }
        }

        queen_mask |=
            self.data.pieces_of_colour(!self.side) & (self.data.knights() | self.data.bishops());

        for victim in self.data.pieces_of_colour(!self.side) & self.data.rooks() {
            if !find_attackers(
                self.square_of_piece(victim),
                Piece::Rook,
                minor_mask,
                rook_mask,
                queen_mask,
            ) {
                return;
            }
        }

        queen_mask |= self.data.pieces_of_colour(!self.side) & self.data.rooks();

        for victim in
            self.data.pieces_of_colour(!self.side) & (self.data.knights() | self.data.bishops())
        {
            if !find_attackers(
                self.square_of_piece(victim),
                Piece::Bishop,
                minor_mask,
                rook_mask,
                queen_mask,
            ) {
                return;
            }
        }

        rook_mask |=
            self.data.pieces_of_colour(!self.side) & (self.data.knights() | self.data.bishops());

        for victim in self.data.pieces_of_colour(!self.side) & self.data.pawns() {
            if !find_attackers(
                self.square_of_piece(victim),
                Piece::Pawn,
                minor_mask,
                rook_mask,
                queen_mask,
            ) {
                return;
            }
        }
    }

    /// Generate a vector of moves on the board.
    ///
    /// # Panics
    /// Panics when Lofty writes shitty code.
    #[allow(clippy::missing_inline_in_public_items)]
    pub fn generate(&self, v: &mut ArrayVec<[Move; 256]>) {
        // Unless something has gone very badly wrong we have to have a king.
        let king_index =
            unsafe { (self.data.kings() & Bitlist::mask_from_colour(self.side)).peek_nonzero() };
        let king_square = self.data.square_of_piece(king_index);
        let checks = self.data.attacks_to(king_square, !self.side);

        if checks.count_ones() == 1 {
            return self.generate_single_check(v);
        }
        if checks.count_ones() == 2 {
            return self.generate_double_check(v);
        }

        let pininfo = self.discover_pinned_pieces();
        self.generate_captures(v);

        // Pawns.
        for pawn in self.data.pawns().and(Bitlist::mask_from_colour(self.side)) {
            let from = self.data.square_of_piece(pawn);
            self.generate_pawn_quiet(v, from, &pininfo);
        }

        // General quiet move loop; pawns and kings handled separately.
        for dest in 0_u8..64 {
            // Squares will always be in range, so this will never panic.
            let dest = unsafe { Square::from_u8_unchecked(dest) };

            // Ignore captures.
            if self.data.has_piece(dest) {
                continue;
            }

            // For every piece that attacks this square, find its location and add it to the move list.
            for attacker in self
                .data
                .attacks_to(dest, self.side)
                .and(!self.data.pawns())
            //.and(!self.data.kings())
            {
                // It's illegal for kings to move to attacked squares; prune those out.
                if self.data.piece_from_bit(attacker) == Piece::King
                    && !self.data.attacks_to(dest, !self.side).empty()
                {
                    continue;
                }

                let from = self.data.square_of_piece(attacker);
                self.try_push_move(v, from, dest, MoveType::Normal, None, &pininfo);
            }
        }

        // Kingside castling.
        if (self.side == Colour::White && self.castle.0)
            || (self.side == Colour::Black && self.castle.2)
        {
            let east1 = king_square.east().unwrap();
            let east2 = east1.east().unwrap();
            if self.data.attacks_to(king_square, !self.side).empty()
                && !self.data.has_piece(east1)
                && self.data.attacks_to(east1, !self.side).empty()
                && !self.data.has_piece(east2)
                && self.data.attacks_to(east2, !self.side).empty()
            {
                self.try_push_move(v, king_square, east2, MoveType::Castle, None, &pininfo);
            }
        }

        // Queenside castling.
        if (self.side == Colour::White && self.castle.1)
            || (self.side == Colour::Black && self.castle.3)
        {
            let west1 = king_square.west().unwrap();
            let west2 = west1.west().unwrap();
            let west3 = west2.west().unwrap();
            if self.data.attacks_to(king_square, !self.side).empty()
                && !self.data.has_piece(west1)
                && self.data.attacks_to(west1, !self.side).empty()
                && !self.data.has_piece(west2)
                && self.data.attacks_to(west2, !self.side).empty()
                && !self.data.has_piece(west3)
            {
                self.try_push_move(v, king_square, west2, MoveType::Castle, None, &pininfo);
            }
        }
    }

    #[must_use]
    pub const fn kings(&self) -> Bitlist {
        self.data.kings()
    }

    /// Return a bitlist of all pieces.
    #[must_use]
    pub const fn pieces(&self) -> Bitlist {
        self.data.pieces()
    }

    /// Given a piece index, return its piece type.
    #[must_use]
    pub fn piece_from_bit(&self, bit: PieceIndex) -> Piece {
        self.data.piece_from_bit(bit)
    }

    #[must_use]
    pub fn piece_from_square(&self, square: Square) -> Option<Piece> {
        self.data.piece_from_square(square)
    }

    #[must_use]
    pub fn square_of_piece(&self, bit: PieceIndex) -> Square {
        self.data.square_of_piece(bit)
    }

    #[must_use]
    pub const fn ep(&self) -> Option<Square> {
        self.ep
    }

    #[must_use]
    pub const fn side(&self) -> Colour {
        self.side
    }

    #[must_use]
    pub const fn hash(&self) -> u64 {
        self.hash
    }

    pub fn recalculate_hash(&mut self, zobrist: &Zobrist) {
        let mut hash = 0;
        for piece in self.pieces() {
            let side = piece.colour() as usize;
            let square = self.square_of_piece(piece).into_inner() as usize;
            let piece = self.piece_from_bit(piece) as usize;
            hash ^= zobrist.piece[side][piece][square];
        }

        if let Some(ep) = self.ep {
            hash ^= zobrist.ep[Rank::from(ep) as usize];
        }

        if self.castle.0 {
            hash ^= zobrist.castling[0];
        }
        if self.castle.1 {
            hash ^= zobrist.castling[1];
        }
        if self.castle.2 {
            hash ^= zobrist.castling[2];
        }
        if self.castle.3 {
            hash ^= zobrist.castling[3];
        }
        if self.side == Colour::Black {
            hash ^= zobrist.side;
        }
        self.hash = hash;
    }

    #[must_use]
    pub fn in_check(&self) -> bool {
        let king_index =
            unsafe { (self.data.kings() & Bitlist::mask_from_colour(self.side)).peek_nonzero() };
        let king_square = self.data.square_of_piece(king_index);
        !self.data.attacks_to(king_square, !self.side).empty()
    }

    #[must_use]
    pub fn make_null(&self, zobrist: &Zobrist) -> Self {
        let mut board = self.clone();
        board.side = !board.side;
        board.ep = None;
        board.hash ^= zobrist.side;
        board
    }
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use tinyvec::ArrayVec;

    use crate::{Board, Move, Square, Zobrist};

    // Helper mostly copied from main engine to convert notated moves into real moves
    fn make_move(board: &Board, zobrist: &Zobrist, move_str: &str) -> Board {
        let (from_str, dest_str) = move_str.split_at(2);
        let from = Square::from_str(from_str).unwrap();
        let dest = Square::from_str(dest_str).unwrap();
        let moves: [Move; 256] = [Move::default(); 256];
        let mut moves = ArrayVec::from(moves);
        moves.set_len(0);
        board.generate(&mut moves);
        for m in moves {
            if m.from == from && m.dest == dest {
                return board.make(m, zobrist);
            }
        }
        unreachable!("Should never hit this under testing conditions");
    }

    // Helper to take a board and compute the hash freshly
    fn fresh_hash(board: &Board, zobrist: &Zobrist) -> u64 {
        // Have to clone to get mutable board
        let mut cloned = board.clone();
        cloned.recalculate_hash(zobrist);
        cloned.hash
    }

    // Check that incrementally computing a Zobrist hash results in the same value as a freshly
    // computed hash
    #[test]
    fn incremental_zobrist() {
        // Compare a set of bad moves generated from earlier (current as of this comment's writing) versions of Yukari
        // Generating a board from a FEN notation computes the hash directly. It should always match the incremental
        // version computed directly
        let zobrist = Zobrist::new();
        let mut board =
            Board::from_fen("8/k7/3p4/p2P1p2/P2P1P2/8/8/K7 w - - 0 1", &zobrist).unwrap();
        // Moves to test
        let moves = ["a1b1", "a7a6", "b1a1", "a6b6", "a1b1", "b6a6"];
        // Make each move
        for (i, &m) in moves.iter().enumerate() {
            board = make_move(&board, &zobrist, m);
            assert_eq!(
                board.hash,
                fresh_hash(&board, &zobrist),
                "Failed testing move #{} ({})",
                i,
                m
            );
        }
    }

    // Test that making and unmaking a move has the same hash before and after
    #[test]
    fn make_unmake() {
        let zobrist = Zobrist::new();
        let mut board =
            Board::from_fen("8/k7/3p4/p2P1p2/P2P1P2/8/8/K7 w - - 0 1", &zobrist).unwrap();
        // This hash will always be the same between incremental and non-incremental because it's been computed directly
        let initial_hash = board.hash;
        // Now make the test move
        board = make_move(&board, &zobrist, "a1b1");
        // Allows us to flip side back without making a move
        board = board.make_null(&zobrist);
        // Option for dev to test that it's the same between both incremental and non
        //assert_eq!(board.hash, fresh_hash(&board, &zobrist), "Made move differs between incremental and fresh");
        // Unmake the move
        board = make_move(&board, &zobrist, "b1a1");
        // Unmake the side swap hash break
        board = board.make_null(&zobrist);
        // Check that it's the same hash
        assert_eq!(
            board.hash, initial_hash,
            "Incremental hash differs between original and unmade"
        );
        // Allow testing if a fresh hash would match
        assert_eq!(
            fresh_hash(&board, &zobrist),
            initial_hash,
            "Freshly computed hash differs between original and unmade"
        );
    }
}
/* impl Drop for Board {
    fn drop(&mut self) {
        if ::std::thread::panicking() {
            println!("{}", self);
        }
    }
} */
