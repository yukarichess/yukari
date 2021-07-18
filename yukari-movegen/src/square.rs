use crate::{colour::Colour, piece::Piece};
use std::{
    convert::TryFrom,
    fmt::{Debug, Display},
    num::NonZeroU8,
    str::FromStr
};

const DIRECTIONS: [Option<Direction>; 240] = [
    Some(Direction::SouthWest),
    None,
    None,
    None,
    None,
    None,
    None,
    Some(Direction::South),
    None,
    None,
    None,
    None,
    None,
    None,
    Some(Direction::SouthEast),
    None,
    None,
    Some(Direction::SouthWest),
    None,
    None,
    None,
    None,
    None,
    Some(Direction::South),
    None,
    None,
    None,
    None,
    None,
    Some(Direction::SouthEast),
    None,
    None,
    None,
    None,
    Some(Direction::SouthWest),
    None,
    None,
    None,
    None,
    Some(Direction::South),
    None,
    None,
    None,
    None,
    Some(Direction::SouthEast),
    None,
    None,
    None,
    None,
    None,
    None,
    Some(Direction::SouthWest),
    None,
    None,
    None,
    Some(Direction::South),
    None,
    None,
    None,
    Some(Direction::SouthEast),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some(Direction::SouthWest),
    None,
    None,
    Some(Direction::South),
    None,
    None,
    Some(Direction::SouthEast),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some(Direction::SouthWest),
    None,
    Some(Direction::South),
    None,
    Some(Direction::SouthEast),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some(Direction::SouthWest),
    Some(Direction::South),
    Some(Direction::SouthEast),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some(Direction::West),
    Some(Direction::West),
    Some(Direction::West),
    Some(Direction::West),
    Some(Direction::West),
    Some(Direction::West),
    Some(Direction::West),
    None,
    Some(Direction::East),
    Some(Direction::East),
    Some(Direction::East),
    Some(Direction::East),
    Some(Direction::East),
    Some(Direction::East),
    Some(Direction::East),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some(Direction::NorthWest),
    Some(Direction::North),
    Some(Direction::NorthEast),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some(Direction::NorthWest),
    None,
    Some(Direction::North),
    None,
    Some(Direction::NorthEast),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some(Direction::NorthWest),
    None,
    None,
    Some(Direction::North),
    None,
    None,
    Some(Direction::NorthEast),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some(Direction::NorthWest),
    None,
    None,
    None,
    Some(Direction::North),
    None,
    None,
    None,
    Some(Direction::NorthEast),
    None,
    None,
    None,
    None,
    None,
    None,
    Some(Direction::NorthWest),
    None,
    None,
    None,
    None,
    Some(Direction::North),
    None,
    None,
    None,
    None,
    Some(Direction::NorthEast),
    None,
    None,
    None,
    None,
    Some(Direction::NorthWest),
    None,
    None,
    None,
    None,
    None,
    Some(Direction::North),
    None,
    None,
    None,
    None,
    None,
    Some(Direction::NorthEast),
    None,
    None,
    Some(Direction::NorthWest),
    None,
    None,
    None,
    None,
    None,
    None,
    Some(Direction::North),
    None,
    None,
    None,
    None,
    None,
    None,
    Some(Direction::NorthEast),
    None,
];

/// A chessboard rank.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Rank {
    One,
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
}

impl Display for Rank {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::One => write!(f, "1"),
            Self::Two => write!(f, "2"),
            Self::Three => write!(f, "3"),
            Self::Four => write!(f, "4"),
            Self::Five => write!(f, "5"),
            Self::Six => write!(f, "6"),
            Self::Seven => write!(f, "7"),
            Self::Eight => write!(f, "8"),
        }
    }
}

impl From<Rank> for u8 {
    #[inline]
    fn from(rank: Rank) -> Self {
        match rank {
            Rank::One => 0,
            Rank::Two => 1,
            Rank::Three => 2,
            Rank::Four => 3,
            Rank::Five => 4,
            Rank::Six => 5,
            Rank::Seven => 6,
            Rank::Eight => 7,
        }
    }
}

