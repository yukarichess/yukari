#![feature(coroutines, coroutine_trait, stmt_expr_attributes)]
#![warn(clippy::pedantic, clippy::nursery, clippy::perf, clippy::style)]

mod board;
mod chessmove;
mod colour;
mod piece;
mod square;

pub use board::{Board, PieceIndex, Zobrist};
pub use chessmove::{Move, MoveType};
pub use colour::Colour;
pub use piece::Piece;
pub use square::Square;
use tinyvec::ArrayVec;

/// Count the number of legal chess positions after N moves.
#[inline]
#[must_use]
pub fn perft(board: &Board, zobrist: &Zobrist, depth: u32) -> u64 {
    if depth == 0 {
        1
    } else if depth == 1 {
        let moves: [Move; 256] = [Move::default(); 256];
        let mut moves = ArrayVec::from(moves);
        moves.set_len(0);
        board.generate(&mut moves);
        moves.len() as u64
    } else {
        let moves: [Move; 256] = [Move::default(); 256];
        let mut moves = ArrayVec::from(moves);
        moves.set_len(0);
        board.generate(&mut moves);

        let mut count = 0;
        for m in moves {
            let board = board.make(m, zobrist);
            count += perft(&board, zobrist, depth - 1);
        }
        count
    }
}

#[cfg(test)]
mod perft {
    use crate::{perft, Board, Zobrist};

