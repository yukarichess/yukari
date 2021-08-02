use yukari::tt::TranspositionTable;
use yukari_movegen::{Board, Move, Zobrist};
use rayon::prelude::*;
use tinyvec::ArrayVec;

/// Count the number of legal chess positions after N moves.
#[inline]
#[must_use]
pub fn perft(board: &Board, zobrist: &Zobrist, tt: &mut TranspositionTable<(u32, u64)>, depth: u32) -> u64 {
    if depth == 0 {
        1
    } else if depth == 1 {
        let moves: [Move; 256] = [Move::default(); 256];
        let mut moves = ArrayVec::from(moves);
        moves.set_len(0);
        board.generate(&mut moves);
        moves.len() as u64
    } else {
        
        if let Some(&(entry_depth, count)) = tt.get(board.hash()) {
            if entry_depth == depth {
                return count;
            }
        }
        
        let moves: [Move; 256] = [Move::default(); 256];
        let mut moves = ArrayVec::from(moves);
        moves.set_len(0);
        board.generate(&mut moves);

        let mut count = 0;
        for m in moves {
            let board = board.make(m, zobrist);
            count += perft(&board, zobrist, tt, depth - 1);
        }
        tt.set(board.hash(), (depth, count));
        count
    }
}

fn main() {
    let zobrist = Zobrist::new();
    let startpos = Board::startpos(&zobrist); //Board::from_fen("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1", &zobrist).unwrap();
    let depth = 6;
    let mut tt = TranspositionTable::new(1024*1024*20);
    let nodes = perft(&startpos, &zobrist, &mut tt, depth);
    println!("Perft {}: {}", depth, nodes);
}
