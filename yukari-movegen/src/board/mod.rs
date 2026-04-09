use std::{
    convert::{TryFrom, TryInto},
    ffi::CString,
    fmt::{Display, Write},
};

use colored::Colorize;
use tinyvec::ArrayVec;
pub use zobrist::Zobrist;

use crate::{
    chessmove::{Move, MoveType},
    colour::Colour,
    piece::Piece,
    square::{File, Rank, Square, Square16x8},
};

mod bitlist;
mod data;
mod eval;
mod feature;
mod index;
mod piecelist;
mod piecemask;
mod pins;
mod zobrist;

use bitlist::Bitlist;
use data::BoardData;
pub use index::PieceIndex;

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

            let square_colour = |s: &str| if (j & 1) ^ ((j >> 3) & 1) == 0 { s.on_green() } else { s.on_white() };

            if let (Some(piece), Some(colour)) = (
                self.data.piece_from_square(j.try_into().expect("square somehow out of bounds")),
                self.data.colour_from_square(j.try_into().expect("square somehow out of bounds")),
            ) {
                let c = match piece {
                    Piece::Pawn => '♙',
                    Piece::Knight => '♘',
                    Piece::Bishop => '♗',
                    Piece::Rook => '♖',
                    Piece::Queen => '♕',
                    Piece::King => '♔',
                };

                let c = if colour == Colour::White { c.to_string().bright_white() } else { c.to_string().black() };

                write!(f, "{}", square_colour(&format!("{c} ")))?;
            } else {
                write!(f, "{}", square_colour("  "))?;
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
            writeln!(f, "{ep}")?;
        } else {
            writeln!(f, "-")?;
        }

        writeln!(f, "{:016x}", self.hash())?;

        Ok(())
    }
}