impl TryFrom<u8> for Rank {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::One),
            1 => Ok(Self::Two),
            2 => Ok(Self::Three),
            3 => Ok(Self::Four),
            4 => Ok(Self::Five),
            5 => Ok(Self::Six),
            6 => Ok(Self::Seven),
            7 => Ok(Self::Eight),
            _ => Err(()),
        }
    }
}

impl Rank {
    pub const fn north(self) -> Option<Self> {
        match self {
            Self::One => Some(Self::Two),
            Self::Two => Some(Self::Three),
            Self::Three => Some(Self::Four),
            Self::Four => Some(Self::Five),
            Self::Five => Some(Self::Six),
            Self::Six => Some(Self::Seven),
            Self::Seven => Some(Self::Eight),
            Self::Eight => None,
        }
    }

    pub const fn south(self) -> Option<Self> {
        match self {
            Self::One => None,
            Self::Two => Some(Self::One),
            Self::Three => Some(Self::Two),
            Self::Four => Some(Self::Three),
            Self::Five => Some(Self::Four),
            Self::Six => Some(Self::Five),
            Self::Seven => Some(Self::Six),
            Self::Eight => Some(Self::Seven),
        }
    }

    pub fn is_relative_fourth(self, colour: Colour) -> bool {
        match colour {
            Colour::White => self == Self::Four,
            Colour::Black => self == Self::Five,
        }
    }

    pub fn is_relative_eighth(self, colour: Colour) -> bool {
        match colour {
            Colour::White => self == Self::Eight,
            Colour::Black => self == Self::One,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum File {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
}

impl Display for File {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::A => write!(f, "a"),
            Self::B => write!(f, "b"),
            Self::C => write!(f, "c"),
            Self::D => write!(f, "d"),
            Self::E => write!(f, "e"),
            Self::F => write!(f, "f"),
            Self::G => write!(f, "g"),
            Self::H => write!(f, "h"),
        }
    }
}

impl From<File> for u8 {
    #[inline]
    fn from(file: File) -> Self {
        match file {
            File::A => 0,
            File::B => 1,
            File::C => 2,
            File::D => 3,
            File::E => 4,
            File::F => 5,
            File::G => 6,
            File::H => 7,
        }
    }
}

impl TryFrom<u8> for File {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::A),
            1 => Ok(Self::B),
            2 => Ok(Self::C),
            3 => Ok(Self::D),
            4 => Ok(Self::E),
            5 => Ok(Self::F),
            6 => Ok(Self::G),
            7 => Ok(Self::H),
            _ => Err(()),
        }
    }
}

impl File {
    pub const fn east(self) -> Option<Self> {
        match self {
            Self::A => Some(Self::B),
            Self::B => Some(Self::C),
            Self::C => Some(Self::D),
            Self::D => Some(Self::E),
            Self::E => Some(Self::F),
            Self::F => Some(Self::G),
            Self::G => Some(Self::H),
            Self::H => None,
        }
    }

    pub const fn west(self) -> Option<Self> {
        match self {
            Self::A => None,
            Self::B => Some(Self::A),
            Self::C => Some(Self::B),
            Self::D => Some(Self::C),
            Self::E => Some(Self::D),
            Self::F => Some(Self::E),
            Self::G => Some(Self::F),
            Self::H => Some(Self::G),
        }
    }
}

#[allow(clippy::module_name_repetitions)]
#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(transparent)]
pub struct Square16x8(u8);

impl From<Square> for Square16x8 {
    fn from(square: Square) -> Self {
        let square = square.into_inner();
        Self(square + (square & !7))
    }
}

impl Square16x8 {
    pub const fn from_square(square: Square) -> Self {
        let square = square.into_inner();
        let square = square + (square & !7);
        Self(square)
    }