    #[test]
    fn perft_test1() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen(
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            &zobrist,
        )
        .unwrap();
        assert_eq!(perft(&startpos, &zobrist, 1), 20);
        assert_eq!(perft(&startpos, &zobrist, 2), 400);
        assert_eq!(perft(&startpos, &zobrist, 3), 8902);
        assert_eq!(perft(&startpos, &zobrist, 4), 197_281);
        assert_eq!(perft(&startpos, &zobrist, 5), 4_865_609);
        assert_eq!(perft(&startpos, &zobrist, 6), 119_060_324);
    }

    #[test]
    fn perft_test2() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen(
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
            &zobrist,
        )
        .unwrap();
        assert_eq!(perft(&startpos, &zobrist, 1), 48);
        assert_eq!(perft(&startpos, &zobrist, 2), 2039);
        assert_eq!(perft(&startpos, &zobrist, 3), 97862);
        assert_eq!(perft(&startpos, &zobrist, 4), 4_085_603);
        assert_eq!(perft(&startpos, &zobrist, 5), 193_690_690);
    }

    #[test]
    fn perft_test3() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("4k3/8/8/8/8/8/8/4K2R w K - 0 1", &zobrist).unwrap();
        assert_eq!(perft(&startpos, &zobrist, 1), 15);
        assert_eq!(perft(&startpos, &zobrist, 2), 66);
        assert_eq!(perft(&startpos, &zobrist, 3), 1197);
        assert_eq!(perft(&startpos, &zobrist, 4), 7059);
        assert_eq!(perft(&startpos, &zobrist, 5), 133_987);
        assert_eq!(perft(&startpos, &zobrist, 6), 764_643);
    }

    #[test]
    fn perft_test4() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("4k3/8/8/8/8/8/8/R3K3 w Q - 0 1", &zobrist).unwrap();
        assert_eq!(perft(&startpos, &zobrist, 1), 16);
        assert_eq!(perft(&startpos, &zobrist, 2), 71);
        assert_eq!(perft(&startpos, &zobrist, 3), 1287);
        assert_eq!(perft(&startpos, &zobrist, 4), 7626);
        assert_eq!(perft(&startpos, &zobrist, 5), 145_232);
        assert_eq!(perft(&startpos, &zobrist, 6), 846_648);
    }

    #[test]
    fn perft_test5() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("4k2r/8/8/8/8/8/8/4K3 w k - 0 1", &zobrist).unwrap();
        assert_eq!(perft(&startpos, &zobrist, 1), 5);
        assert_eq!(perft(&startpos, &zobrist, 2), 75);
        assert_eq!(perft(&startpos, &zobrist, 3), 459);
        assert_eq!(perft(&startpos, &zobrist, 4), 8290);
        assert_eq!(perft(&startpos, &zobrist, 5), 47635);
        assert_eq!(perft(&startpos, &zobrist, 6), 899_442);
    }

    #[test]
    fn perft_test6() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("r3k3/8/8/8/8/8/8/4K3 w q - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 5);
        assert_eq!(perft(&startpos, &zobrist, 2), 80);
        assert_eq!(perft(&startpos, &zobrist, 3), 493);
        assert_eq!(perft(&startpos, &zobrist, 4), 8897);
        assert_eq!(perft(&startpos, &zobrist, 5), 52710);
        assert_eq!(perft(&startpos, &zobrist, 6), 1_001_523);
    }

    #[test]
    fn perft_test7() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("4k3/8/8/8/8/8/8/R3K2R w KQ - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 26);
        assert_eq!(perft(&startpos, &zobrist, 2), 112);
        assert_eq!(perft(&startpos, &zobrist, 3), 3189);
        assert_eq!(perft(&startpos, &zobrist, 4), 17945);
        assert_eq!(perft(&startpos, &zobrist, 5), 532_933);
        assert_eq!(perft(&startpos, &zobrist, 6), 2_788_982);
    }

    #[test]
    fn perft_test8() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("r3k2r/8/8/8/8/8/8/4K3 w kq - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 5);
        assert_eq!(perft(&startpos, &zobrist, 2), 130);
        assert_eq!(perft(&startpos, &zobrist, 3), 782);
        assert_eq!(perft(&startpos, &zobrist, 4), 22180);
        assert_eq!(perft(&startpos, &zobrist, 5), 118_882);
        assert_eq!(perft(&startpos, &zobrist, 6), 3_517_770);
    }

    #[test]
    fn perft_test9() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("8/8/8/8/8/8/6k1/4K2R w K - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 12);
        assert_eq!(perft(&startpos, &zobrist, 2), 38);
        assert_eq!(perft(&startpos, &zobrist, 3), 564);
        assert_eq!(perft(&startpos, &zobrist, 4), 2219);
        assert_eq!(perft(&startpos, &zobrist, 5), 37735);
        assert_eq!(perft(&startpos, &zobrist, 6), 185_867);
    }

    #[test]
    fn perft_test10() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("8/8/8/8/8/8/1k6/R3K3 w Q - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 15);
        assert_eq!(perft(&startpos, &zobrist, 2), 65);
        assert_eq!(perft(&startpos, &zobrist, 3), 1018);
        assert_eq!(perft(&startpos, &zobrist, 4), 4573);
        assert_eq!(perft(&startpos, &zobrist, 5), 80619);
        assert_eq!(perft(&startpos, &zobrist, 6), 413_018);
    }

    #[test]
    fn perft_test11() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("4k2r/6K1/8/8/8/8/8/8 w k - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 3);
        assert_eq!(perft(&startpos, &zobrist, 2), 32);
        assert_eq!(perft(&startpos, &zobrist, 3), 134);
        assert_eq!(perft(&startpos, &zobrist, 4), 2073);
        assert_eq!(perft(&startpos, &zobrist, 5), 10485);
        assert_eq!(perft(&startpos, &zobrist, 6), 179_869);
    }

    #[test]
    fn perft_test12() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("r3k3/1K6/8/8/8/8/8/8 w q - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 4);
        assert_eq!(perft(&startpos, &zobrist, 2), 49);
        assert_eq!(perft(&startpos, &zobrist, 3), 243);
        assert_eq!(perft(&startpos, &zobrist, 4), 3991);
        assert_eq!(perft(&startpos, &zobrist, 5), 20780);
        assert_eq!(perft(&startpos, &zobrist, 6), 367_724);
    }

    #[test]
    fn perft_test13() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 26);
        assert_eq!(perft(&startpos, &zobrist, 2), 568);
        assert_eq!(perft(&startpos, &zobrist, 3), 13744);
        assert_eq!(perft(&startpos, &zobrist, 4), 314_346);
        assert_eq!(perft(&startpos, &zobrist, 5), 7_594_526);
        assert_eq!(perft(&startpos, &zobrist, 6), 179_862_938);
    }

    #[test]
    fn perft_test14() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("r3k2r/8/8/8/8/8/8/1R2K2R w Kkq - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 25);
        assert_eq!(perft(&startpos, &zobrist, 2), 567);
        assert_eq!(perft(&startpos, &zobrist, 3), 14095);
        assert_eq!(perft(&startpos, &zobrist, 4), 328_965);
        assert_eq!(perft(&startpos, &zobrist, 5), 8_153_719);
        assert_eq!(perft(&startpos, &zobrist, 6), 195_629_489);
    }

    #[test]
    fn perft_test15() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("r3k2r/8/8/8/8/8/8/2R1K2R w Kkq - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 25);
        assert_eq!(perft(&startpos, &zobrist, 2), 548);
        assert_eq!(perft(&startpos, &zobrist, 3), 13502);
        assert_eq!(perft(&startpos, &zobrist, 4), 312_835);
        assert_eq!(perft(&startpos, &zobrist, 5), 7_736_373);
        assert_eq!(perft(&startpos, &zobrist, 6), 184_411_439);
    }

    #[test]
    fn perft_test16() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("r3k2r/8/8/8/8/8/8/R3K1R1 w Qkq - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 25);
        assert_eq!(perft(&startpos, &zobrist, 2), 547);
        assert_eq!(perft(&startpos, &zobrist, 3), 13579);
        assert_eq!(perft(&startpos, &zobrist, 4), 316_214);
        assert_eq!(perft(&startpos, &zobrist, 5), 7_878_456);
        assert_eq!(perft(&startpos, &zobrist, 6), 189_224_276);
    }

    #[test]
    fn perft_test17() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("1r2k2r/8/8/8/8/8/8/R3K2R w KQk - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 26);
        assert_eq!(perft(&startpos, &zobrist, 2), 583);
        assert_eq!(perft(&startpos, &zobrist, 3), 14252);
        assert_eq!(perft(&startpos, &zobrist, 4), 334_705);
        assert_eq!(perft(&startpos, &zobrist, 5), 8_198_901);
        assert_eq!(perft(&startpos, &zobrist, 6), 198_328_929);
    }

    #[test]
    fn perft_test18() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("2r1k2r/8/8/8/8/8/8/R3K2R w KQk - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 25);
        assert_eq!(perft(&startpos, &zobrist, 2), 560);
        assert_eq!(perft(&startpos, &zobrist, 3), 13592);
        assert_eq!(perft(&startpos, &zobrist, 4), 317_324);
        assert_eq!(perft(&startpos, &zobrist, 5), 7_710_115);
        assert_eq!(perft(&startpos, &zobrist, 6), 185_959_088);
    }

    #[test]
    fn perft_test19() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("r3k1r1/8/8/8/8/8/8/R3K2R w KQq - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 25);
        assert_eq!(perft(&startpos, &zobrist, 2), 560);
        assert_eq!(perft(&startpos, &zobrist, 3), 13607);
        assert_eq!(perft(&startpos, &zobrist, 4), 320_792);
        assert_eq!(perft(&startpos, &zobrist, 5), 7_848_606);
        assert_eq!(perft(&startpos, &zobrist, 6), 190_755_813);
    }

    #[test]
    fn perft_test20() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("4k3/8/8/8/8/8/8/4K2R b K - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 5);
        assert_eq!(perft(&startpos, &zobrist, 2), 75);
        assert_eq!(perft(&startpos, &zobrist, 3), 459);
        assert_eq!(perft(&startpos, &zobrist, 4), 8290);
        assert_eq!(perft(&startpos, &zobrist, 5), 47635);
        assert_eq!(perft(&startpos, &zobrist, 6), 899_442);
    }

    #[test]
    fn perft_test21() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("4k3/8/8/8/8/8/8/R3K3 b Q - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 5);
        assert_eq!(perft(&startpos, &zobrist, 2), 80);
        assert_eq!(perft(&startpos, &zobrist, 3), 493);
        assert_eq!(perft(&startpos, &zobrist, 4), 8897);
        assert_eq!(perft(&startpos, &zobrist, 5), 52710);
        assert_eq!(perft(&startpos, &zobrist, 6), 1_001_523);
    }

    #[test]
    fn perft_test22() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("4k2r/8/8/8/8/8/8/4K3 b k - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 15);
        assert_eq!(perft(&startpos, &zobrist, 2), 66);
        assert_eq!(perft(&startpos, &zobrist, 3), 1197);
        assert_eq!(perft(&startpos, &zobrist, 4), 7059);
        assert_eq!(perft(&startpos, &zobrist, 5), 133_987);
        assert_eq!(perft(&startpos, &zobrist, 6), 764_643);
    }

    #[test]
    fn perft_test23() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("r3k3/8/8/8/8/8/8/4K3 b q - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 16);
        assert_eq!(perft(&startpos, &zobrist, 2), 71);
        assert_eq!(perft(&startpos, &zobrist, 3), 1287);
        assert_eq!(perft(&startpos, &zobrist, 4), 7626);
        assert_eq!(perft(&startpos, &zobrist, 5), 145_232);
        assert_eq!(perft(&startpos, &zobrist, 6), 846_648);
    }

    #[test]
    fn perft_test24() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("4k3/8/8/8/8/8/8/R3K2R b KQ - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 5);
        assert_eq!(perft(&startpos, &zobrist, 2), 130);
        assert_eq!(perft(&startpos, &zobrist, 3), 782);
        assert_eq!(perft(&startpos, &zobrist, 4), 22180);
        assert_eq!(perft(&startpos, &zobrist, 5), 118_882);
        assert_eq!(perft(&startpos, &zobrist, 6), 3_517_770);
    }

    #[test]
    fn perft_test25() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("r3k2r/8/8/8/8/8/8/4K3 b kq - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 26);
        assert_eq!(perft(&startpos, &zobrist, 2), 112);
        assert_eq!(perft(&startpos, &zobrist, 3), 3189);
        assert_eq!(perft(&startpos, &zobrist, 4), 17945);
        assert_eq!(perft(&startpos, &zobrist, 5), 532_933);
        assert_eq!(perft(&startpos, &zobrist, 6), 2_788_982);
    }

    #[test]
    fn perft_test26() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("8/8/8/8/8/8/6k1/4K2R b K - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 3);
        assert_eq!(perft(&startpos, &zobrist, 2), 32);
        assert_eq!(perft(&startpos, &zobrist, 3), 134);
        assert_eq!(perft(&startpos, &zobrist, 4), 2073);
        assert_eq!(perft(&startpos, &zobrist, 5), 10485);
        assert_eq!(perft(&startpos, &zobrist, 6), 179_869);
    }

    #[test]
    fn perft_test27() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("8/8/8/8/8/8/1k6/R3K3 b Q - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 4);
        assert_eq!(perft(&startpos, &zobrist, 2), 49);
        assert_eq!(perft(&startpos, &zobrist, 3), 243);
        assert_eq!(perft(&startpos, &zobrist, 4), 3991);
        assert_eq!(perft(&startpos, &zobrist, 5), 20780);
        assert_eq!(perft(&startpos, &zobrist, 6), 367_724);
    }

    #[test]
    fn perft_test28() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("4k2r/6K1/8/8/8/8/8/8 b k - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 12);
        assert_eq!(perft(&startpos, &zobrist, 2), 38);
        assert_eq!(perft(&startpos, &zobrist, 3), 564);
        assert_eq!(perft(&startpos, &zobrist, 4), 2219);
        assert_eq!(perft(&startpos, &zobrist, 5), 37735);
        assert_eq!(perft(&startpos, &zobrist, 6), 185_867);
    }

    #[test]
    fn perft_test29() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("r3k3/1K6/8/8/8/8/8/8 b q - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 15);
        assert_eq!(perft(&startpos, &zobrist, 2), 65);
        assert_eq!(perft(&startpos, &zobrist, 3), 1018);
        assert_eq!(perft(&startpos, &zobrist, 4), 4573);
        assert_eq!(perft(&startpos, &zobrist, 5), 80619);
        assert_eq!(perft(&startpos, &zobrist, 6), 413_018);
    }

    #[test]
    fn perft_test30() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("r3k2r/8/8/8/8/8/8/R3K2R b KQkq - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 26);
        assert_eq!(perft(&startpos, &zobrist, 2), 568);
        assert_eq!(perft(&startpos, &zobrist, 3), 13744);
        assert_eq!(perft(&startpos, &zobrist, 4), 314_346);
        assert_eq!(perft(&startpos, &zobrist, 5), 7_594_526);
        assert_eq!(perft(&startpos, &zobrist, 6), 179_862_938);
    }

    #[test]
    fn perft_test31() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("r3k2r/8/8/8/8/8/8/1R2K2R b Kkq - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 26);
        assert_eq!(perft(&startpos, &zobrist, 2), 583);
        assert_eq!(perft(&startpos, &zobrist, 3), 14252);
        assert_eq!(perft(&startpos, &zobrist, 4), 334_705);
        assert_eq!(perft(&startpos, &zobrist, 5), 8_198_901);
        assert_eq!(perft(&startpos, &zobrist, 6), 198_328_929);
    }

    #[test]
    fn perft_test32() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("r3k2r/8/8/8/8/8/8/2R1K2R b Kkq - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 25);
        assert_eq!(perft(&startpos, &zobrist, 2), 560);
        assert_eq!(perft(&startpos, &zobrist, 3), 13592);
        assert_eq!(perft(&startpos, &zobrist, 4), 317_324);
        assert_eq!(perft(&startpos, &zobrist, 5), 7_710_115);
        assert_eq!(perft(&startpos, &zobrist, 6), 185_959_088);
    }

    #[test]
    fn perft_test33() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("r3k2r/8/8/8/8/8/8/R3K1R1 b Qkq - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 25);
        assert_eq!(perft(&startpos, &zobrist, 2), 560);
        assert_eq!(perft(&startpos, &zobrist, 3), 13607);
        assert_eq!(perft(&startpos, &zobrist, 4), 320_792);
        assert_eq!(perft(&startpos, &zobrist, 5), 7_848_606);
        assert_eq!(perft(&startpos, &zobrist, 6), 190_755_813);
    }

    #[test]
    fn perft_test34() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("1r2k2r/8/8/8/8/8/8/R3K2R b KQk - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 25);
        assert_eq!(perft(&startpos, &zobrist, 2), 567);
        assert_eq!(perft(&startpos, &zobrist, 3), 14095);
        assert_eq!(perft(&startpos, &zobrist, 4), 328_965);
        assert_eq!(perft(&startpos, &zobrist, 5), 8_153_719);
        assert_eq!(perft(&startpos, &zobrist, 6), 195_629_489);
    }

    #[test]
    fn perft_test35() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("2r1k2r/8/8/8/8/8/8/R3K2R b KQk - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 25);
        assert_eq!(perft(&startpos, &zobrist, 2), 548);
        assert_eq!(perft(&startpos, &zobrist, 3), 13502);
        assert_eq!(perft(&startpos, &zobrist, 4), 312_835);
        assert_eq!(perft(&startpos, &zobrist, 5), 7_736_373);
        assert_eq!(perft(&startpos, &zobrist, 6), 184_411_439);
    }

    #[test]
    fn perft_test36() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("r3k1r1/8/8/8/8/8/8/R3K2R b KQq - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 25);
        assert_eq!(perft(&startpos, &zobrist, 2), 547);
        assert_eq!(perft(&startpos, &zobrist, 3), 13579);
        assert_eq!(perft(&startpos, &zobrist, 4), 316_214);
        assert_eq!(perft(&startpos, &zobrist, 5), 7_878_456);
        assert_eq!(perft(&startpos, &zobrist, 6), 189_224_276);
    }

    #[test]
    fn perft_test37() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("8/1n4N1/2k5/8/8/5K2/1N4n1/8 w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 14);
        assert_eq!(perft(&startpos, &zobrist, 2), 195);
        assert_eq!(perft(&startpos, &zobrist, 3), 2760);
        assert_eq!(perft(&startpos, &zobrist, 4), 38675);
        assert_eq!(perft(&startpos, &zobrist, 5), 570_726);
        assert_eq!(perft(&startpos, &zobrist, 6), 8_107_539);
    }

    #[test]
    fn perft_test38() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("8/1k6/8/5N2/8/4n3/8/2K5 w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 11);
        assert_eq!(perft(&startpos, &zobrist, 2), 156);
        assert_eq!(perft(&startpos, &zobrist, 3), 1636);
        assert_eq!(perft(&startpos, &zobrist, 4), 20534);
        assert_eq!(perft(&startpos, &zobrist, 5), 223_507);
        assert_eq!(perft(&startpos, &zobrist, 6), 2_594_412);
    }

    #[test]
    fn perft_test39() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("8/8/4k3/3Nn3/3nN3/4K3/8/8 w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 19);
        assert_eq!(perft(&startpos, &zobrist, 2), 289);
        assert_eq!(perft(&startpos, &zobrist, 3), 4442);
        assert_eq!(perft(&startpos, &zobrist, 4), 73584);
        assert_eq!(perft(&startpos, &zobrist, 5), 1_198_299);
        assert_eq!(perft(&startpos, &zobrist, 6), 19_870_403);
    }

    #[test]
    fn perft_test40() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("K7/8/2n5/1n6/8/8/8/k6N w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 3);
        assert_eq!(perft(&startpos, &zobrist, 2), 51);
        assert_eq!(perft(&startpos, &zobrist, 3), 345);
        assert_eq!(perft(&startpos, &zobrist, 4), 5301);
        assert_eq!(perft(&startpos, &zobrist, 5), 38348);
        assert_eq!(perft(&startpos, &zobrist, 6), 588_695);
    }

    #[test]
    fn perft_test41() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("k7/8/2N5/1N6/8/8/8/K6n w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 17);
        assert_eq!(perft(&startpos, &zobrist, 2), 54);
        assert_eq!(perft(&startpos, &zobrist, 3), 835);
        assert_eq!(perft(&startpos, &zobrist, 4), 5910);
        assert_eq!(perft(&startpos, &zobrist, 5), 92250);
        assert_eq!(perft(&startpos, &zobrist, 6), 688_780);
    }

    #[test]
    fn perft_test42() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("8/1n4N1/2k5/8/8/5K2/1N4n1/8 b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 15);
        assert_eq!(perft(&startpos, &zobrist, 2), 193);
        assert_eq!(perft(&startpos, &zobrist, 3), 2816);
        assert_eq!(perft(&startpos, &zobrist, 4), 40039);
        assert_eq!(perft(&startpos, &zobrist, 5), 582_642);
        assert_eq!(perft(&startpos, &zobrist, 6), 8_503_277);
    }

    #[test]
    fn perft_test43() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("8/1k6/8/5N2/8/4n3/8/2K5 b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 16);
        assert_eq!(perft(&startpos, &zobrist, 2), 180);
        assert_eq!(perft(&startpos, &zobrist, 3), 2290);
        assert_eq!(perft(&startpos, &zobrist, 4), 24640);
        assert_eq!(perft(&startpos, &zobrist, 5), 288_141);
        assert_eq!(perft(&startpos, &zobrist, 6), 3_147_566);
    }

    #[test]
    fn perft_test44() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("8/8/3K4/3Nn3/3nN3/4k3/8/8 b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 4);
        assert_eq!(perft(&startpos, &zobrist, 2), 68);
        assert_eq!(perft(&startpos, &zobrist, 3), 1118);
        assert_eq!(perft(&startpos, &zobrist, 4), 16199);
        assert_eq!(perft(&startpos, &zobrist, 5), 281_190);
        assert_eq!(perft(&startpos, &zobrist, 6), 4_405_103);
    }

    #[test]
    fn perft_test45() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("K7/8/2n5/1n6/8/8/8/k6N b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 17);
        assert_eq!(perft(&startpos, &zobrist, 2), 54);
        assert_eq!(perft(&startpos, &zobrist, 3), 835);
        assert_eq!(perft(&startpos, &zobrist, 4), 5910);
        assert_eq!(perft(&startpos, &zobrist, 5), 92250);
        assert_eq!(perft(&startpos, &zobrist, 6), 688_780);
    }

    #[test]
    fn perft_test46() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("k7/8/2N5/1N6/8/8/8/K6n b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 3);
        assert_eq!(perft(&startpos, &zobrist, 2), 51);
        assert_eq!(perft(&startpos, &zobrist, 3), 345);
        assert_eq!(perft(&startpos, &zobrist, 4), 5301);
        assert_eq!(perft(&startpos, &zobrist, 5), 38348);
        assert_eq!(perft(&startpos, &zobrist, 6), 588_695);
    }

    #[test]
    fn perft_test47() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("B6b/8/8/8/2K5/4k3/8/b6B w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 17);
        assert_eq!(perft(&startpos, &zobrist, 2), 278);
        assert_eq!(perft(&startpos, &zobrist, 3), 4607);
        assert_eq!(perft(&startpos, &zobrist, 4), 76778);
        assert_eq!(perft(&startpos, &zobrist, 5), 1_320_507);
        assert_eq!(perft(&startpos, &zobrist, 6), 22_823_890);
    }

    #[test]
    fn perft_test48() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("8/8/1B6/7b/7k/8/2B1b3/7K w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 21);
        assert_eq!(perft(&startpos, &zobrist, 2), 316);
        assert_eq!(perft(&startpos, &zobrist, 3), 5744);
        assert_eq!(perft(&startpos, &zobrist, 4), 93338);
        assert_eq!(perft(&startpos, &zobrist, 5), 1_713_368);
        assert_eq!(perft(&startpos, &zobrist, 6), 28_861_171);
    }

    #[test]
    fn perft_test49() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("k7/B7/1B6/1B6/8/8/8/K6b w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 21);
        assert_eq!(perft(&startpos, &zobrist, 2), 144);
        assert_eq!(perft(&startpos, &zobrist, 3), 3242);
        assert_eq!(perft(&startpos, &zobrist, 4), 32955);
        assert_eq!(perft(&startpos, &zobrist, 5), 787_524);
        assert_eq!(perft(&startpos, &zobrist, 6), 7_881_673);
    }

    #[test]
    fn perft_test50() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("K7/b7/1b6/1b6/8/8/8/k6B w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 7);
        assert_eq!(perft(&startpos, &zobrist, 2), 143);
        assert_eq!(perft(&startpos, &zobrist, 3), 1416);
        assert_eq!(perft(&startpos, &zobrist, 4), 31787);
        assert_eq!(perft(&startpos, &zobrist, 5), 310_862);
        assert_eq!(perft(&startpos, &zobrist, 6), 7_382_896);
    }

    #[test]
    fn perft_test51() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("B6b/8/8/8/2K5/5k2/8/b6B b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 6);
        assert_eq!(perft(&startpos, &zobrist, 2), 106);
        assert_eq!(perft(&startpos, &zobrist, 3), 1829);
        assert_eq!(perft(&startpos, &zobrist, 4), 31151);
        assert_eq!(perft(&startpos, &zobrist, 5), 530_585);
        assert_eq!(perft(&startpos, &zobrist, 6), 9_250_746);
    }

    #[test]
    fn perft_test52() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("8/8/1B6/7b/7k/8/2B1b3/7K b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 17);
        assert_eq!(perft(&startpos, &zobrist, 2), 309);
        assert_eq!(perft(&startpos, &zobrist, 3), 5133);
        assert_eq!(perft(&startpos, &zobrist, 4), 93603);
        assert_eq!(perft(&startpos, &zobrist, 5), 1_591_064);
        assert_eq!(perft(&startpos, &zobrist, 6), 29_027_891);
    }

    #[test]
    fn perft_test53() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("k7/B7/1B6/1B6/8/8/8/K6b b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 7);
        assert_eq!(perft(&startpos, &zobrist, 2), 143);
        assert_eq!(perft(&startpos, &zobrist, 3), 1416);
        assert_eq!(perft(&startpos, &zobrist, 4), 31787);
        assert_eq!(perft(&startpos, &zobrist, 5), 310_862);
        assert_eq!(perft(&startpos, &zobrist, 6), 7_382_896);
    }

    #[test]
    fn perft_test54() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("K7/b7/1b6/1b6/8/8/8/k6B b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 21);
        assert_eq!(perft(&startpos, &zobrist, 2), 144);
        assert_eq!(perft(&startpos, &zobrist, 3), 3242);
        assert_eq!(perft(&startpos, &zobrist, 4), 32955);
        assert_eq!(perft(&startpos, &zobrist, 5), 787_524);
        assert_eq!(perft(&startpos, &zobrist, 6), 7_881_673);
    }

    #[test]
    fn perft_test55() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("7k/RR6/8/8/8/8/rr6/7K w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 19);
        assert_eq!(perft(&startpos, &zobrist, 2), 275);
        assert_eq!(perft(&startpos, &zobrist, 3), 5300);
        assert_eq!(perft(&startpos, &zobrist, 4), 104_342);
        assert_eq!(perft(&startpos, &zobrist, 5), 2_161_211);
        assert_eq!(perft(&startpos, &zobrist, 6), 44_956_585);
    }

    #[test]
    fn perft_test56() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("R6r/8/8/2K5/5k2/8/8/r6R w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 36);
        assert_eq!(perft(&startpos, &zobrist, 2), 1027);
        assert_eq!(perft(&startpos, &zobrist, 3), 29215);
        assert_eq!(perft(&startpos, &zobrist, 4), 771_461);
        assert_eq!(perft(&startpos, &zobrist, 5), 20_506_480);
        assert_eq!(perft(&startpos, &zobrist, 6), 525_169_084);
    }

    #[test]
    fn perft_test57() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("7k/RR6/8/8/8/8/rr6/7K b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 19);
        assert_eq!(perft(&startpos, &zobrist, 2), 275);
        assert_eq!(perft(&startpos, &zobrist, 3), 5300);
        assert_eq!(perft(&startpos, &zobrist, 4), 104_342);
        assert_eq!(perft(&startpos, &zobrist, 5), 2_161_211);
        assert_eq!(perft(&startpos, &zobrist, 6), 44_956_585);
    }

    #[test]
    fn perft_test58() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("R6r/8/8/2K5/5k2/8/8/r6R b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 36);
        assert_eq!(perft(&startpos, &zobrist, 2), 1027);
        assert_eq!(perft(&startpos, &zobrist, 3), 29227);
        assert_eq!(perft(&startpos, &zobrist, 4), 771_368);
        assert_eq!(perft(&startpos, &zobrist, 5), 20_521_342);
        assert_eq!(perft(&startpos, &zobrist, 6), 524_966_748);
    }

    #[test]
    fn perft_test59() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("6kq/8/8/8/8/8/8/7K w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 2);
        assert_eq!(perft(&startpos, &zobrist, 2), 36);
        assert_eq!(perft(&startpos, &zobrist, 3), 143);
        assert_eq!(perft(&startpos, &zobrist, 4), 3637);
        assert_eq!(perft(&startpos, &zobrist, 5), 14893);
        assert_eq!(perft(&startpos, &zobrist, 6), 391_507);
    }

    #[test]
    fn perft_test60() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("6KQ/8/8/8/8/8/8/7k b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 2);
        assert_eq!(perft(&startpos, &zobrist, 2), 36);
        assert_eq!(perft(&startpos, &zobrist, 3), 143);
        assert_eq!(perft(&startpos, &zobrist, 4), 3637);
        assert_eq!(perft(&startpos, &zobrist, 5), 14893);
        assert_eq!(perft(&startpos, &zobrist, 6), 391_507);
    }

    #[test]
    fn perft_test61() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("K7/8/8/3Q4/4q3/8/8/7k w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 6);
        assert_eq!(perft(&startpos, &zobrist, 2), 35);
        assert_eq!(perft(&startpos, &zobrist, 3), 495);
        assert_eq!(perft(&startpos, &zobrist, 4), 8349);
        assert_eq!(perft(&startpos, &zobrist, 5), 166_741);
        assert_eq!(perft(&startpos, &zobrist, 6), 3_370_175);
    }

    #[test]
    fn perft_test62() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("6qk/8/8/8/8/8/8/7K b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 22);
        assert_eq!(perft(&startpos, &zobrist, 2), 43);
        assert_eq!(perft(&startpos, &zobrist, 3), 1015);
        assert_eq!(perft(&startpos, &zobrist, 4), 4167);
        assert_eq!(perft(&startpos, &zobrist, 5), 105_749);
        assert_eq!(perft(&startpos, &zobrist, 6), 419_369);
    }

    #[test]
    fn perft_test63() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("6KQ/8/8/8/8/8/8/7k b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 2);
        assert_eq!(perft(&startpos, &zobrist, 2), 36);
        assert_eq!(perft(&startpos, &zobrist, 3), 143);
        assert_eq!(perft(&startpos, &zobrist, 4), 3637);
        assert_eq!(perft(&startpos, &zobrist, 5), 14893);
        assert_eq!(perft(&startpos, &zobrist, 6), 391_507);
    }

    #[test]
    fn perft_test64() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("K7/8/8/3Q4/4q3/8/8/7k b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 6);
        assert_eq!(perft(&startpos, &zobrist, 2), 35);
        assert_eq!(perft(&startpos, &zobrist, 3), 495);
        assert_eq!(perft(&startpos, &zobrist, 4), 8349);
        assert_eq!(perft(&startpos, &zobrist, 5), 166_741);
        assert_eq!(perft(&startpos, &zobrist, 6), 3_370_175);
    }

    #[test]
    fn perft_test65() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("8/8/8/8/8/K7/P7/k7 w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 3);
        assert_eq!(perft(&startpos, &zobrist, 2), 7);
        assert_eq!(perft(&startpos, &zobrist, 3), 43);
        assert_eq!(perft(&startpos, &zobrist, 4), 199);
        assert_eq!(perft(&startpos, &zobrist, 5), 1347);
        assert_eq!(perft(&startpos, &zobrist, 6), 6249);
    }

    #[test]
    fn perft_test66() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("8/8/8/8/8/7K/7P/7k w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 3);
        assert_eq!(perft(&startpos, &zobrist, 2), 7);
        assert_eq!(perft(&startpos, &zobrist, 3), 43);
        assert_eq!(perft(&startpos, &zobrist, 4), 199);
        assert_eq!(perft(&startpos, &zobrist, 5), 1347);
        assert_eq!(perft(&startpos, &zobrist, 6), 6249);
    }

    #[test]
    fn perft_test67() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("K7/p7/k7/8/8/8/8/8 w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 1);
        assert_eq!(perft(&startpos, &zobrist, 2), 3);
        assert_eq!(perft(&startpos, &zobrist, 3), 12);
        assert_eq!(perft(&startpos, &zobrist, 4), 80);
        assert_eq!(perft(&startpos, &zobrist, 5), 342);
        assert_eq!(perft(&startpos, &zobrist, 6), 2343);
    }

    #[test]
    fn perft_test68() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("7K/7p/7k/8/8/8/8/8 w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 1);
        assert_eq!(perft(&startpos, &zobrist, 2), 3);
        assert_eq!(perft(&startpos, &zobrist, 3), 12);
        assert_eq!(perft(&startpos, &zobrist, 4), 80);
        assert_eq!(perft(&startpos, &zobrist, 5), 342);
        assert_eq!(perft(&startpos, &zobrist, 6), 2343);
    }

    #[test]
    fn perft_test69() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("8/2k1p3/3pP3/3P2K1/8/8/8/8 w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 7);
        assert_eq!(perft(&startpos, &zobrist, 2), 35);
        assert_eq!(perft(&startpos, &zobrist, 3), 210);
        assert_eq!(perft(&startpos, &zobrist, 4), 1091);
        assert_eq!(perft(&startpos, &zobrist, 5), 7028);
        assert_eq!(perft(&startpos, &zobrist, 6), 34834);
    }

    #[test]
    fn perft_test70() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("8/8/8/8/8/K7/P7/k7 b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 1);
        assert_eq!(perft(&startpos, &zobrist, 2), 3);
        assert_eq!(perft(&startpos, &zobrist, 3), 12);
        assert_eq!(perft(&startpos, &zobrist, 4), 80);
        assert_eq!(perft(&startpos, &zobrist, 5), 342);
        assert_eq!(perft(&startpos, &zobrist, 6), 2343);
    }

    #[test]
    fn perft_test71() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("8/8/8/8/8/7K/7P/7k b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 1);
        assert_eq!(perft(&startpos, &zobrist, 2), 3);
        assert_eq!(perft(&startpos, &zobrist, 3), 12);
        assert_eq!(perft(&startpos, &zobrist, 4), 80);
        assert_eq!(perft(&startpos, &zobrist, 5), 342);
        assert_eq!(perft(&startpos, &zobrist, 6), 2343);
    }

    #[test]
    fn perft_test72() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("K7/p7/k7/8/8/8/8/8 b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 3);
        assert_eq!(perft(&startpos, &zobrist, 2), 7);
        assert_eq!(perft(&startpos, &zobrist, 3), 43);
        assert_eq!(perft(&startpos, &zobrist, 4), 199);
        assert_eq!(perft(&startpos, &zobrist, 5), 1347);
        assert_eq!(perft(&startpos, &zobrist, 6), 6249);
    }

    #[test]
    fn perft_test73() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("7K/7p/7k/8/8/8/8/8 b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 3);
        assert_eq!(perft(&startpos, &zobrist, 2), 7);
        assert_eq!(perft(&startpos, &zobrist, 3), 43);
        assert_eq!(perft(&startpos, &zobrist, 4), 199);
        assert_eq!(perft(&startpos, &zobrist, 5), 1347);
        assert_eq!(perft(&startpos, &zobrist, 6), 6249);
    }

    #[test]
    fn perft_test74() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("8/2k1p3/3pP3/3P2K1/8/8/8/8 b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 5);
        assert_eq!(perft(&startpos, &zobrist, 2), 35);
        assert_eq!(perft(&startpos, &zobrist, 3), 182);
        assert_eq!(perft(&startpos, &zobrist, 4), 1091);
        assert_eq!(perft(&startpos, &zobrist, 5), 5408);
        assert_eq!(perft(&startpos, &zobrist, 6), 34822);
    }

    #[test]
    fn perft_test75() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("8/8/8/8/8/4k3/4P3/4K3 w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 2);
        assert_eq!(perft(&startpos, &zobrist, 2), 8);
        assert_eq!(perft(&startpos, &zobrist, 3), 44);
        assert_eq!(perft(&startpos, &zobrist, 4), 282);
        assert_eq!(perft(&startpos, &zobrist, 5), 1814);
        assert_eq!(perft(&startpos, &zobrist, 6), 11848);
    }

    #[test]
    fn perft_test76() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("4k3/4p3/4K3/8/8/8/8/8 b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 2);
        assert_eq!(perft(&startpos, &zobrist, 2), 8);
        assert_eq!(perft(&startpos, &zobrist, 3), 44);
        assert_eq!(perft(&startpos, &zobrist, 4), 282);
        assert_eq!(perft(&startpos, &zobrist, 5), 1814);
        assert_eq!(perft(&startpos, &zobrist, 6), 11848);
    }

    #[test]
    fn perft_test77() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("8/8/7k/7p/7P/7K/8/8 w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 3);
        assert_eq!(perft(&startpos, &zobrist, 2), 9);
        assert_eq!(perft(&startpos, &zobrist, 3), 57);
        assert_eq!(perft(&startpos, &zobrist, 4), 360);
        assert_eq!(perft(&startpos, &zobrist, 5), 1969);
        assert_eq!(perft(&startpos, &zobrist, 6), 10724);
    }

    #[test]
    fn perft_test78() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("8/8/k7/p7/P7/K7/8/8 w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 3);
        assert_eq!(perft(&startpos, &zobrist, 2), 9);
        assert_eq!(perft(&startpos, &zobrist, 3), 57);
        assert_eq!(perft(&startpos, &zobrist, 4), 360);
        assert_eq!(perft(&startpos, &zobrist, 5), 1969);
        assert_eq!(perft(&startpos, &zobrist, 6), 10724);
    }

    #[test]
    fn perft_test79() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("8/8/3k4/3p4/3P4/3K4/8/8 w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 5);
        assert_eq!(perft(&startpos, &zobrist, 2), 25);
        assert_eq!(perft(&startpos, &zobrist, 3), 180);
        assert_eq!(perft(&startpos, &zobrist, 4), 1294);
        assert_eq!(perft(&startpos, &zobrist, 5), 8296);
        assert_eq!(perft(&startpos, &zobrist, 6), 53138);
    }

    #[test]
    fn perft_test80() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("8/3k4/3p4/8/3P4/3K4/8/8 w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 8);
        assert_eq!(perft(&startpos, &zobrist, 2), 61);
        assert_eq!(perft(&startpos, &zobrist, 3), 483);
        assert_eq!(perft(&startpos, &zobrist, 4), 3213);
        assert_eq!(perft(&startpos, &zobrist, 5), 23599);
        assert_eq!(perft(&startpos, &zobrist, 6), 157_093);
    }

    #[test]
    fn perft_test81() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("8/8/3k4/3p4/8/3P4/3K4/8 w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 8);
        assert_eq!(perft(&startpos, &zobrist, 2), 61);
        assert_eq!(perft(&startpos, &zobrist, 3), 411);
        assert_eq!(perft(&startpos, &zobrist, 4), 3213);
        assert_eq!(perft(&startpos, &zobrist, 5), 21637);
        assert_eq!(perft(&startpos, &zobrist, 6), 158_065);
    }

    #[test]
    fn perft_test82() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("k7/8/3p4/8/3P4/8/8/7K w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 4);
        assert_eq!(perft(&startpos, &zobrist, 2), 15);
        assert_eq!(perft(&startpos, &zobrist, 3), 90);
        assert_eq!(perft(&startpos, &zobrist, 4), 534);
        assert_eq!(perft(&startpos, &zobrist, 5), 3450);
        assert_eq!(perft(&startpos, &zobrist, 6), 20960);
    }

    #[test]
    fn perft_test83() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("8/8/7k/7p/7P/7K/8/8 b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 3);
        assert_eq!(perft(&startpos, &zobrist, 2), 9);
        assert_eq!(perft(&startpos, &zobrist, 3), 57);
        assert_eq!(perft(&startpos, &zobrist, 4), 360);
        assert_eq!(perft(&startpos, &zobrist, 5), 1969);
        assert_eq!(perft(&startpos, &zobrist, 6), 10724);
    }

    #[test]
    fn perft_test84() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("8/8/k7/p7/P7/K7/8/8 b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 3);
        assert_eq!(perft(&startpos, &zobrist, 2), 9);
        assert_eq!(perft(&startpos, &zobrist, 3), 57);
        assert_eq!(perft(&startpos, &zobrist, 4), 360);
        assert_eq!(perft(&startpos, &zobrist, 5), 1969);
        assert_eq!(perft(&startpos, &zobrist, 6), 10724);
    }

    #[test]
    fn perft_test85() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("8/8/3k4/3p4/3P4/3K4/8/8 b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 5);
        assert_eq!(perft(&startpos, &zobrist, 2), 25);
        assert_eq!(perft(&startpos, &zobrist, 3), 180);
        assert_eq!(perft(&startpos, &zobrist, 4), 1294);
        assert_eq!(perft(&startpos, &zobrist, 5), 8296);
        assert_eq!(perft(&startpos, &zobrist, 6), 53138);
    }

    #[test]
    fn perft_test86() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("8/3k4/3p4/8/3P4/3K4/8/8 b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 8);
        assert_eq!(perft(&startpos, &zobrist, 2), 61);
        assert_eq!(perft(&startpos, &zobrist, 3), 411);
        assert_eq!(perft(&startpos, &zobrist, 4), 3213);
        assert_eq!(perft(&startpos, &zobrist, 5), 21637);
        assert_eq!(perft(&startpos, &zobrist, 6), 158_065);
    }

    #[test]
    fn perft_test87() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("8/8/3k4/3p4/8/3P4/3K4/8 b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 8);
        assert_eq!(perft(&startpos, &zobrist, 2), 61);
        assert_eq!(perft(&startpos, &zobrist, 3), 483);
        assert_eq!(perft(&startpos, &zobrist, 4), 3213);
        assert_eq!(perft(&startpos, &zobrist, 5), 23599);
        assert_eq!(perft(&startpos, &zobrist, 6), 157_093);
    }

    #[test]
    fn perft_test88() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("k7/8/3p4/8/3P4/8/8/7K b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 4);
        assert_eq!(perft(&startpos, &zobrist, 2), 15);
        assert_eq!(perft(&startpos, &zobrist, 3), 89);
        assert_eq!(perft(&startpos, &zobrist, 4), 537);
        assert_eq!(perft(&startpos, &zobrist, 5), 3309);
        assert_eq!(perft(&startpos, &zobrist, 6), 21104);
    }

    #[test]
    fn perft_test89() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("7k/3p4/8/8/3P4/8/8/K7 w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 4);
        assert_eq!(perft(&startpos, &zobrist, 2), 19);
        assert_eq!(perft(&startpos, &zobrist, 3), 117);
        assert_eq!(perft(&startpos, &zobrist, 4), 720);
        assert_eq!(perft(&startpos, &zobrist, 5), 4661);
        assert_eq!(perft(&startpos, &zobrist, 6), 32191);
    }

    #[test]
    fn perft_test90() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("7k/8/8/3p4/8/8/3P4/K7 w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 5);
        assert_eq!(perft(&startpos, &zobrist, 2), 19);
        assert_eq!(perft(&startpos, &zobrist, 3), 116);
        assert_eq!(perft(&startpos, &zobrist, 4), 716);
        assert_eq!(perft(&startpos, &zobrist, 5), 4786);
        assert_eq!(perft(&startpos, &zobrist, 6), 30980);
    }

    #[test]
    fn perft_test91() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("k7/8/8/7p/6P1/8/8/K7 w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 5);
        assert_eq!(perft(&startpos, &zobrist, 2), 22);
        assert_eq!(perft(&startpos, &zobrist, 3), 139);
        assert_eq!(perft(&startpos, &zobrist, 4), 877);
        assert_eq!(perft(&startpos, &zobrist, 5), 6112);
        assert_eq!(perft(&startpos, &zobrist, 6), 41874);
    }

    #[test]
    fn perft_test92() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("k7/8/7p/8/8/6P1/8/K7 w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 4);
        assert_eq!(perft(&startpos, &zobrist, 2), 16);
        assert_eq!(perft(&startpos, &zobrist, 3), 101);
        assert_eq!(perft(&startpos, &zobrist, 4), 637);
        assert_eq!(perft(&startpos, &zobrist, 5), 4354);
        assert_eq!(perft(&startpos, &zobrist, 6), 29679);
    }

    #[test]
    fn perft_test93() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("k7/8/8/6p1/7P/8/8/K7 w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 5);
        assert_eq!(perft(&startpos, &zobrist, 2), 22);
        assert_eq!(perft(&startpos, &zobrist, 3), 139);
        assert_eq!(perft(&startpos, &zobrist, 4), 877);
        assert_eq!(perft(&startpos, &zobrist, 5), 6112);
        assert_eq!(perft(&startpos, &zobrist, 6), 41874);
    }

    #[test]
    fn perft_test94() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("k7/8/6p1/8/8/7P/8/K7 w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 4);
        assert_eq!(perft(&startpos, &zobrist, 2), 16);
        assert_eq!(perft(&startpos, &zobrist, 3), 101);
        assert_eq!(perft(&startpos, &zobrist, 4), 637);
        assert_eq!(perft(&startpos, &zobrist, 5), 4354);
        assert_eq!(perft(&startpos, &zobrist, 6), 29679);
    }

    #[test]
    fn perft_test95() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("k7/8/8/3p4/4p3/8/8/7K w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 3);
        assert_eq!(perft(&startpos, &zobrist, 2), 15);
        assert_eq!(perft(&startpos, &zobrist, 3), 84);
        assert_eq!(perft(&startpos, &zobrist, 4), 573);
        assert_eq!(perft(&startpos, &zobrist, 5), 3013);
        assert_eq!(perft(&startpos, &zobrist, 6), 22886);
    }

    #[test]
    fn perft_test96() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("k7/8/3p4/8/8/4P3/8/7K w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 4);
        assert_eq!(perft(&startpos, &zobrist, 2), 16);
        assert_eq!(perft(&startpos, &zobrist, 3), 101);
        assert_eq!(perft(&startpos, &zobrist, 4), 637);
        assert_eq!(perft(&startpos, &zobrist, 5), 4271);
        assert_eq!(perft(&startpos, &zobrist, 6), 28662);
    }

    #[test]
    fn perft_test97() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("7k/3p4/8/8/3P4/8/8/K7 b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 5);
        assert_eq!(perft(&startpos, &zobrist, 2), 19);
        assert_eq!(perft(&startpos, &zobrist, 3), 117);
        assert_eq!(perft(&startpos, &zobrist, 4), 720);
        assert_eq!(perft(&startpos, &zobrist, 5), 5014);
        assert_eq!(perft(&startpos, &zobrist, 6), 32167);
    }

    #[test]
    fn perft_test98() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("7k/8/8/3p4/8/8/3P4/K7 b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 4);
        assert_eq!(perft(&startpos, &zobrist, 2), 19);
        assert_eq!(perft(&startpos, &zobrist, 3), 117);
        assert_eq!(perft(&startpos, &zobrist, 4), 712);
        assert_eq!(perft(&startpos, &zobrist, 5), 4658);
        assert_eq!(perft(&startpos, &zobrist, 6), 30749);
    }

    #[test]
    fn perft_test99() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("k7/8/8/7p/6P1/8/8/K7 b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 5);
        assert_eq!(perft(&startpos, &zobrist, 2), 22);
        assert_eq!(perft(&startpos, &zobrist, 3), 139);
        assert_eq!(perft(&startpos, &zobrist, 4), 877);
        assert_eq!(perft(&startpos, &zobrist, 5), 6112);
        assert_eq!(perft(&startpos, &zobrist, 6), 41874);
    }

    #[test]
    fn perft_test100() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("k7/8/7p/8/8/6P1/8/K7 b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 4);
        assert_eq!(perft(&startpos, &zobrist, 2), 16);
        assert_eq!(perft(&startpos, &zobrist, 3), 101);
        assert_eq!(perft(&startpos, &zobrist, 4), 637);
        assert_eq!(perft(&startpos, &zobrist, 5), 4354);
        assert_eq!(perft(&startpos, &zobrist, 6), 29679);
    }

    #[test]
    fn perft_test101() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("k7/8/8/6p1/7P/8/8/K7 b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 5);
        assert_eq!(perft(&startpos, &zobrist, 2), 22);
        assert_eq!(perft(&startpos, &zobrist, 3), 139);
        assert_eq!(perft(&startpos, &zobrist, 4), 877);
        assert_eq!(perft(&startpos, &zobrist, 5), 6112);
        assert_eq!(perft(&startpos, &zobrist, 6), 41874);
    }

    #[test]
    fn perft_test102() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("k7/8/6p1/8/8/7P/8/K7 b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 4);
        assert_eq!(perft(&startpos, &zobrist, 2), 16);
        assert_eq!(perft(&startpos, &zobrist, 3), 101);
        assert_eq!(perft(&startpos, &zobrist, 4), 637);
        assert_eq!(perft(&startpos, &zobrist, 5), 4354);
        assert_eq!(perft(&startpos, &zobrist, 6), 29679);
    }

    #[test]
    fn perft_test103() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("k7/8/8/3p4/4p3/8/8/7K b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 5);
        assert_eq!(perft(&startpos, &zobrist, 2), 15);
        assert_eq!(perft(&startpos, &zobrist, 3), 102);
        assert_eq!(perft(&startpos, &zobrist, 4), 569);
        assert_eq!(perft(&startpos, &zobrist, 5), 4337);
        assert_eq!(perft(&startpos, &zobrist, 6), 22579);
    }

    #[test]
    fn perft_test104() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("k7/8/3p4/8/8/4P3/8/7K b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 4);
        assert_eq!(perft(&startpos, &zobrist, 2), 16);
        assert_eq!(perft(&startpos, &zobrist, 3), 101);
        assert_eq!(perft(&startpos, &zobrist, 4), 637);
        assert_eq!(perft(&startpos, &zobrist, 5), 4271);
        assert_eq!(perft(&startpos, &zobrist, 6), 28662);
    }

    #[test]
    fn perft_test105() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("7k/8/8/p7/1P6/8/8/7K w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 5);
        assert_eq!(perft(&startpos, &zobrist, 2), 22);
        assert_eq!(perft(&startpos, &zobrist, 3), 139);
        assert_eq!(perft(&startpos, &zobrist, 4), 877);
        assert_eq!(perft(&startpos, &zobrist, 5), 6112);
        assert_eq!(perft(&startpos, &zobrist, 6), 41874);
    }

    #[test]
    fn perft_test106() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("7k/8/p7/8/8/1P6/8/7K w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 4);
        assert_eq!(perft(&startpos, &zobrist, 2), 16);
        assert_eq!(perft(&startpos, &zobrist, 3), 101);
        assert_eq!(perft(&startpos, &zobrist, 4), 637);
        assert_eq!(perft(&startpos, &zobrist, 5), 4354);
        assert_eq!(perft(&startpos, &zobrist, 6), 29679);
    }

    #[test]
    fn perft_test107() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("7k/8/8/1p6/P7/8/8/7K w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 5);
        assert_eq!(perft(&startpos, &zobrist, 2), 22);
        assert_eq!(perft(&startpos, &zobrist, 3), 139);
        assert_eq!(perft(&startpos, &zobrist, 4), 877);
        assert_eq!(perft(&startpos, &zobrist, 5), 6112);
        assert_eq!(perft(&startpos, &zobrist, 6), 41874);
    }

    #[test]
    fn perft_test108() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("7k/8/1p6/8/8/P7/8/7K w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 4);
        assert_eq!(perft(&startpos, &zobrist, 2), 16);
        assert_eq!(perft(&startpos, &zobrist, 3), 101);
        assert_eq!(perft(&startpos, &zobrist, 4), 637);
        assert_eq!(perft(&startpos, &zobrist, 5), 4354);
        assert_eq!(perft(&startpos, &zobrist, 6), 29679);
    }

    #[test]
    fn perft_test109() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("k7/7p/8/8/8/8/6P1/K7 w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 5);
        assert_eq!(perft(&startpos, &zobrist, 2), 25);
        assert_eq!(perft(&startpos, &zobrist, 3), 161);
        assert_eq!(perft(&startpos, &zobrist, 4), 1035);
        assert_eq!(perft(&startpos, &zobrist, 5), 7574);
        assert_eq!(perft(&startpos, &zobrist, 6), 55338);
    }

    #[test]
    fn perft_test110() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("k7/6p1/8/8/8/8/7P/K7 w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 5);
        assert_eq!(perft(&startpos, &zobrist, 2), 25);
        assert_eq!(perft(&startpos, &zobrist, 3), 161);
        assert_eq!(perft(&startpos, &zobrist, 4), 1035);
        assert_eq!(perft(&startpos, &zobrist, 5), 7574);
        assert_eq!(perft(&startpos, &zobrist, 6), 55338);
    }

    #[test]
    fn perft_test111() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("3k4/3pp3/8/8/8/8/3PP3/3K4 w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 7);
        assert_eq!(perft(&startpos, &zobrist, 2), 49);
        assert_eq!(perft(&startpos, &zobrist, 3), 378);
        assert_eq!(perft(&startpos, &zobrist, 4), 2902);
        assert_eq!(perft(&startpos, &zobrist, 5), 24122);
        assert_eq!(perft(&startpos, &zobrist, 6), 199_002);
    }

    #[test]
    fn perft_test112() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("7k/8/8/p7/1P6/8/8/7K b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 5);
        assert_eq!(perft(&startpos, &zobrist, 2), 22);
        assert_eq!(perft(&startpos, &zobrist, 3), 139);
        assert_eq!(perft(&startpos, &zobrist, 4), 877);
        assert_eq!(perft(&startpos, &zobrist, 5), 6112);
        assert_eq!(perft(&startpos, &zobrist, 6), 41874);
    }

    #[test]
    fn perft_test113() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("7k/8/p7/8/8/1P6/8/7K b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 4);
        assert_eq!(perft(&startpos, &zobrist, 2), 16);
        assert_eq!(perft(&startpos, &zobrist, 3), 101);
        assert_eq!(perft(&startpos, &zobrist, 4), 637);
        assert_eq!(perft(&startpos, &zobrist, 5), 4354);
        assert_eq!(perft(&startpos, &zobrist, 6), 29679);
    }

    #[test]
    fn perft_test114() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("7k/8/8/1p6/P7/8/8/7K b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 5);
        assert_eq!(perft(&startpos, &zobrist, 2), 22);
        assert_eq!(perft(&startpos, &zobrist, 3), 139);
        assert_eq!(perft(&startpos, &zobrist, 4), 877);
        assert_eq!(perft(&startpos, &zobrist, 5), 6112);
        assert_eq!(perft(&startpos, &zobrist, 6), 41874);
    }

    #[test]
    fn perft_test115() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("7k/8/1p6/8/8/P7/8/7K b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 4);
        assert_eq!(perft(&startpos, &zobrist, 2), 16);
        assert_eq!(perft(&startpos, &zobrist, 3), 101);
        assert_eq!(perft(&startpos, &zobrist, 4), 637);
        assert_eq!(perft(&startpos, &zobrist, 5), 4354);
        assert_eq!(perft(&startpos, &zobrist, 6), 29679);
    }

    #[test]
    fn perft_test116() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("k7/7p/8/8/8/8/6P1/K7 b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 5);
        assert_eq!(perft(&startpos, &zobrist, 2), 25);
        assert_eq!(perft(&startpos, &zobrist, 3), 161);
        assert_eq!(perft(&startpos, &zobrist, 4), 1035);
        assert_eq!(perft(&startpos, &zobrist, 5), 7574);
        assert_eq!(perft(&startpos, &zobrist, 6), 55338);
    }

    #[test]
    fn perft_test117() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("k7/6p1/8/8/8/8/7P/K7 b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 5);
        assert_eq!(perft(&startpos, &zobrist, 2), 25);
        assert_eq!(perft(&startpos, &zobrist, 3), 161);
        assert_eq!(perft(&startpos, &zobrist, 4), 1035);
        assert_eq!(perft(&startpos, &zobrist, 5), 7574);
        assert_eq!(perft(&startpos, &zobrist, 6), 55338);
    }

    #[test]
    fn perft_test118() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("3k4/3pp3/8/8/8/8/3PP3/3K4 b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 7);
        assert_eq!(perft(&startpos, &zobrist, 2), 49);
        assert_eq!(perft(&startpos, &zobrist, 3), 378);
        assert_eq!(perft(&startpos, &zobrist, 4), 2902);
        assert_eq!(perft(&startpos, &zobrist, 5), 24122);
        assert_eq!(perft(&startpos, &zobrist, 6), 199_002);
    }

    #[test]
    fn perft_test119() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("8/Pk6/8/8/8/8/6Kp/8 w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 11);
        assert_eq!(perft(&startpos, &zobrist, 2), 97);
        assert_eq!(perft(&startpos, &zobrist, 3), 887);
        assert_eq!(perft(&startpos, &zobrist, 4), 8048);
        assert_eq!(perft(&startpos, &zobrist, 5), 90606);
        assert_eq!(perft(&startpos, &zobrist, 6), 1_030_499);
    }

    #[test]
    fn perft_test120() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("n1n5/1Pk5/8/8/8/8/5Kp1/5N1N w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 24);
        assert_eq!(perft(&startpos, &zobrist, 2), 421);
        assert_eq!(perft(&startpos, &zobrist, 3), 7421);
        assert_eq!(perft(&startpos, &zobrist, 4), 124_608);
        assert_eq!(perft(&startpos, &zobrist, 5), 2_193_768);
        assert_eq!(perft(&startpos, &zobrist, 6), 37_665_329);
    }

    #[test]
    fn perft_test121() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("8/PPPk4/8/8/8/8/4Kppp/8 w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 18);
        assert_eq!(perft(&startpos, &zobrist, 2), 270);
        assert_eq!(perft(&startpos, &zobrist, 3), 4699);
        assert_eq!(perft(&startpos, &zobrist, 4), 79355);
        assert_eq!(perft(&startpos, &zobrist, 5), 1_533_145);
        assert_eq!(perft(&startpos, &zobrist, 6), 28_859_283);
    }

    #[test]
    fn perft_test122() {
        let zobrist = Zobrist::new();
        let startpos =
            Board::from_fen("n1n5/PPPk4/8/8/8/8/4Kppp/5N1N w - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 24);
        assert_eq!(perft(&startpos, &zobrist, 2), 496);
        assert_eq!(perft(&startpos, &zobrist, 3), 9483);
        assert_eq!(perft(&startpos, &zobrist, 4), 182_838);
        assert_eq!(perft(&startpos, &zobrist, 5), 3_605_103);
        assert_eq!(perft(&startpos, &zobrist, 6), 71_179_139);
    }

    #[test]
    fn perft_test123() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("8/Pk6/8/8/8/8/6Kp/8 b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 11);
        assert_eq!(perft(&startpos, &zobrist, 2), 97);
        assert_eq!(perft(&startpos, &zobrist, 3), 887);
        assert_eq!(perft(&startpos, &zobrist, 4), 8048);
        assert_eq!(perft(&startpos, &zobrist, 5), 90606);
        assert_eq!(perft(&startpos, &zobrist, 6), 1_030_499);
    }

    #[test]
    fn perft_test124() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("n1n5/1Pk5/8/8/8/8/5Kp1/5N1N b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 24);
        assert_eq!(perft(&startpos, &zobrist, 2), 421);
        assert_eq!(perft(&startpos, &zobrist, 3), 7421);
        assert_eq!(perft(&startpos, &zobrist, 4), 124_608);
        assert_eq!(perft(&startpos, &zobrist, 5), 2_193_768);
        assert_eq!(perft(&startpos, &zobrist, 6), 37_665_329);
    }

    #[test]
    fn perft_test125() {
        let zobrist = Zobrist::new();
        let startpos = Board::from_fen("8/PPPk4/8/8/8/8/4Kppp/8 b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 18);
        assert_eq!(perft(&startpos, &zobrist, 2), 270);
        assert_eq!(perft(&startpos, &zobrist, 3), 4699);
        assert_eq!(perft(&startpos, &zobrist, 4), 79355);
        assert_eq!(perft(&startpos, &zobrist, 5), 1_533_145);
        assert_eq!(perft(&startpos, &zobrist, 6), 28_859_283);
    }

    #[test]
    fn perft_test126() {
        let zobrist = Zobrist::new();
        let startpos =
            Board::from_fen("n1n5/PPPk4/8/8/8/8/4Kppp/5N1N b - - 0 1", &zobrist).unwrap();

        assert_eq!(perft(&startpos, &zobrist, 1), 24);
        assert_eq!(perft(&startpos, &zobrist, 2), 496);
        assert_eq!(perft(&startpos, &zobrist, 3), 9483);
        assert_eq!(perft(&startpos, &zobrist, 4), 182_838);
        assert_eq!(perft(&startpos, &zobrist, 5), 3_605_103);
        assert_eq!(perft(&startpos, &zobrist, 6), 71_179_139);
    }
}