impl Board {
    /// Create a new empty board.
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self { side: Colour::White, castle: (false, false, false, false), ep: None, data: BoardData::new() }
    }

    #[allow(clippy::missing_panics_doc)]
    #[must_use]
    pub fn startpos() -> Self {
        Self::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap()
    }

    #[must_use]
    pub const fn data(&self) -> &BoardData {
        &self.data
    }

    #[must_use]
    pub const fn castle(&self) -> &(bool, bool, bool, bool) {
        &self.castle
    }

    /// Check if this board is illegal by seeing if the enemy king is attacked by friendly pieces.
    /// If it is, it implies the move the enemy made left them in check, which is illegal.
    #[must_use]
    #[inline]
    pub fn illegal(&self) -> bool {
        // A valid chessboard has a white king and a black king.
        if (self.data().piecemask().kings() & Bitlist::white()).empty() {
            return true;
        }
        if (self.data().piecemask().kings() & Bitlist::black()).empty() {
            return true;
        }
        // The opponent's king should not be in check.
        if !self.data.attacks_to(self.data.king_square(!self.side), self.side).empty() {
            return true;
        }
        false
    }

    /// Parse a position in Forsyth-Edwards Notation into a board.
    ///
    /// # Panics
    /// Panics if `fen` is not ASCII.
    #[must_use]
    pub fn from_fen(fen: &str) -> Option<Self> {
        let fen = CString::new(fen).expect("FEN is not ASCII");
        let fen = fen.as_bytes();
        Self::from_fen_bytes(fen)
    }

    /// Parse a position in Forsyth-Edwards Notation into a board.
    ///
    /// # Panics
    /// Panics when invalid FEN is input.
    #[must_use]
    pub fn from_fen_bytes(fen: &[u8]) -> Option<Self> {
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

                    let colour = if c.is_ascii_uppercase() { Colour::White } else { Colour::Black };

                    let square = Square::from_rank_file(rank.try_into().unwrap(), file.try_into().unwrap());

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
                b.data.add_castling(0);
                idx += 1;
                c = fen[idx];
            }
            if c == b'Q' {
                b.castle.1 = true;
                b.data.add_castling(1);
                idx += 1;
                c = fen[idx];
            }
            if c == b'k' {
                b.castle.2 = true;
                b.data.add_castling(2);
                idx += 1;
                c = fen[idx];
            }
            if c == b'q' {
                b.castle.3 = true;
                b.data.add_castling(3);
                idx += 1;
            }
        }
        idx += 1;
        c = fen[idx];
        b.ep = None;
        if c != b'-' {
            let file = File::try_from(c - b'a').unwrap();
            idx += 1;
            c = fen[idx];
            let rank = Rank::try_from(c - b'1').unwrap();
            b.set_ep(Some(Square::from_rank_file(rank, file)));
        }

        b.data.rebuild_attacks();
        b.data.rebuild_accumulators();

        if b.side == Colour::Black {
            b.data.toggle_side();
        }

        if b.illegal() {
            return None;
        }

        Some(b)
    }

    fn set_ep(&mut self, ep: Option<Square>) {
        self.data.set_ep(self.ep, ep);
        self.ep = ep;
    }

    /// Make a move on the board.
    ///
    /// # Panics
    /// Panics when Lofty hasn't implemented necessary code.
    #[inline]
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn make(&self, m: Move) -> Self {
        let mut b = self.clone();
        match m.kind() {
            MoveType::PromotionKnight | MoveType::PromotionBishop | MoveType::PromotionRook | MoveType::PromotionQueen | MoveType::Normal | MoveType::DoublePush | MoveType::_Unused1 | MoveType::_Unused2 => {}
            MoveType::Capture | MoveType::CapturePromotionKnight | MoveType::CapturePromotionBishop | MoveType::CapturePromotionRook | MoveType::CapturePromotionQueen => {
                let piece_index =
                    b.data.piece_index(m.dest()).unwrap_or_else(|| panic!("move {m} attempts to capture an empty square"));
                b.data.remove_piece(piece_index, true);
            }
            MoveType::KingsideCastle => {
                let (rook_from, rook_to) = (m.dest().east().unwrap(), m.dest().west().unwrap());
                b.data.move_piece(rook_from, rook_to);
            }
            MoveType::QueensideCastle => {
                let (rook_from, rook_to) = (m.dest().west().unwrap().west().unwrap(), m.dest().east().unwrap());
                b.data.move_piece(rook_from, rook_to);
            }
            MoveType::EnPassant => {
                let target_square = b.ep.unwrap().relative_south(b.side).unwrap();
                let target_piece = b.data.piece_index(target_square).unwrap();
                b.data.remove_piece(target_piece, true);
            }
        }

        b.data.move_piece(m.from(), m.dest());

        if m.is_promotion() {
            let piece_index = b.data.piece_index(m.dest()).unwrap();
            b.data.remove_piece(piece_index, true);
            b.data.add_piece(m.promotion_piece().unwrap(), b.side, m.dest(), true);
        }

        let candidate_ep = (|| {
            let MoveType::DoublePush = m.kind() else { return None };
            let candidate_ep = m.from().relative_north(b.side)?;
            let attacks = b.data().attacks_to(candidate_ep, !b.side());
            if (attacks & b.data().piecemask().pawns()).empty() {
                return None;
            }
            Some(candidate_ep)
        })();
        b.set_ep(candidate_ep);

        let a1 = Square::from_rank_file(Rank::One, File::A);
        let a8 = Square::from_rank_file(Rank::Eight, File::A);
        let e1 = Square::from_rank_file(Rank::One, File::E);
        let e8 = Square::from_rank_file(Rank::Eight, File::E);
        let h1 = Square::from_rank_file(Rank::One, File::H);
        let h8 = Square::from_rank_file(Rank::Eight, File::H);

        if m.from() == e1 {
            if b.castle.0 {
                b.castle.0 = false;
                b.data.remove_castling(0);
            }
            if b.castle.1 {
                b.castle.1 = false;
                b.data.remove_castling(1);
            }
        }

        if m.from() == e8 {
            if b.castle.2 {
                b.castle.2 = false;
                b.data.remove_castling(2);
            }
            if b.castle.3 {
                b.castle.3 = false;
                b.data.remove_castling(3);
            }
        }

        if (m.from() == h1 || m.dest() == h1) && b.castle.0 {
            b.castle.0 = false;
            b.data.remove_castling(0);
        }

        if (m.from() == a1 || m.dest() == a1) && b.castle.1 {
            b.castle.1 = false;
            b.data.remove_castling(1);
        }

        if (m.from() == h8 || m.dest() == h8) && b.castle.2 {
            b.castle.2 = false;
            b.data.remove_castling(2);
        }

        if (m.from() == a8 || m.dest() == a8) && b.castle.3 {
            b.castle.3 = false;
            b.data.remove_castling(3);
        }

        b.side = !b.side;
        b.data.toggle_side();
        b
    }

    fn try_push_move(
        &self, v: &mut ArrayVec<[Move; 256]>, from: Square, dest: Square, kind: MoveType,
        pininfo: &pins::PinInfo,
    ) {
        if let Some(dir) = pininfo.pins[self.data.piece_index(from).unwrap().into_inner() as usize] {
            let Some(move_dir) = from.direction(dest) else {
                // Pinned knight can't move.
                return;
            };
            // Pinned slider can only move along pin ray.
            if dir != move_dir && dir != move_dir.opposite() {
                return;
            }
        }
        v.push(Move::new(from, dest, kind));
    }

    /// Generate en-passant pawn moves.
    fn generate_pawn_enpassant(&self, v: &mut ArrayVec<[Move; 256]>, pininfo: &pins::PinInfo) {
        let Some(ep) = self.ep else {
            return;
        };
        for capturer in self.data.attacks_to(ep, self.side).and(self.data.piecemask().pawns()).and(!pininfo.enpassant_pinned) {
            let from = self.data.square_of_piece(capturer);
            self.try_push_move(v, from, ep, MoveType::EnPassant, pininfo);
        }
    }

    /// Generate pawn-specific quiet moves.
    fn generate_pawn_quiet(&self, v: &mut ArrayVec<[Move; 256]>, from: Square, pininfo: &pins::PinInfo) {
        let north = from.relative_north(self.side);
        let Some(dest) = north else {
            return;
        };
        // Pawn single pushes.
        if self.data.has_piece(dest) {
            return;
        }
        if Rank::from(dest).is_relative_eighth(self.side) {
            self.try_push_move(v, from, dest, MoveType::PromotionQueen, pininfo);
            self.try_push_move(v, from, dest, MoveType::PromotionKnight, pininfo);
            self.try_push_move(v, from, dest, MoveType::PromotionRook, pininfo);
            self.try_push_move(v, from, dest, MoveType::PromotionBishop, pininfo);
        } else {
            self.try_push_move(v, from, dest, MoveType::Normal, pininfo);
        }

        // Pawn double pushes.
        let Some(dest) = dest.relative_north(self.side) else {
            return;
        };
        if Rank::from(dest).is_relative_fourth(self.side) && !self.data.has_piece(dest) {
            self.try_push_move(v, from, dest, MoveType::DoublePush, pininfo);
        }
    }

    /// Generate moves when in check by a single piece.
    #[allow(clippy::too_many_lines)]
    fn generate_single_check(&self, v: &mut ArrayVec<[Move; 256]>) {
        let king_square = self.data.king_square(self.side);
        let king_square_16x8 = Square16x8::from_square(king_square);
        let attacker_bit = self.data.attacks_to(king_square, !self.side);
        let attacker_index = unsafe { attacker_bit.peek_nonzero() };
        let attacker_piece = self.data.piece_from_bit(attacker_index);
        let attacker_square = self.data.square_of_piece(attacker_index);
        let attacker_direction = attacker_square.direction(king_square);

        let pininfo = pins::PinInfo::discover(self);

        let add_pawn_block = |v: &mut ArrayVec<[Move; 256]>, from, dest, kind| {
            let Some(colour) = self.data.colour_from_square(from) else { return };
            if colour != self.side {
                return;
            }
            if !Rank::from(dest).is_relative_eighth(self.side) {
                self.try_push_move(v, from, dest, kind, &pininfo);
                return;
            }
            self.try_push_move(v, from, dest, MoveType::PromotionQueen, &pininfo);
            self.try_push_move(v, from, dest, MoveType::PromotionKnight, &pininfo);
            self.try_push_move(v, from, dest, MoveType::PromotionRook, &pininfo);
            self.try_push_move(v, from, dest, MoveType::PromotionBishop, &pininfo);
        };

        let add_pawn_blocks = |v: &mut ArrayVec<[Move; 256]>, dest: Square| {
            let Some(from) = dest.relative_south(self.side) else { return };
            match self.data.piece_from_square(from) {
                Some(Piece::Pawn) => add_pawn_block(v, from, dest, MoveType::Normal),
                Some(_) => {}
                None => {
                    if Rank::from(dest).is_relative_fourth(self.side) {
                        let Some(from) = from.relative_south(self.side) else { return };
                        if self.data.piece_from_square(from) == Some(Piece::Pawn) {
                            add_pawn_block(v, from, dest, MoveType::DoublePush);
                        }
                    }
                }
            }
        };

        // Can we capture the attacker?
        for capturer in self.data.attacks_to(attacker_square, self.side) {
            let from = self.data.square_of_piece(capturer);
            if self.data.piece_from_bit(capturer) == Piece::King && !self.data.attacks_to(attacker_square, !self.side).empty() {
                continue;
            }
            if self.data.piece_from_bit(capturer) != Piece::Pawn || !Rank::from(attacker_square).is_relative_eighth(self.side) {
                self.try_push_move(v, from, attacker_square, MoveType::Capture, &pininfo);
                continue;
            }
            self.try_push_move(v, from, attacker_square, MoveType::CapturePromotionQueen, &pininfo);
            self.try_push_move(v, from, attacker_square, MoveType::CapturePromotionKnight, &pininfo);
            self.try_push_move(v, from, attacker_square, MoveType::CapturePromotionRook, &pininfo);
            self.try_push_move(v, from, attacker_square, MoveType::CapturePromotionBishop, &pininfo);
        }

        // en-passant
        (|| {
            let Some(ep) = self.ep else { return };
            let Some(ep_south) = ep.relative_south(self.side) else { return };
            if ep_south != attacker_square || attacker_piece != Piece::Pawn {
                return;
            }
            for capturer in self.data.attacks_to(ep, self.side) & self.data.piecemask().pawns() & !pininfo.enpassant_pinned {
                self.try_push_move(v, self.data.square_of_piece(capturer), ep, MoveType::EnPassant, &pininfo);
            }
        })();

        // Can we block the check?
        if let Piece::Bishop | Piece::Rook | Piece::Queen = attacker_piece {
            let direction = king_square.direction(attacker_square).unwrap();
            for dest in king_square_16x8.ray_attacks(direction) {
                if dest == attacker_square {
                    break;
                }

                // Piece moves.
                for attacker in
                    self.data.attacks_to(dest, self.side).and(!self.data.piecemask().pawns()).and(!self.data.piecemask().kings())
                {
                    self.try_push_move(v, self.data.square_of_piece(attacker), dest, MoveType::Normal, &pininfo);
                }

                // Pawn moves.
                add_pawn_blocks(v, dest);
            }
        }

        // Can we move the king?
        for square in king_square.king_attacks() {
            let kind = if self.data.has_piece(square) {
                if square == attacker_square || self.data.colour_from_square(square) == Some(self.side) {
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
                if let Some(xray_square) = king_square.travel(attacker_direction)
                    && matches!(attacker_piece, Piece::Bishop | Piece::Rook | Piece::Queen)
                    && xray_square == square
                {
                    continue;
                }
            }

            v.push(Move::new(king_square, square, kind));
        }
    }

    fn generate_double_check(&self, v: &mut ArrayVec<[Move; 256]>) {
        let king_square = self.data.king_square(self.side);
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
            if let Some(attacker1_direction) = attacker1_direction
                && let Some(xray_square) = king_square.travel(attacker1_direction)
                && matches!(attacker1_piece, Piece::Bishop | Piece::Rook | Piece::Queen)
                && xray_square == square
            {
                continue;
            }

            if let Some(attacker2_direction) = attacker2_direction
                && let Some(xray_square) = king_square.travel(attacker2_direction)
                && matches!(attacker2_piece, Piece::Bishop | Piece::Rook | Piece::Queen)
                && xray_square == square
            {
                continue;
            }

            v.push(Move::new(king_square, square, kind));
        }
    }

    pub fn generate_captures(&self, v: &mut ArrayVec<[Move; 256]>) {
        let pininfo = pins::PinInfo::discover(self);

        let mut find_attackers = |dest: Square| {
            let attacks = self.data.attacks_to(dest, self.side);
            for capturer in attacks & self.data.piecemask().pawns() {
                let from = self.data.square_of_piece(capturer);
                if Rank::from(dest).is_relative_eighth(self.side) {
                    self.try_push_move(v, from, dest, MoveType::CapturePromotionQueen, &pininfo);
                    self.try_push_move(v, from, dest, MoveType::CapturePromotionKnight, &pininfo);
                    self.try_push_move(v, from, dest, MoveType::CapturePromotionRook, &pininfo);
                    self.try_push_move(v, from, dest, MoveType::CapturePromotionBishop, &pininfo);
                } else {
                    self.try_push_move(v, from, dest, MoveType::Capture, &pininfo);
                }
            }
            let capturers = (attacks & self.data.piecemask().knights())
                .into_iter()
                .chain(attacks & self.data.piecemask().bishops())
                .chain(attacks & self.data.piecemask().rooks())
                .chain(attacks & self.data.piecemask().queens());

            for capturer in capturers {
                let from = self.data.square_of_piece(capturer);
                self.try_push_move(v, from, dest, MoveType::Capture, &pininfo);
            }
            for capturer in attacks & self.data.piecemask().kings() {
                let from = self.data.square_of_piece(capturer);
                if !self.data.attacks_to(dest, !self.side).empty() {
                    // Moving into check is illegal.
                    continue;
                }
                self.try_push_move(v, from, dest, MoveType::Capture, &pininfo);
            }
        };

        let victims = (self.data.piecemask().pieces_of_colour(!self.side) & self.data.piecemask().queens())
            .into_iter()
            .chain(self.data.piecemask().pieces_of_colour(!self.side) & self.data.piecemask().rooks())
            .chain(self.data.piecemask().pieces_of_colour(!self.side) & self.data.piecemask().bishops())
            .chain(self.data.piecemask().pieces_of_colour(!self.side) & self.data.piecemask().knights())
            .chain(self.data.piecemask().pieces_of_colour(!self.side) & self.data.piecemask().pawns());

        for victim in victims {
            find_attackers(self.square_of_piece(victim));
        }

        self.generate_pawn_enpassant(v, &pininfo);
    }

    pub fn generate_quiesce(&self, v: &mut ArrayVec<[Move; 256]>) {
        let king_square = self.data.king_square(self.side);
        let checks = self.data.attacks_to(king_square, !self.side);

        // special case: being in check.
        if checks.count_ones() != 0 {
            let mut v2 = ArrayVec::new();
            v2.set_len(0);
            if checks.count_ones() == 1 {
                self.generate_single_check(&mut v2);
            } else if checks.count_ones() == 2 {
                self.generate_double_check(&mut v2);
            }

            for m in v2 {
                if m.is_capture() {
                    v.push(m);
                }
            }
            return;
        }

        self.generate_captures(v);
    }

    /// Generate a vector of moves on the board.
    ///
    /// # Panics
    /// Panics when Lofty writes shitty code.
    #[allow(clippy::missing_inline_in_public_items)]
    pub fn generate(&self, v: &mut ArrayVec<[Move; 256]>) {
        // Unless something has gone very badly wrong we have to have a king.
        let king_square = self.data.king_square(self.side);
        let checks = self.data.attacks_to(king_square, !self.side);

        if checks.count_ones() == 1 {
            return self.generate_single_check(v);
        }
        if checks.count_ones() == 2 {
            return self.generate_double_check(v);
        }

        let pininfo = pins::PinInfo::discover(self);
        self.generate_captures(v);

        // Pawns.
        for pawn in self.data.piecemask().pawns().and(Bitlist::mask_from_colour(self.side)) {
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
            for attacker in self.data.attacks_to(dest, self.side).and(!self.data.piecemask().pawns()) {
                // It's illegal for kings to move to attacked squares; prune those out.
                if self.data.piece_from_bit(attacker) == Piece::King && !self.data.attacks_to(dest, !self.side).empty() {
                    continue;
                }

                let from = self.data.square_of_piece(attacker);
                self.try_push_move(v, from, dest, MoveType::Normal, &pininfo);
            }
        }

        // Kingside castling.
        if (self.side == Colour::White && self.castle.0) || (self.side == Colour::Black && self.castle.2) {
            let east1 = king_square.east().unwrap();
            let east2 = east1.east().unwrap();
            if self.data.attacks_to(king_square, !self.side).empty()
                && !self.data.has_piece(east1)
                && self.data.attacks_to(east1, !self.side).empty()
                && !self.data.has_piece(east2)
                && self.data.attacks_to(east2, !self.side).empty()
            {
                self.try_push_move(v, king_square, east2, MoveType::KingsideCastle, &pininfo);
            }
        }

        // Queenside castling.
        if (self.side == Colour::White && self.castle.1) || (self.side == Colour::Black && self.castle.3) {
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
                self.try_push_move(v, king_square, west2, MoveType::QueensideCastle, &pininfo);
            }
        }
    }

    #[must_use]
    #[allow(clippy::too_many_lines, clippy::missing_panics_doc)]
    pub fn static_exchange_evaluation(&self, m: Move) -> i32 {
        let mut our_attacks = self.data.attacks_to(m.dest(), self.side());
        let mut their_attacks = self.data.attacks_to(m.dest(), !self.side());
        let mut moved_pieces = Bitlist::new();

        let add_xrays = |mover_square: Square, our_attacks: &mut Bitlist, their_attacks: &mut Bitlist, moved_pieces: &Bitlist| {
            let mover_bitlist =
                self.data.attacks_to(mover_square, Colour::White) | self.data.attacks_to(mover_square, Colour::Black);
            let mover_bitlist =
                mover_bitlist & (self.data.piecemask().bishops() | self.data.piecemask().rooks() | self.data.piecemask().queens());
            let mover_bitlist = mover_bitlist & moved_pieces.invert();

            let target_square_16x8 = Square16x8::from_square(m.dest());
            let mover_square_16x8 = Square16x8::from_square(mover_square);
            let Some(direction) = mover_square_16x8.direction(target_square_16x8) else { return };

            for candidate in mover_bitlist {
                let candidate_square = self.square_of_piece(candidate);
                let candidate_square_16x8 = Square16x8::from_square(candidate_square);
                let candidate_type = self.data.piece_from_bit(candidate);
                let candidate_colour = candidate.colour();
                let Some(candidate_direction) = candidate_square_16x8.direction(target_square_16x8) else {
                    continue;
                };
                if !candidate_direction.valid_for_slider(candidate_type) {
                    continue;
                }
                if candidate_direction != direction {
                    continue;
                }

                if candidate_colour == self.side() {
                    *our_attacks |= Bitlist::from_piece(candidate);
                } else {
                    *their_attacks |= Bitlist::from_piece(candidate);
                }

                break;
            }
        };

        let next_piece = |bitlist: Bitlist,
                          our_attacks: &mut Bitlist,
                          their_attacks: &mut Bitlist,
                          moved_pieces: &mut Bitlist|
         -> Option<Piece> {
            let bitlist = bitlist & moved_pieces.invert();
            if let Some(piece) = (bitlist & self.data.piecemask().pawns()).peek() {
                let not_piece = Bitlist::from_piece(piece).invert();
                *our_attacks &= not_piece;
                *their_attacks &= not_piece;
                *moved_pieces |= Bitlist::from_piece(piece);
                add_xrays(self.square_of_piece(piece), our_attacks, their_attacks, moved_pieces);
                return Some(Piece::Pawn);
            }
            if let Some(piece) = (bitlist & self.data.piecemask().knights()).peek() {
                let not_piece = Bitlist::from_piece(piece).invert();
                *our_attacks &= not_piece;
                *their_attacks &= not_piece;
                *moved_pieces |= Bitlist::from_piece(piece);
                return Some(Piece::Knight);
            }
            if let Some(piece) = (bitlist & self.data.piecemask().bishops()).peek() {
                let not_piece = Bitlist::from_piece(piece).invert();
                *our_attacks &= not_piece;
                *their_attacks &= not_piece;
                *moved_pieces |= Bitlist::from_piece(piece);
                add_xrays(self.square_of_piece(piece), our_attacks, their_attacks, moved_pieces);
                return Some(Piece::Bishop);
            }
            if let Some(piece) = (bitlist & self.data.piecemask().rooks()).peek() {
                let not_piece = Bitlist::from_piece(piece).invert();
                *our_attacks &= not_piece;
                *their_attacks &= not_piece;
                *moved_pieces |= Bitlist::from_piece(piece);
                add_xrays(self.square_of_piece(piece), our_attacks, their_attacks, moved_pieces);
                return Some(Piece::Rook);
            }
            if let Some(piece) = (bitlist & self.data.piecemask().queens()).peek() {
                let not_piece = Bitlist::from_piece(piece).invert();
                *our_attacks &= not_piece;
                *their_attacks &= not_piece;
                *moved_pieces |= Bitlist::from_piece(piece);
                add_xrays(self.square_of_piece(piece), our_attacks, their_attacks, moved_pieces);
                return Some(Piece::Queen);
            }
            if let Some(piece) = (bitlist & self.data.piecemask().kings()).peek() {
                let not_piece = Bitlist::from_piece(piece).invert();
                *our_attacks &= not_piece;
                *their_attacks &= not_piece;
                *moved_pieces |= Bitlist::from_piece(piece);
                return Some(Piece::King);
            }
            None
        };

        let piece_value = |piece: Option<Piece>| {
            static PIECE_VALUES: [i32; 6] = [1, 3, 3, 5, 9, 100];
            let Some(piece) = piece else {
                return 0;
            };
            PIECE_VALUES[piece as usize]
        };

        our_attacks &= Bitlist::from_piece(self.data.piece_index(m.from()).unwrap()).invert();
        moved_pieces |= Bitlist::from_piece(self.data.piece_index(m.from()).unwrap());
        add_xrays(m.from(), &mut our_attacks, &mut their_attacks, &moved_pieces);

        let mut victim = self.piece_from_square(m.dest());
        let mut attacker = self.piece_from_square(m.from());
        let mut score = if m.kind() == MoveType::EnPassant { 1 } else { piece_value(victim) };

        if let Some(prom) = m.promotion_piece() {
            score += piece_value(Some(prom)) - piece_value(Some(Piece::Pawn));
            attacker = Some(prom);
        }

        let mut alpha = -1000;
        let mut beta = score;

        loop {
            victim = attacker;
            attacker = next_piece(their_attacks, &mut our_attacks, &mut their_attacks, &mut moved_pieces);
            if attacker.is_none() {
                score = beta;
                break;
            }

            score -= piece_value(victim);
            if score >= beta {
                score = beta;
                break;
            }
            alpha = alpha.max(score);

            victim = attacker;
            attacker = next_piece(our_attacks, &mut our_attacks, &mut their_attacks, &mut moved_pieces);
            if attacker.is_none() {
                score = alpha;
                break;
            }

            score += piece_value(victim);
            if score <= alpha {
                score = alpha;
                break;
            }
            beta = beta.min(score);
        }

        score
    }

    /// Given a piece index, return its piece type.
    #[must_use]
    pub const fn piece_from_bit(&self, bit: PieceIndex) -> Piece {
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
        self.data.hash()
    }

    #[inline]
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn hash_after(&self, m: Move) -> u64 {
        let mut hash = self.hash();

        match m.kind() {
            MoveType::PromotionKnight | MoveType::PromotionBishop | MoveType::PromotionRook | MoveType::PromotionQueen | MoveType::Normal | MoveType::DoublePush | MoveType::_Unused1 | MoveType::_Unused2 => {}
            MoveType::Capture | MoveType::CapturePromotionKnight | MoveType::CapturePromotionBishop | MoveType::CapturePromotionRook | MoveType::CapturePromotionQueen => {
                let piece_index =
                    self.data.piece_index(m.dest()).unwrap_or_else(|| panic!("move {m} attempts to capture an empty square"));
                Zobrist::remove_piece(piece_index.colour(), self.data.piece_from_bit(piece_index), self.data.square_of_piece(piece_index), &mut hash);
            }
            MoveType::KingsideCastle => {
                let (rook_from, rook_to) = (m.dest().east().unwrap(), m.dest().west().unwrap());
                let piece_index = self.data.piece_index(rook_from).unwrap();
                Zobrist::move_piece(piece_index.colour(), self.data.piece_from_bit(piece_index), rook_from, rook_to, &mut hash);
            }
            MoveType::QueensideCastle => {
                let (rook_from, rook_to) = (m.dest().west().unwrap().west().unwrap(), m.dest().east().unwrap());
                let piece_index = self.data.piece_index(rook_from).unwrap();
                Zobrist::move_piece(piece_index.colour(), self.data.piece_from_bit(piece_index), rook_from, rook_to, &mut hash);
            }
            MoveType::EnPassant => {
                let target_square = self.ep.unwrap().relative_south(self.side).unwrap();
                let target_piece = self.data.piece_index(target_square).unwrap();
                Zobrist::remove_piece(target_piece.colour(), self.data.piece_from_bit(target_piece), self.data.square_of_piece(target_piece), &mut hash);
            }
        }

        let piece_index = self.data.piece_index(m.from()).unwrap();
        Zobrist::move_piece(self.side, self.piece_from_bit(piece_index), m.from(), m.dest(), &mut hash);

        if m.is_promotion() {
            Zobrist::remove_piece(self.side, self.data.piece_from_bit(piece_index), m.dest(), &mut hash);
            Zobrist::add_piece(self.side, m.promotion_piece().unwrap(), m.dest(), &mut hash);
        }

        let candidate_ep = (|| {
            let MoveType::DoublePush = m.kind() else { return None };
            let candidate_ep = m.from().relative_north(self.side)?;
            let attacks = self.data().attacks_to(candidate_ep, !self.side());
            if (attacks & self.data().piecemask().pawns()).empty() {
                return None;
            }
            Some(candidate_ep)
        })();
        Zobrist::set_ep(self.ep, candidate_ep, &mut hash);

        let a1 = Square::from_rank_file(Rank::One, File::A);
        let a8 = Square::from_rank_file(Rank::Eight, File::A);
        let e1 = Square::from_rank_file(Rank::One, File::E);
        let e8 = Square::from_rank_file(Rank::Eight, File::E);
        let h1 = Square::from_rank_file(Rank::One, File::H);
        let h8 = Square::from_rank_file(Rank::Eight, File::H);

        if m.from() == e1 {
            if self.castle.0 {
                Zobrist::remove_castling(0, &mut hash);
            }
            if self.castle.1 {
                Zobrist::remove_castling(1, &mut hash);
            }
        }

        if m.from() == e8 {
            if self.castle.2 {
                Zobrist::remove_castling(2, &mut hash);
            }
            if self.castle.3 {
                Zobrist::remove_castling(3, &mut hash);
            }
        }

        if (m.from() == h1 || m.dest() == h1) && self.castle.0 {
            Zobrist::remove_castling(0, &mut hash);
        }

        if (m.from() == a1 || m.dest() == a1) && self.castle.1 {
            Zobrist::remove_castling(1, &mut hash);
        }

        if (m.from() == h8 || m.dest() == h8) && self.castle.2 {
            Zobrist::remove_castling(2, &mut hash);
        }

        if (m.from() == a8 || m.dest() == a8) && self.castle.3 {
            Zobrist::remove_castling(3, &mut hash);
        }

        Zobrist::toggle_side(&mut hash);
        hash
    }

    #[must_use]
    pub fn hash_pawns(&self) -> u64 {
        self.data.hash_pawns()
    }

    #[must_use]
    pub fn eval(&self, colour: Colour) -> i32 {
        self.data.eval(colour)
    }

    #[must_use]
    pub fn in_check(&self) -> bool {
        !self.data.attacks_to(self.data.king_square(self.side), !self.side).empty()
    }

    #[must_use]
    pub fn make_null(&self) -> Self {
        let mut board = self.clone();
        board.side = !board.side;
        board.set_ep(None);
        board.data.toggle_side();
        board
    }

    /// # Panics
    /// Panics when a nonsense move is encountered.
    #[must_use]
    pub fn to_san(&self, m: Move) -> String {
        let mut san = String::new();

        // Special case: castling
        if m.kind() == MoveType::KingsideCastle {
            write!(san, "O-O").unwrap();
            return san;
        }
        if m.kind() == MoveType::QueensideCastle {
            write!(san, "O-O-O").unwrap();
            return san;
        }

        // Moving piece
        let piece = self.piece_from_square(m.from()).unwrap_or_else(|| panic!("{m} has no origin piece on board\n{self}"));
        let piece_char = match piece {
            Piece::Pawn => "",
            Piece::Knight => "N",
            Piece::Bishop => "B",
            Piece::Rook => "R",
            Piece::Queen => "Q",
            Piece::King => "K",
        };
        write!(san, "{piece_char}").unwrap();

        // Disambiguation
        let mut moves = ArrayVec::new();
        self.generate(&mut moves);

        let mut ambiguities = Vec::new();
        for mv in moves {
            if mv.dest() == m.dest() && self.piece_from_square(mv.from()) == self.piece_from_square(m.from()) && mv.from() != m.from() {
                ambiguities.push(mv);
            }
        }

        let must_disambiguate = !ambiguities.is_empty();
        let mut piece_on_same_rank = false;
        let mut piece_on_same_file = false;

        let rank = Rank::from(m.from());
        let file = File::from(m.from());
        for ambiguity in ambiguities {
            let attacker_rank = Rank::from(ambiguity.from());
            let attacker_file = File::from(ambiguity.from());
            piece_on_same_rank |= attacker_rank == rank;
            piece_on_same_file |= attacker_file == file;
        }

        if piece != Piece::Pawn && must_disambiguate {
            if piece_on_same_rank || !piece_on_same_file {
                write!(san, "{file}").unwrap();
            }
            if piece_on_same_file {
                write!(san, "{rank}").unwrap();
            }
        }

        // Capture?
        if m.is_capture() {
            // Pawns always have their file.
            if piece == Piece::Pawn {
                write!(san, "{file}").unwrap();
            }
            write!(san, "x").unwrap();
        }

        let rank = Rank::from(m.dest());
        let file = File::from(m.dest());
        write!(san, "{file}{rank}").unwrap();

        // Promotion?
        if m.is_promotion() {
            let piece_char = match m.promotion_piece().unwrap() {
                Piece::Pawn => 'P',
                Piece::Knight => 'N',
                Piece::Bishop => 'B',
                Piece::Rook => 'R',
                Piece::Queen => 'Q',
                Piece::King => 'K',
            };
            write!(san, "={piece_char}").unwrap();
        }

        // Check?
        let child = self.make(m);
        if child.in_check() {
            // Checkmate?
            let mut moves = ArrayVec::new();
            child.generate(&mut moves);
            if moves.is_empty() {
                write!(san, "#").unwrap();
            } else {
                write!(san, "+").unwrap();
            }
        }

        san
    }

    #[must_use]
    pub fn pv_to_san(&self, pv: &[Move]) -> String {
        let mut s = String::new();
        let mut board = self.clone();
        for &m in pv {
            let san = board.to_san(m);
            if board.side() == Colour::White {
                write!(s, "{} ", san.bright_white()).unwrap();
            } else {
                write!(s, "{} ", san.bright_black()).unwrap();
            }
            board = board.make(m);
        }
        s
    }

    #[must_use]
    pub fn insufficient_material(&self) -> bool {
        let white_count = (self.data.piecemask().occupied() & Bitlist::mask_from_colour(Colour::White)).count_ones();
        let white_knight_count = (self.data.piecemask().knights() & Bitlist::mask_from_colour(Colour::White)).count_ones();
        let white_bishop_count = (self.data.piecemask().bishops() & Bitlist::mask_from_colour(Colour::White)).count_ones();

        let black_count = (self.data.piecemask().occupied() & Bitlist::mask_from_colour(Colour::Black)).count_ones();
        let black_knight_count = (self.data.piecemask().knights() & Bitlist::mask_from_colour(Colour::Black)).count_ones();
        let black_bishop_count = (self.data.piecemask().bishops() & Bitlist::mask_from_colour(Colour::Black)).count_ones();

        if white_count == 1 && black_count == 1 {
            return true;
        }

        if (white_count == 2 && (white_knight_count == 1 || white_bishop_count == 1) && black_count == 1)
            || (black_count == 2 && (black_knight_count == 1 || black_bishop_count == 1) && white_count == 1)
        {
            return true;
        }

        false
    }
}

