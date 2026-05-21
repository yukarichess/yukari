use crate::{
    Piece, Square,
    square::{Direction, Square16x8},
};

const DIRECTIONS: [&[Direction]; 6] = [
    &[],
    &[
        Direction::NorthNorthEast,
        Direction::EastNorthEast,
        Direction::EastSouthEast,
        Direction::SouthSouthEast,
        Direction::SouthSouthWest,
        Direction::WestSouthWest,
        Direction::WestNorthWest,
        Direction::NorthNorthWest,
    ],
    &[Direction::NorthEast, Direction::SouthEast, Direction::SouthWest, Direction::NorthWest],
    &[Direction::North, Direction::East, Direction::South, Direction::West],
    &[
        Direction::NorthEast,
        Direction::SouthEast,
        Direction::SouthWest,
        Direction::NorthWest,
        Direction::North,
        Direction::East,
        Direction::South,
        Direction::West,
    ],
    &[
        Direction::NorthEast,
        Direction::SouthEast,
        Direction::SouthWest,
        Direction::NorthWest,
        Direction::North,
        Direction::East,
        Direction::South,
        Direction::West,
    ],
];

const SLIDER: [bool; 6] = [false, false, true, true, true, false];

const PAWN_INDEX: usize = 84;
const KNIGHT_INDEX: [usize; 65] = index_array_for_piece(Piece::Knight);
const BISHOP_INDEX: [usize; 65] = index_array_for_piece(Piece::Bishop);
const ROOK_INDEX: [usize; 65] = index_array_for_piece(Piece::Rook);
const QUEEN_INDEX: [usize; 65] = index_array_for_piece(Piece::Queen);

const PAWN_OFFSET: usize = 0;
const KNIGHT_OFFSET: usize = PAWN_OFFSET + 6 * PAWN_INDEX;
const BISHOP_OFFSET: usize = KNIGHT_OFFSET + 10 * KNIGHT_INDEX[64];
const ROOK_OFFSET: usize = BISHOP_OFFSET + 8 * BISHOP_INDEX[64];
const QUEEN_OFFSET: usize = ROOK_OFFSET + 8 * ROOK_INDEX[64];
pub const MAX_OFFSET: usize = QUEEN_OFFSET + 10 * QUEEN_INDEX[64];

const KNIGHT_ATTACKS: [u64; 64] = attacks_for_piece(Piece::Knight);
const BISHOP_ATTACKS: [u64; 64] = attacks_for_piece(Piece::Bishop);
const ROOK_ATTACKS: [u64; 64] = attacks_for_piece(Piece::Rook);
const QUEEN_ATTACKS: [u64; 64] = attacks_for_piece(Piece::Queen);

const PAWN_MAP: [[Option<usize>; 6]; 2] = [[Some(0), Some(1), None, Some(2), None, None], [Some(3), Some(4), None, Some(5), None, None]];
const KNIGHT_QUEEN_MAP: [[Option<usize>; 6]; 2] = [[Some(0), Some(1), Some(2), Some(3), Some(4), None], [Some(5), Some(6), Some(7), Some(8), Some(9), None]];
const BISHOP_ROOK_MAP: [[Option<usize>; 6]; 2] = [[Some(0), Some(1), Some(2), Some(3), None, None], [Some(4), Some(5), Some(6), Some(7), None, None]];

const fn index_array_for_piece(piece: Piece) -> [usize; 65] {
    assert!(!matches!(piece, Piece::Pawn));
    let mut indexes = [0_usize; 65];
    let mut square = 0_u8;
    let mut count = 0;

    while square < 64 {
        indexes[square as usize] = count;

        let square16x8 = Square16x8::from_square(unsafe { Square::from_u8_unchecked(square) });

        let mut dir_index = 0;
        while dir_index < DIRECTIONS[piece as usize].len() {
            let dir = DIRECTIONS[piece as usize][dir_index];
            let mut dest = square16x8.add_dir(dir);

            while !dest.is_off_board() {
                count += 1;
                if !SLIDER[piece as usize] {
                    break;
                }
                dest = dest.add_dir(dir);
            }

            dir_index += 1;
        }

        square += 1;
    }
    indexes[64] = count;

    indexes
}

const fn attacks_for_piece(piece: Piece) -> [u64; 64] {
    assert!(!matches!(piece, Piece::Pawn));
    let mut attacks = [0_u64; 64];
    let mut square = 0_u8;

    while square < 64 {
        let square16x8 = Square16x8::from_square(unsafe { Square::from_u8_unchecked(square) });
        let mut dir_index = 0;

        while dir_index < DIRECTIONS[piece as usize].len() {
            let dir = DIRECTIONS[piece as usize][dir_index];
            let mut dest = square16x8.add_dir(dir);

            while !dest.is_off_board() {
                attacks[square as usize] |= 1_u64 << (dest.to_square().unwrap().into_inner());
                if !SLIDER[piece as usize] {
                    break;
                }
                dest = dest.add_dir(dir);
            }

            dir_index += 1;
        }

        square += 1;
    }

    attacks
}

