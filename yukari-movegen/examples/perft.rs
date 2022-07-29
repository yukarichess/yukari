use rayon::prelude::*;
use tinyvec::ArrayVec;
use yukari_movegen::{perft, Board, Move, Zobrist};

#[must_use]
pub fn divide(board: &Board, zobrist: &Zobrist, depth: u32) -> u64 {
    if depth == 0 {
        1
    } else {
        let moves: [Move; 256] = [Move::default(); 256];
        let mut moves = ArrayVec::from(moves);
        moves.set_len(0);
        board.generate(&mut moves);

        moves
            .par_iter()
            .map(|m| {
                let board = board.make(*m, zobrist);
                let nodes = perft(&board, zobrist, depth - 1);
                println!("{} {}", m, nodes);
                nodes
            })
            .sum()
    }
}

fn main() {
    let zobrist = Zobrist::new();
    let startpos = Board::startpos(&zobrist); //Board::from_fen("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1", &zobrist).unwrap();
    let depth = 6;
    //let nodes = divide(&startpos, &zobrist, depth);
    let nodes = perft(&startpos, &zobrist, depth);
    println!("Perft {}: {}", depth, nodes);
}