/* impl Drop for Board {
    fn drop(&mut self) {
        if ::std::thread::panicking() {
            eprintln!("{}", self);
        }
    }
} */

mod tests {
    #[cfg(test)]
    use crate::Board;

    #[cfg(test)]
    fn find_move(board: &Board, cmd: &str) -> crate::Move {
        use std::str::FromStr;

        let from = crate::Square::from_str(&cmd[..2]).unwrap();
        let dest = crate::Square::from_str(&cmd[2..4]).unwrap();
        let prom = if cmd.len() == 5 {
            match cmd.chars().nth(4).unwrap() {
                'n' => Some(crate::Piece::Knight),
                'b' => Some(crate::Piece::Bishop),
                'r' => Some(crate::Piece::Rook),
                'q' => Some(crate::Piece::Queen),
                _ => None,
            }
        } else {
            None
        };
        let mut moves = tinyvec::ArrayVec::new();
        board.generate(&mut moves);
        moves.into_iter().find(|&m| m.from() == from && m.dest() == dest && m.promotion_piece() == prom).unwrap()
    }

    #[test]
    fn see_test0() {
        let board = Board::from_fen("6k1/1pp4p/p1pb4/6q1/3P1pRr/2P4P/PP1Br1P1/5RKN w - - ").unwrap();
        let m = find_move(&board, "f1f4");
        assert_eq!(board.static_exchange_evaluation(m), -1);
    }