pub fn index_pst(piece: Piece, square: Square, king: Square, friendly: bool) -> usize {
    let square = square.into_inner() as usize ^ crate::board::eval::Eval::mirror(king);
    64 * (usize::from(!friendly) * 6 + piece as usize) + square
}

pub fn index_threat(
    from_piece: Piece, from_square: Square, to_piece: Piece, to_square: Square, king: Square, friendly: bool, attacking_enemy: bool, flipped: bool,
) -> Option<usize> {
    if from_piece == Piece::King || to_piece == Piece::King {
        return None;
    }

    let attacker = match (from_piece, friendly ^ flipped) {
        (Piece::Pawn, true) => "P",
        (Piece::Pawn, false) => "p",
        (Piece::Knight, true) => "N",
        (Piece::Knight, false) => "n",
        (Piece::Bishop, true) => "B",
        (Piece::Bishop, false) => "b",
        (Piece::Rook, true) => "R",
        (Piece::Rook, false) => "r",
        (Piece::Queen, true) => "Q",
        (Piece::Queen, false) => "q",
        (Piece::King, _) => unreachable!(),
    };
    let victim = match (to_piece, !attacking_enemy ^ flipped) {
        (Piece::Pawn, true) => "P",
        (Piece::Pawn, false) => "p",
        (Piece::Knight, true) => "N",
        (Piece::Knight, false) => "n",
        (Piece::Bishop, true) => "B",
        (Piece::Bishop, false) => "b",
        (Piece::Rook, true) => "R",
        (Piece::Rook, false) => "r",
        (Piece::Queen, true) => "Q",
        (Piece::Queen, false) => "q",
        (Piece::King, _) => unreachable!(),
    };
    if flipped {
        //print!("attacker = {attacker}, victim = {victim}, from = {}, to = {}, ", from_square.flip(), to_square.flip());
    } else {
        //print!("attacker = {attacker}, victim = {victim}, from = {}, to = {}, ", from_square, to_square);
    }

    if friendly == attacking_enemy && to_square > from_square && to_piece == from_piece {
        //println!("SEMIËXCLUDED");
        return None;
    }

    let from_square = from_square.into_inner() as usize ^ crate::board::eval::Eval::mirror(king);
    let to_square = to_square.into_inner() as usize ^ crate::board::eval::Eval::mirror(king);

    let pawn_threat = || {
        let to_piece = to_piece as usize;
        let attacking_enemy = usize::from(attacking_enemy);
        let Some(map) = PAWN_MAP[attacking_enemy][to_piece] else {
            //println!("SEMIËXCLUDED");
            return None;
        };

        let up = usize::from(to_square > from_square);
        let diff = to_square.abs_diff(from_square);
        let id = usize::from(diff != [9, 7][up]);
        let attack = 2 * (from_square % 8) + id - 1;
        let idx = PAWN_OFFSET + map * PAWN_INDEX + (from_square / 8 - 1) * 14 + attack;
        Some(idx)
    };

    let piece_threat = |indexes: &[usize; 65], attacks: &[u64; 64], map: &[[Option<usize>; 6]; 2], offset: usize| {
        let to_piece = to_piece as usize;
        let attacking_enemy = usize::from(attacking_enemy);
        let Some(map) = map[attacking_enemy][to_piece] else {
            //println!("SEMIËXCLUDED");
            return None;
        };

        let below = (attacks[from_square] & ((1 << to_square) - 1)).count_ones() as usize;
        let idx = offset + (map * indexes[64]) + indexes[from_square] + below;
        Some(idx)
    };

    let idx = match from_piece {
        Piece::Pawn => pawn_threat(),
        Piece::Knight => piece_threat(&KNIGHT_INDEX, &KNIGHT_ATTACKS, &KNIGHT_QUEEN_MAP, KNIGHT_OFFSET),
        Piece::Bishop => piece_threat(&BISHOP_INDEX, &BISHOP_ATTACKS, &BISHOP_ROOK_MAP, BISHOP_OFFSET),
        Piece::Rook => piece_threat(&ROOK_INDEX, &ROOK_ATTACKS, &BISHOP_ROOK_MAP, ROOK_OFFSET),
        Piece::Queen => piece_threat(&QUEEN_INDEX, &QUEEN_ATTACKS, &KNIGHT_QUEEN_MAP, QUEEN_OFFSET),
        Piece::King => None
    }?;

    let feature_idx = (usize::from(!friendly) * MAX_OFFSET) + idx;
    //println!("{feature_idx}");

    Some(768 + feature_idx)
}
