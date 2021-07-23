use yukari::Search;
use yukari_movegen::{Board, Zobrist};
use tinyvec::ArrayVec;

use std::time::Instant;

fn main() {
    let fen = &std::env::args().nth(1).expect("Please provide a FEN string or 'bench'");
    let zobrist = Zobrist::new();
    let board = Board::from_fen(if fen == "bench" {
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1"
    } else {
        fen
    }, &zobrist).unwrap();

    let mut s = Search::new(None, &zobrist);
    let start = Instant::now();
    for depth in 1..=10 {
        let mut keystack = Vec::new();
        let mut pv = ArrayVec::new();
        pv.set_len(0);
        let score = s.search_root(&board, depth, &mut pv, &mut keystack);
        let now = Instant::now().duration_since(start);
        print!(
            "{} {:.2} {} {} ",
            depth,
            score,
            now.as_millis() / 10,
            s.nodes() + s.qnodes()
        );
        for m in pv {
            print!("{} ", m);
        }
        println!();
    }
    println!(
        "# QS: {:.3}%",
        (100 * s.qnodes()) as f64 / (s.nodes() as f64 + s.qnodes() as f64)
    );
    println!(
        "# Branching factor: {:.3} (AB); {:.3} (QS); {:.3} overall",
        (s.nodes() as f64).powf(0.1),
        (s.qnodes() as f64).powf(0.1),
        ((s.nodes() + s.qnodes()) as f64).powf(0.1)
    );
    println!(
        "# Nullmove success: {:.3}%",
        s.nullmove_success()
    );
}