    pub(crate) const fn to_square(self) -> Option<Square> {
        if self.is_off_board() {
            return None;
        }
        let sq = self.0;
        Some(unsafe { Square::from_u8_unchecked((sq + (sq & 7)) >> 1) })
    }

    pub(crate) const fn is_off_board(self) -> bool {
        self.0 & 0x88 != 0
    }

    pub(crate) const fn vector(self, dest: Self) -> usize {
        let from = self.0;
        let dest = dest.0;
        dest.wrapping_sub(from).wrapping_add(119) as usize
    }

    pub(crate) fn add_dir(self, dir: Direction) -> Self {
        let sq = i16::from(self.0);
        let sq = sq.wrapping_add(dir.to_16x8());
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let sq = Self(sq as u8);
        sq
    }

    /// Return the `Direction` between two squares, if any exists.
    #[must_use]
    pub(crate) fn direction(self, dest: Self) -> Option<Direction> {
        unsafe { *DIRECTIONS.get_unchecked(self.vector(dest)) }
    }

    /// An iterator over the squares in a `Direction`.
    #[must_use]
    pub(crate) const fn ray_attacks(self, dir: Direction) -> RayIter {
        RayIter(self, dir)
    }
}

/// A square on a chessboard.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Square(NonZeroU8);

impl Default for Square {
    fn default() -> Self {
        // SAFETY: One is not zero.
        Self(unsafe { NonZeroU8::new_unchecked(1) })
    }
}

impl Display for Square {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", File::from(*self), Rank::from(*self))
    }
}

impl Debug for Square {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", File::from(*self), Rank::from(*self))
    }
}

impl From<Square> for Rank {
    fn from(square: Square) -> Self {
        // This is an exhaustive match, so the unreachable! really is unreachable.
        #[allow(clippy::unreachable)]
        match square.into_inner() / 8 {
            0 => Self::One,
            1 => Self::Two,
            2 => Self::Three,
            3 => Self::Four,
            4 => Self::Five,
            5 => Self::Six,
            6 => Self::Seven,
            7 => Self::Eight,
            _ => unreachable!(),
        }
    }
}

impl From<Square> for File {
    fn from(square: Square) -> Self {
        // This is an exhaustive match, so the unreachable! really is unreachable.
        #[allow(clippy::unreachable)]
        match square.into_inner() % 8 {
            0 => Self::A,
            1 => Self::B,
            2 => Self::C,
            3 => Self::D,
            4 => Self::E,
            5 => Self::F,
            6 => Self::G,
            7 => Self::H,
            _ => unreachable!(),
        }
    }
}

impl FromStr for Square {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, ()> {
        if !s.is_ascii() {
            return Err(());
        }
        let chars = s.as_bytes();
        if !(b'a'..=b'h').contains(&chars[0]) {
            return Err(());
        }
        if !(b'1'..=b'8').contains(&chars[1]) {
            return Err(());
        }
        let file = chars[0] - b'a';
        let rank = chars[1] - b'1';
        // SAFETY: values are constrained above and the "plus one" ensures this will never be zero.
        let square = unsafe { NonZeroU8::new_unchecked((8 * rank + file) + 1) };
        Ok(Self(square))
    }
}

impl TryFrom<u8> for Square {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(Self::from_rank_file(
            Rank::try_from(value / 8)?,
            File::try_from(value % 8)?,
        ))
    }
}

impl Square {
    /// Construct a `Square` from a `Rank` and `File`.
    #[must_use]
    pub fn from_rank_file(rank: Rank, file: File) -> Self {
        let rank = u8::from(rank);
        let file = u8::from(file);
        // SAFETY: the "plus one" ensures this will never be zero.
        let square = unsafe { NonZeroU8::new_unchecked((8 * rank + file) + 1) };
        Self(square)
    }

    /// Construct a `Square` directly from a `u8`.
    ///
    /// # Safety
    ///
    /// `sq` must be in the range 0-63.
    #[must_use]
    pub const unsafe fn from_u8_unchecked(sq: u8) -> Self {
        Self(NonZeroU8::new_unchecked(sq + 1))
    }