    #[test]
    fn see_test1() {
        let board = Board::from_fen("5rk1/1pp2q1p/p1pb4/8/3P1NP1/2P5/1P1BQ1P1/5RK1 b - - ").unwrap();
        let m = find_move(&board, "d6f4");
        assert_eq!(board.static_exchange_evaluation(m), 0);
    }

    #[test]
    fn see_test2() {
        let board = Board::from_fen("4R3/2r3p1/5bk1/1p1r3p/p2PR1P1/P1BK1P2/1P6/8 b - - ").unwrap();
        let m = find_move(&board, "h5g4");
        assert_eq!(board.static_exchange_evaluation(m), 0);
    }

    #[test]
    fn see_test3() {
        let board = Board::from_fen("4R3/2r3p1/5bk1/1p1r1p1p/p2PR1P1/P1BK1P2/1P6/8 b - - ").unwrap();
        let m = find_move(&board, "h5g4");
        assert_eq!(board.static_exchange_evaluation(m), 0);
    }

    #[test]
    fn see_test4() {
        let board = Board::from_fen("4r1k1/5pp1/nbp4p/1p2p2q/1P2P1b1/1BP2N1P/1B2QPPK/3R4 b - - ").unwrap();
        let m = find_move(&board, "g4f3");
        assert_eq!(board.static_exchange_evaluation(m), 0);
    }

