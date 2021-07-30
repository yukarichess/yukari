use std::{fs::File, io::Read};

use yukari::Tune;
use yukari_movegen::{Board, Zobrist};
use revad::tape::Tape;

fn main() {
    const EPOCHS: usize = 500_000;
    let mut weights = [0.0; 780];

    weights[0] = 100.0;
    weights[1] = 300.0;
    weights[2] = 300.0;
    weights[3] = 500.0;
    weights[4] = 900.0;

    weights[6] = 100.0;
    weights[7] = 300.0;
    weights[8] = 300.0;
    weights[9] = 500.0;
    weights[10] = 900.0;

    println!("Loading FENs...");

    let zobrist = Zobrist::new();
    let boards = {
        let mut boards = Vec::new();
        let mut s = String::new();
        let mut f = File::open("ccrl4040_shuffled_5M.epd").unwrap();
        f.read_to_string(&mut s).unwrap();

        for line in s.lines() {
            boards.push(Board::from_fen(line, &zobrist).unwrap());
        }
        boards
    };

    for epoch in 1..=EPOCHS {
        let tape = Tape::new();
        let mut tune = Tune::new(&tape);
        tune.set_state(&tape, &weights);

        let grads = tune.tune(&tape, &boards, &zobrist);

        let td = grads.iter().map(|(_, td)| td.abs()).sum::<f64>();

        if epoch % 100 == 0 {
            println!("iter: {:>6} |td|: {:.6}", epoch, td);
        }

        const ALPHA: f64 = 1.0;
        if epoch == EPOCHS {
            tune.dump();
        }

        let weights_var = tune.get_state();

        for (index, weight) in weights_var.iter().enumerate().skip(12) {
            let mut gradient = 0.0;
            for (grad, discount) in &grads {
                gradient += grad.wrt(*weight) * discount;
            }

            // TD-Leaf update rule:
            weights[index] += ALPHA * gradient;
        }
    }
}