    /// Return the internal `u8` with the range 0-63.
    #[must_use]
    pub const fn into_inner(self) -> u8 {
        // The "& 63" is to hint to the compiler that this will never be greater than it.
        (self.0.get() - 1) & 63
    }

    /// Return the `Direction` between two squares, if any exists.
    #[must_use]
    pub fn direction(self, dest: Self) -> Option<Direction> {
        let dest = Square16x8::from(dest);
        let from = Square16x8::from(self);

        unsafe { *DIRECTIONS.get_unchecked(from.vector(dest)) }
    }

    /// Return the `Square` in a given `Direction`, if one exists.
    #[must_use]
    pub fn travel(self, direction: Direction) -> Option<Self> {
        Square16x8::from_square(self).add_dir(direction).to_square()
    }

    #[must_use]
    pub fn north(self) -> Option<Self> {
        self.travel(Direction::North)
    }

    #[must_use]
    pub fn north_east(self) -> Option<Self> {
        self.travel(Direction::NorthEast)
    }

    #[must_use]
    pub fn east(self) -> Option<Self> {
        self.travel(Direction::East)
    }

    #[must_use]
    pub fn south_east(self) -> Option<Self> {
        self.travel(Direction::SouthEast)
    }

    #[must_use]
    pub fn south(self) -> Option<Self> {
        self.travel(Direction::South)
    }

    #[must_use]
    pub fn south_west(self) -> Option<Self> {
        self.travel(Direction::SouthWest)
    }

    #[must_use]
    pub fn west(self) -> Option<Self> {
        self.travel(Direction::West)
    }

    #[must_use]
    pub fn north_west(self) -> Option<Self> {
        self.travel(Direction::NorthWest)
    }

    /// The colour-dependent north of a square.
    #[must_use]
    pub fn relative_north(self, colour: Colour) -> Option<Self> {
        match colour {
            Colour::White => self.north(),
            Colour::Black => self.south(),
        }
    }

    /// The colour-dependent south of a square.
    #[must_use]
    pub fn relative_south(self, colour: Colour) -> Option<Self> {
        match colour {
            Colour::White => self.south(),
            Colour::Black => self.north(),
        }
    }

    /// An iterator over the squares a pawn attacks.
    #[must_use]
    pub fn pawn_attacks(self, colour: Colour) -> PawnIter {
        let relative_north = match colour {
            Colour::White => self.north(),
            Colour::Black => self.south(),
        };

        PawnIter(relative_north, 0)
    }

    /// An iterator over the squares a knight attacks.
    #[must_use]
    pub const fn knight_attacks(self) -> KnightIter {
        KnightIter(self, 0)
    }

    /// An iterator over the squares a king attacks.
    #[must_use]
    pub const fn king_attacks(self) -> KingIter {
        KingIter(self, 0)
    }

    #[must_use]
    pub const fn flip(self) -> Self {
        unsafe { Self::from_u8_unchecked(self.into_inner() ^ 56) }
    }
}

/// A chess direction.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Direction {
    /// North.
    North = 0,
    /// North-northeast.
    NorthNorthEast = 1,
    /// Northeast.
    NorthEast = 2,
    /// East-northeast.
    EastNorthEast = 3,
    /// East.
    East = 4,
    /// East-southeast.
    EastSouthEast = 5,
    /// Southeast.
    SouthEast = 6,
    /// South-southeast.
    SouthSouthEast = 7,
    /// South.
    South = 8,
    /// South-southwest.
    SouthSouthWest = 9,
    /// Southwest.
    SouthWest = 10,
    /// West-southwest.
    WestSouthWest = 11,
    /// West.
    West = 12,
    /// West-northwest.
    WestNorthWest = 13,
    /// Northwest.
    NorthWest = 14,
    /// North-northwest.
    NorthNorthWest = 15,
}