    #[test]
    fn see_test5() {
        let board = Board::from_fen("2r1r1k1/pp1bppbp/3p1np1/q3P3/2P2P2/1P2B3/P1N1B1PP/2RQ1RK1 b - - ").unwrap();
        let m = find_move(&board, "d6e5");
        assert_eq!(board.static_exchange_evaluation(m), 1);
    }

    #[test]
    fn see_test6() {
        let board = Board::from_fen("7r/5qpk/p1Qp1b1p/3r3n/BB3p2/5p2/P1P2P2/4RK1R w - - ").unwrap();
        let m = find_move(&board, "e1e8");
        assert_eq!(board.static_exchange_evaluation(m), 0);
    }

    #[test]
    fn see_test7() {
        let board = Board::from_fen("6rr/6pk/p1Qp1b1p/2n5/1B3p2/5p2/P1P2P2/4RK1R w - - ").unwrap();
        let m = find_move(&board, "e1e8");
        assert_eq!(board.static_exchange_evaluation(m), -5);
    }

    #[test]
    fn see_test8() {
        let board = Board::from_fen("7r/5qpk/2Qp1b1p/1N1r3n/BB3p2/5p2/P1P2P2/4RK1R w - - ").unwrap();
        let m = find_move(&board, "e1e8");
        assert_eq!(board.static_exchange_evaluation(m), -5);
    }

