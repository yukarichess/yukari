use crate::{
    Piece,
    board::{Board, bitlist::Bitlist},
    square::{Direction, Square16x8},
};

/// Pin information in a board.
#[derive(Default)]
pub struct PinInfo {
    pub pins:             [Option<Direction>; 32],
    pub enpassant_pinned: Bitlist,
}

impl PinInfo {
    /// Find pinned pieces and handle them specially.
    ///
    /// # Panics
    /// Panics when Lofty has written shitty code.
    #[must_use]
    pub fn discover(board: &Board) -> Self {
        let mut info = Self::default();

        let sliders = board.data.piecemask().bishops() | board.data.piecemask().rooks() | board.data.piecemask().queens();
        let king_square = board.data.king_square(board.side);
        let king_square_16x8 = Square16x8::from_square(king_square);

        for possible_pinner in board.data.piecemask().pieces_of_colour(!board.side).and(sliders) {
            let pinner_square = board.data.square_of_piece(possible_pinner);
            let pinner_square_16x8 = Square16x8::from_square(pinner_square);
            let pinner_type = board.data.piece_from_bit(possible_pinner);
            let Some(pinner_king_dir) = pinner_square_16x8.direction(king_square_16x8) else {
                continue;
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

                if let Some(piece_index) = board.data.piece_index(square) {
                    if board.data.colour_from_square(square) == Some(!board.side) {
                        match enemy_blocker {
                            Some(_) => {
                                friendly_blocker = None;
                                enemy_blocker = None;
                                break;
                            },
                            None => {
                                enemy_blocker = Some(piece_index);
                            },
                        }
                    } else {
                        match friendly_blocker {
                            Some(_) => {
                                friendly_blocker = None;
                                enemy_blocker = None;
                                break;
                            },
                            None => {
                                friendly_blocker = Some(piece_index);
                            },
                        }
                    }
                }
            }

            match (friendly_blocker, enemy_blocker) {
                // There are no friendly blockers: skip.
                (None, _) => (),
                // There is one friendly blocker: it is pinned.
                (Some(blocker), None) => {
                    info.pins[blocker.into_inner() as usize] = Some(pinner_king_dir);
                },
                // There is one friendly blocker and one enemy blocker: it *may* be pinned for en-passant purposes
                (Some(friendly_blocker), Some(enemy_blocker)) => {
                    // If at least one of the blockers is a piece, we don't need to worry about en-passant.
                    if board.data.piece_from_bit(friendly_blocker) != Piece::Pawn
                        || board.data.piece_from_bit(enemy_blocker) != Piece::Pawn
                        || (pinner_king_dir != Direction::East && pinner_king_dir != Direction::West)
                    {
                        continue;
                    }

                    // Alas, we do have to care.
                    info.enpassant_pinned |= Bitlist::from(friendly_blocker);
                },
            }
        }

        info
    }
}
