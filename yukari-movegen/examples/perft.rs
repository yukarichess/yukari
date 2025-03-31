use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

use tinyvec::ArrayVec;
use yukari_movegen::{Board, Move};

#[derive(Default)]
#[repr(align(16))]
pub struct PerftEntry {
    key: AtomicU64,
    data: AtomicU64,
}

const _PERFT_ENTRY_IS_16_BYTE: () = assert!(std::mem::size_of::<PerftEntry>() == 16);

pub fn allocate_perft_tt(megabytes: usize) -> Vec<PerftEntry> {
    let target_bytes = megabytes * 1024 * 1024;

    let mut size = 1_usize;
    loop {
        if size > target_bytes {
            break;
        }
        size *= 2;
    }
    size /= 2;
    size /= std::mem::size_of::<PerftEntry>();

    let mut tt: Vec<PerftEntry> = Vec::new();
    tt.resize_with(size, Default::default);
    println!("# Allocated {} bytes of perft hash", size * std::mem::size_of::<PerftEntry>());
    tt
}

/// Count the number of legal chess positions after N moves.
#[inline]
#[must_use]
pub fn perft_with_hash(board: &Board, depth: u32, tt: &[PerftEntry]) -> u64 {
    if depth == 0 {
        1
    } else if depth == 1 {
        let moves: [Move; 256] = [Move::default(); 256];
        let mut moves = ArrayVec::from(moves);
        moves.set_len(0);
        board.generate(&mut moves);
        moves.len() as u64
    } else {
        {
            let entry = (board.hash() & ((tt.len() - 1) as u64)) as usize;
            let entry = &tt[entry];

            let entry_key = entry.key.load(Ordering::Relaxed);
            let entry_data = entry.data.load(Ordering::Relaxed);
            let entry_depth = (entry_data >> 56) as u32;
            let entry_nodes = entry_data & 0x00FF_FFFF_FFFF_FFFF;
            if entry_key ^ entry_data == board.hash() && entry_depth == depth {
                return entry_nodes;
            }
        }

        let moves: [Move; 256] = [Move::default(); 256];
        let mut moves = ArrayVec::from(moves);
        moves.set_len(0);
        board.generate(&mut moves);

        let mut count = 0;
        for m in moves {
            let board = board.make(m);
            count += perft_with_hash(&board, depth - 1, tt);
        }

        {
            let entry = (board.hash() & ((tt.len() - 1) as u64)) as usize;
            let entry = &tt[entry];

            let entry_data = u64::from(depth) << 56 | count;
            entry.key.store(board.hash() ^ entry_data, Ordering::Relaxed);
            entry.data.store(entry_data, Ordering::Relaxed);
        }

        count
    }
}

#[must_use]
pub fn divide(board: &Board, depth: u32, tt: &[PerftEntry]) -> u64 {
    if depth == 0 {
        1
    } else {
        let moves: [Move; 256] = [Move::default(); 256];
        let mut moves = ArrayVec::from(moves);
        moves.set_len(0);
        board.generate(&mut moves);

        moves
            .iter()
            .map(|m| {
                let board = board.make(*m);
                let nodes = perft_with_hash(&board, depth - 1, tt);
                println!("{m} {nodes}");
                nodes
            })
            .sum()
    }
}

fn main() {
    let fen = std::env::args().nth(1).expect("Please provide a FEN string wrapped in quotes or the string 'bench' as argument");
    let depth = std::env::args()
        .nth(2)
        .expect("Please provide a FEN string wrapped in quotes or the string 'bench' as argument")
        .parse::<u32>()
        .expect("Please provide a FEN string wrapped in quotes or the string 'bench' as argument");
    let board = Board::from_fen(if fen == "startpos" {
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
    } else if fen == "kiwipete" {
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1"
    } else {
        &fen
    })
    .unwrap();
    //let nodes = divide(&startpos, &zobrist, depth);
    let tt = allocate_perft_tt(256);
    let start = Instant::now();
    let nodes = divide(&board, depth, &tt);
    println!("Perft {depth}: {nodes}");
    println!("time: {:.3}s", Instant::now().duration_since(start).as_secs_f32());
}