    #[test]
    fn see_test9() {
        let board = Board::from_fen("6RR/4bP2/8/8/5r2/3K4/5p2/4k3 w - - ").unwrap();
        let m = find_move(&board, "f7f8q");
        assert_eq!(board.static_exchange_evaluation(m), 2);
    }

    #[test]
    fn see_test10() {
        let board = Board::from_fen("6RR/4bP2/8/8/5r2/3K4/5p2/4k3 w - - ").unwrap();
        let m = find_move(&board, "f7f8n");
        assert_eq!(board.static_exchange_evaluation(m), 2);
    }

    #[test]
    fn see_test11() {
        let board = Board::from_fen("7R/5P2/8/8/6r1/3K4/5p2/4k3 w - - ").unwrap();
        let m = find_move(&board, "f7f8q");
        assert_eq!(board.static_exchange_evaluation(m), 8);
    }

    #[test]
    fn see_test12() {
        let board = Board::from_fen("7R/5P2/8/8/6r1/3K4/5p2/4k3 w - - ").unwrap();
        let m = find_move(&board, "f7f8b");
        assert_eq!(board.static_exchange_evaluation(m), 2);
    }

    #[test]
    fn see_test13() {
        let board = Board::from_fen("7R/4bP2/8/8/1q6/3K4/5p2/4k3 w - - ").unwrap();
        let m = find_move(&board, "f7f8r");
        assert_eq!(board.static_exchange_evaluation(m), -1);
    }

    #[test]
    fn see_test14() {
        let board = Board::from_fen("8/4kp2/2npp3/1Nn5/1p2PQP1/7q/1PP1B3/4KR1r b - - ").unwrap();
        let m = find_move(&board, "h1f1");
        assert_eq!(board.static_exchange_evaluation(m), 0);
    }

    #[test]
    fn see_test15() {
        let board = Board::from_fen("8/4kp2/2npp3/1Nn5/1p2P1P1/7q/1PP1B3/4KR1r b - - ").unwrap();
        let m = find_move(&board, "h1f1");
        assert_eq!(board.static_exchange_evaluation(m), 0);
    }

    #[test]
    fn see_test16() {
        let board = Board::from_fen("2r2r1k/6bp/p7/2q2p1Q/3PpP2/1B6/P5PP/2RR3K b - - ").unwrap();
        let m = find_move(&board, "c5c1");
        assert_eq!(board.static_exchange_evaluation(m), 1);
    }

    #[test]
    fn see_test17() {
        let board = Board::from_fen("r2qk1nr/pp2ppbp/2b3p1/2p1p3/8/2N2N2/PPPP1PPP/R1BQR1K1 w kq - ").unwrap();
        let m = find_move(&board, "f3e5");
        assert_eq!(board.static_exchange_evaluation(m), 1);
    }

    #[test]
    fn see_test18() {
        let board = Board::from_fen("6r1/4kq2/b2p1p2/p1pPb3/p1P2B1Q/2P4P/2B1R1P1/6K1 w - - ").unwrap();
        let m = find_move(&board, "f4e5");
        assert_eq!(board.static_exchange_evaluation(m), 0);
    }

    #[test]
    fn see_test19() {
        let board = Board::from_fen("3q2nk/pb1r1p2/np6/3P2Pp/2p1P3/2R4B/PQ3P1P/3R2K1 w - h6 ").unwrap();
        let m = find_move(&board, "g5h6");
        assert_eq!(board.static_exchange_evaluation(m), 0);
    }

    #[test]
    fn see_test20() {
        let board = Board::from_fen("3q2nk/pb1r1p2/np6/3P2Pp/2p1P3/2R1B2B/PQ3P1P/3R2K1 w - h6 ").unwrap();
        let m = find_move(&board, "g5h6");
        assert_eq!(board.static_exchange_evaluation(m), 1);
    }

    #[test]
    fn see_test21() {
        let board = Board::from_fen("2r4r/1P4pk/p2p1b1p/7n/BB3p2/2R2p2/P1P2P2/4RK2 w - - ").unwrap();
        let m = find_move(&board, "c3c8");
        assert_eq!(board.static_exchange_evaluation(m), 5);
    }

    #[test]
    fn see_test22() {
        let board = Board::from_fen("2r5/1P4pk/p2p1b1p/5b1n/BB3p2/2R2p2/P1P2P2/4RK2 w - - ").unwrap();
        let m = find_move(&board, "c3c8");
        assert_eq!(board.static_exchange_evaluation(m), 5);
    }

    #[test]
    fn see_test23() {
        let board = Board::from_fen("2r4k/2r4p/p7/2b2p1b/4pP2/1BR5/P1R3PP/2Q4K w - - ").unwrap();
        let m = find_move(&board, "c3c5");
        assert_eq!(board.static_exchange_evaluation(m), 3);
    }

    #[test]
    fn see_test24() {
        let board = Board::from_fen("8/pp6/2pkp3/4bp2/2R3b1/2P5/PP4B1/1K6 w - - ").unwrap();
        let m = find_move(&board, "g2c6");
        assert_eq!(board.static_exchange_evaluation(m), -2);
    }