impl Direction {
    /// The `Direction` 180 degrees of the given `Direction`.
    pub const fn opposite(self) -> Self {
        const OPPOSITE: [Direction; 16] = [
            Direction::South,
            Direction::SouthSouthWest,
            Direction::SouthWest,
            Direction::WestSouthWest,
            Direction::West,
            Direction::WestNorthWest,
            Direction::NorthWest,
            Direction::NorthNorthWest,
            Direction::North,
            Direction::NorthNorthEast,
            Direction::NorthEast,
            Direction::EastNorthEast,
            Direction::East,
            Direction::EastSouthEast,
            Direction::SouthEast,
            Direction::SouthSouthEast,
        ];

        OPPOSITE[self as usize]
    }

    /// Returns true if the direction is diagonal.
    pub const fn diagonal(self) -> bool {
        matches!(
            self,
            Self::NorthEast | Self::SouthEast | Self::SouthWest | Self::NorthWest
        )
    }

    /// Return true if the direction is orthogonal.
    pub const fn orthogonal(self) -> bool {
        matches!(self, Self::North | Self::East | Self::West | Self::South)
    }

    /// Returns the 16x8 square difference of this Direction.
    pub const fn to_16x8(self) -> i16 {
        const VECTORS: [i16; 16] = [
            16, 33, 17, 18, 1, -14, -15, -31, -16, -33, -17, -18, -1, 14, 15, 31,
        ];
        VECTORS[self as usize]
    }

    pub fn valid_for_slider(self, piece: Piece) -> bool {
        match piece {
            Piece::Bishop => self.diagonal(),
            Piece::Rook => self.orthogonal(),
            Piece::Queen => self.diagonal() || self.orthogonal(),
            _ => unreachable!("piece {:?} is not a slider", piece),
        }
    }
}

pub struct PawnIter(Option<Square>, u8);

impl Iterator for PawnIter {
    type Item = Square;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let next = match self.1 {
                0 => self.0.and_then(Square::east),
                1 => self.0.and_then(Square::west),
                _ => return None,
            };

            self.1 += 1;

            if next.is_some() {
                return next;
            }
        }
    }
}

/// An iterator over the knight attacks of a `Square`.
pub struct KnightIter(Square, u8);

impl Iterator for KnightIter {
    type Item = Square;

    fn next(&mut self) -> Option<Self::Item> {
        use Direction::{
            EastNorthEast, EastSouthEast, NorthNorthEast, NorthNorthWest, SouthSouthEast,
            SouthSouthWest, WestNorthWest, WestSouthWest,
        };
        const KNIGHT_DIR: [Direction; 8] = [
            NorthNorthEast,
            EastNorthEast,
            EastSouthEast,
            SouthSouthEast,
            SouthSouthWest,
            WestSouthWest,
            WestNorthWest,
            NorthNorthWest,
        ];

        loop {
            if self.1 >= 8 {
                return None;
            }

            let next = self.0.travel(KNIGHT_DIR[self.1 as usize]);
            self.1 += 1;

            if next.is_some() {
                return next;
            }
        }
    }
}

/// An iterator over the `Square`s in a `Direction`.
pub struct RayIter(Square16x8, Direction);

impl Iterator for RayIter {
    type Item = Square;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.0.add_dir(self.1);
        if next.is_off_board() {
            return None;
        }
        self.0 = next;
        next.to_square()
    }
}

/// An iterator over the king attacks of a `Square`.
pub struct KingIter(Square, u8);

impl Iterator for KingIter {
    type Item = Square;

    fn next(&mut self) -> Option<Self::Item> {
        use Direction::{East, North, NorthEast, NorthWest, South, SouthEast, SouthWest, West};
        const KING_DIR: [Direction; 8] = [
            North, NorthEast, East, SouthEast, South, SouthWest, West, NorthWest,
        ];

        loop {
            if self.1 >= 8 {
                return None;
            }

            let next = self.0.travel(KING_DIR[self.1 as usize]);
            self.1 += 1;

            if next.is_some() {
                return next;
            }
        }
    }
}