    #[test]
    fn see_test25() {
        let board = Board::from_fen("4q3/1p1pr1k1/1B2rp2/6p1/p3PP2/P3R1P1/1P2R1K1/4Q3 b - - ").unwrap();
        let m = find_move(&board, "e6e4");
        assert_eq!(board.static_exchange_evaluation(m), -4);
    }

    #[test]
    fn see_test26() {
        let board = Board::from_fen("4q3/1p1pr1kb/1B2rp2/6p1/p3PP2/P3R1P1/1P2R1K1/4Q3 b - - ").unwrap();
        let m = find_move(&board, "h7e4");
        assert_eq!(board.static_exchange_evaluation(m), 1);
    }

    #[test]
    fn see_test27() {
        let board = Board::from_fen("3r3k/3r4/2n1n3/8/3p4/2PR4/1B1Q4/3R3K w - - ").unwrap();
        let m = find_move(&board, "d3d4");
        assert_eq!(board.static_exchange_evaluation(m), -1);
    }

    #[test]
    fn see_test28() {
        let board = Board::from_fen("1k1r4/1ppn3p/p4b2/4n3/8/P2N2P1/1PP1R1BP/2K1Q3 w - - ").unwrap();
        let m = find_move(&board, "d3e5");
        assert_eq!(board.static_exchange_evaluation(m), 1);
    }

    #[test]
    fn see_test29() {
        let board = Board::from_fen("1k1r3q/1ppn3p/p4b2/4p3/8/P2N2P1/1PP1R1BP/2K1Q3 w - - ").unwrap();
        let m = find_move(&board, "d3e5");
        assert_eq!(board.static_exchange_evaluation(m), -2);
    }

    #[test]
    fn see_test30() {
        let board = Board::from_fen("rnb2b1r/ppp2kpp/5n2/4P3/q2P3B/5R2/PPP2PPP/RN1QKB2 w Q - ").unwrap();
        let m = find_move(&board, "h4f6");
        assert_eq!(board.static_exchange_evaluation(m), 1);
    }

    #[test]
    fn see_test31() {
        let board = Board::from_fen("r2q1rk1/2p1bppp/p2p1n2/1p2P3/4P1b1/1nP1BN2/PP3PPP/RN1QR1K1 b - - ").unwrap();
        let m = find_move(&board, "g4f3");
        assert_eq!(board.static_exchange_evaluation(m), 0);
    }

    #[test]
    fn see_test32() {
        let board = Board::from_fen("r1bqkb1r/2pp1ppp/p1n5/1p2p3/3Pn3/1B3N2/PPP2PPP/RNBQ1RK1 b kq - ").unwrap();
        let m = find_move(&board, "c6d4");
        assert_eq!(board.static_exchange_evaluation(m), 0);
    }

    #[test]
    fn see_test33() {
        let board = Board::from_fen("r1bq1r2/pp1ppkbp/4N1p1/n3P1B1/8/2N5/PPP2PPP/R2QK2R w KQ - ").unwrap();
        let m = find_move(&board, "e6g7");
        assert_eq!(board.static_exchange_evaluation(m), 0);
    }

    #[test]
    fn see_test34() {
        let board = Board::from_fen("r1bq1r2/pp1ppkbp/4N1pB/n3P3/8/2N5/PPP2PPP/R2QK2R w KQ - ").unwrap();
        let m = find_move(&board, "e6g7");
        assert_eq!(board.static_exchange_evaluation(m), 3);
    }

    #[test]
    fn see_test35() {
        let board = Board::from_fen("rnq1k2r/1b3ppp/p2bpn2/1p1p4/3N4/1BN1P3/PPP2PPP/R1BQR1K1 b kq - ").unwrap();
        let m = find_move(&board, "d6h2");
        assert_eq!(board.static_exchange_evaluation(m), -2);
    }

    #[test]
    fn see_test36() {
        let board = Board::from_fen("rn2k2r/1bq2ppp/p2bpn2/1p1p4/3N4/1BN1P3/PPP2PPP/R1BQR1K1 b kq - ").unwrap();
        let m = find_move(&board, "d6h2");
        assert_eq!(board.static_exchange_evaluation(m), 1);
    }

    #[test]
    fn see_test37() {
        let board = Board::from_fen("r2qkbn1/ppp1pp1p/3p1rp1/3Pn3/4P1b1/2N2N2/PPP2PPP/R1BQKB1R b KQq - ").unwrap();
        let m = find_move(&board, "g4f3");
        assert_eq!(board.static_exchange_evaluation(m), 1);
    }

    #[test]
    fn see_test38() {
        let board = Board::from_fen("rnbq1rk1/pppp1ppp/4pn2/8/1bPP4/P1N5/1PQ1PPPP/R1B1KBNR b KQ - ").unwrap();
        let m = find_move(&board, "b4c3");
        assert_eq!(board.static_exchange_evaluation(m), 0);
    }

    #[test]
    fn see_test39() {
        let board = Board::from_fen("r4rk1/3nppbp/bq1p1np1/2pP4/8/2N2NPP/PP2PPB1/R1BQR1K1 b - - ").unwrap();
        let m = find_move(&board, "b6b2");
        assert_eq!(board.static_exchange_evaluation(m), -8);
    }

    #[test]
    fn see_test40() {
        let board = Board::from_fen("r4rk1/1q1nppbp/b2p1np1/2pP4/8/2N2NPP/PP2PPB1/R1BQR1K1 b - - ").unwrap();
        let m = find_move(&board, "f6d5");
        assert_eq!(board.static_exchange_evaluation(m), -2);
    }

    #[test]
    fn see_test41() {
        let board = Board::from_fen("1r3r2/5p2/4p2p/2k1n1P1/2PN1nP1/1P3P2/8/2KR1B1R b - - ").unwrap();
        let m = find_move(&board, "b8b3");
        assert_eq!(board.static_exchange_evaluation(m), -4);
    }

    #[test]
    fn see_test42() {
        let board = Board::from_fen("1r3r2/5p2/4p2p/4n1P1/kPPN1nP1/5P2/8/2KR1B1R b - - ").unwrap();
        let m = find_move(&board, "b8b4");
        assert_eq!(board.static_exchange_evaluation(m), 1);
    }

    #[test]
    fn see_test43() {
        let board = Board::from_fen("2r2rk1/5pp1/pp5p/q2p4/P3n3/1Q3NP1/1P2PP1P/2RR2K1 b - - ").unwrap();
        let m = find_move(&board, "c8c1");
        assert_eq!(board.static_exchange_evaluation(m), 0);
    }

    #[test]
    fn see_test44() {
        let board = Board::from_fen("5rk1/5pp1/2r4p/5b2/2R5/6Q1/R1P1qPP1/5NK1 b - - ").unwrap();
        let m = find_move(&board, "f5c2");
        assert_eq!(board.static_exchange_evaluation(m), -1);
    }

    #[test]
    fn see_test45() {
        let board = Board::from_fen("1r3r1k/p4pp1/2p1p2p/qpQP3P/2P5/3R4/PP3PP1/1K1R4 b - - ").unwrap();
        let m = find_move(&board, "a5a2");
        assert_eq!(board.static_exchange_evaluation(m), -8);
    }

    #[test]
    fn see_test46() {
        let board = Board::from_fen("1r5k/p4pp1/2p1p2p/qpQP3P/2P2P2/1P1R4/P4rP1/1K1R4 b - - ").unwrap();
        let m = find_move(&board, "a5a2");
        assert_eq!(board.static_exchange_evaluation(m), 1);
    }

    #[test]
    fn see_test47() {
        let board = Board::from_fen("r2q1rk1/1b2bppp/p2p1n2/1ppNp3/3nP3/P2P1N1P/BPP2PP1/R1BQR1K1 w - - ").unwrap();
        let m = find_move(&board, "d5e7");
        assert_eq!(board.static_exchange_evaluation(m), 0);
    }

    #[test]
    fn see_test48() {
        let board = Board::from_fen("rnbqrbn1/pp3ppp/3p4/2p2k2/4p3/3B1K2/PPP2PPP/RNB1Q1NR w - - ").unwrap();
        let m = find_move(&board, "d3e4");
        assert_eq!(board.static_exchange_evaluation(m), 1);
    }

    #[test]
    fn see_test49() {
        let board = Board::from_fen("rnb1k2r/p3p1pp/1p3p1b/7n/1N2N3/3P1PB1/PPP1P1PP/R2QKB1R w KQkq - ").unwrap();
        let m = find_move(&board, "e4d6");
        assert_eq!(board.static_exchange_evaluation(m), -2);
    }

    #[test]
    fn see_test50() {
        let board = Board::from_fen("r1b1k2r/p4npp/1pp2p1b/7n/1N2N3/3P1PB1/PPP1P1PP/R2QKB1R w KQkq - ").unwrap();
        let m = find_move(&board, "e4d6");
        assert_eq!(board.static_exchange_evaluation(m), 0);
    }

    #[test]
    fn see_test51() {
        let board = Board::from_fen("2r1k2r/pb4pp/5p1b/2KB3n/4N3/2NP1PB1/PPP1P1PP/R2Q3R w k - ").unwrap();
        let m = find_move(&board, "d5c6");
        assert_eq!(board.static_exchange_evaluation(m), -3);
    }

    #[test]
    fn see_test52() {
        let board = Board::from_fen("2r1k2r/pb4pp/5p1b/2KB3n/1N2N3/3P1PB1/PPP1P1PP/R2Q3R w k - ").unwrap();
        let m = find_move(&board, "d5c6");
        assert_eq!(board.static_exchange_evaluation(m), 0);
    }

    #[test]
    fn see_test53() {
        let board = Board::from_fen("2r1k3/pbr3pp/5p1b/2KB3n/1N2N3/3P1PB1/PPP1P1PP/R2Q3R w - - ").unwrap();
        let m = find_move(&board, "d5c6");
        assert_eq!(board.static_exchange_evaluation(m), -3);
    }

    #[test]
    fn see_test54() {
        let board = Board::from_fen("5k2/p2P2pp/8/1pb5/1Nn1P1n1/6Q1/PPP4P/R3K1NR w KQ - ").unwrap();
        let m = find_move(&board, "d7d8q");
        assert_eq!(board.static_exchange_evaluation(m), 8);
    }

    #[test]
    fn see_test55() {
        let board = Board::from_fen("r4k2/p2P2pp/8/1pb5/1Nn1P1n1/6Q1/PPP4P/R3K1NR w KQ - ").unwrap();
        let m = find_move(&board, "d7d8q");
        assert_eq!(board.static_exchange_evaluation(m), -1);
    }

    #[test]
    fn see_test56() {
        let board = Board::from_fen("5k2/p2P2pp/1b6/1p6/1Nn1P1n1/8/PPP4P/R2QK1NR w KQ - ").unwrap();
        let m = find_move(&board, "d7d8q");
        assert_eq!(board.static_exchange_evaluation(m), 2);
    }

    #[test]
    fn see_test57() {
        let board = Board::from_fen("4kbnr/p1P1pppp/b7/4q3/7n/8/PP1PPPPP/RNBQKBNR w KQk - ").unwrap();
        let m = find_move(&board, "c7c8q");
        assert_eq!(board.static_exchange_evaluation(m), -1);
    }

    #[test]
    fn see_test58() {
        let board = Board::from_fen("4kbnr/p1P1pppp/b7/4q3/7n/8/PPQPPPPP/RNB1KBNR w KQk - ").unwrap();
        let m = find_move(&board, "c7c8q");
        assert_eq!(board.static_exchange_evaluation(m), 2);
    }

    #[test]
    fn see_test59() {
        let board = Board::from_fen("4kbnr/p1P1pppp/b7/4q3/7n/8/PPQPPPPP/RNB1KBNR w KQk - ").unwrap();
        let m = find_move(&board, "c7c8q");
        assert_eq!(board.static_exchange_evaluation(m), 2);
    }

    #[test]
    fn see_test60() {
        let board = Board::from_fen("4kbnr/p1P4p/b1q5/5pP1/4n3/5Q2/PP1PPP1P/RNB1KBNR w KQk f6 ").unwrap();
        let m = find_move(&board, "g5f6");
        assert_eq!(board.static_exchange_evaluation(m), 0);
    }

    #[test]
    fn see_test61() {
        let board = Board::from_fen("4kbnr/p1P4p/b1q5/5pP1/4n3/5Q2/PP1PPP1P/RNB1KBNR w KQk f6 ").unwrap();
        let m = find_move(&board, "g5f6");
        assert_eq!(board.static_exchange_evaluation(m), 0);
    }

    #[test]
    fn see_test62() {
        let board = Board::from_fen("4kbnr/p1P4p/b1q5/5pP1/4n2Q/8/PP1PPP1P/RNB1KBNR w KQk f6 ").unwrap();
        let m = find_move(&board, "g5f6");
        assert_eq!(board.static_exchange_evaluation(m), 0);
    }

    #[test]
    fn see_test63() {
        let board = Board::from_fen("1n2kb1r/p1P4p/2qb4/5pP1/4n2Q/8/PP1PPP1P/RNB1KBNR w KQk - ").unwrap();
        let m = find_move(&board, "c7b8q");
        assert_eq!(board.static_exchange_evaluation(m), 2);
    }

    #[test]
    fn see_test64() {
        let board = Board::from_fen("rnbqk2r/pp3ppp/2p1pn2/3p4/3P4/N1P1BN2/PPB1PPPb/R2Q1RK1 w kq - ").unwrap();
        let m = find_move(&board, "g1h2");
        assert_eq!(board.static_exchange_evaluation(m), 3);
    }

    #[test]
    fn see_test65() {
        let board = Board::from_fen("3N4/2K5/2n5/1k6/8/8/8/8 b - - ").unwrap();
        let m = find_move(&board, "c6d8");
        assert_eq!(board.static_exchange_evaluation(m), 0);
    }

    #[test]
    fn see_test66() {
        let board = Board::from_fen("3n3r/2P5/8/1k6/8/8/3Q4/4K3 w - - ").unwrap();
        let m = find_move(&board, "c7d8q");
        assert_eq!(board.static_exchange_evaluation(m), 7);
    }

    #[test]
    fn see_test67() {
        let board = Board::from_fen("r2n3r/2P1P3/4N3/1k6/8/8/8/4K3 w - - ").unwrap();
        let m = find_move(&board, "e6d8");
        assert_eq!(board.static_exchange_evaluation(m), 3);
    }

    #[test]
    fn see_test68() {
        let board = Board::from_fen("8/8/8/1k6/6b1/4N3/2p3K1/3n4 w - - ").unwrap();
        let m = find_move(&board, "e3d1");
        assert_eq!(board.static_exchange_evaluation(m), 0);
    }

    #[test]
    fn see_test69() {
        let board = Board::from_fen("8/8/1k6/8/8/2N1N3/4p1K1/3n4 w - - ").unwrap();
        let m = find_move(&board, "c3d1");
        assert_eq!(board.static_exchange_evaluation(m), 1);
    }

    #[test]
    fn see_test70() {
        let board = Board::from_fen("r1bqk1nr/pppp1ppp/2n5/1B2p3/1b2P3/5N2/PPPP1PPP/RNBQK2R w KQkq - ").unwrap();
        let m = find_move(&board, "e1g1");
        assert_eq!(board.static_exchange_evaluation(m), 0);
    }
}
