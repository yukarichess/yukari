use std::{fs::File, io::Read};

use yukari::Tune;
use yukari_movegen::{Board, Zobrist};

fn main() {
    println!("Loading FENs...");

    let zobrist = Zobrist::new();
    let boards = {
        let mut boards = Vec::new();
        let mut s = String::new();
        let mut f = File::open("quiescent_positions_with_results").unwrap();
        f.read_to_string(&mut s).unwrap();

        for line in s.lines() {
            let mut fields = line.split_ascii_whitespace();
            let mut line = String::new();
            for field in fields.clone().take(4) {
                line += field;
                line += " ";
            }
            line += "0 1";
            let result_str = fields.nth(4).unwrap();
            let mut result = 0.0;
            if result_str == "1-0" {
                result = 1.0;
            } else if result_str == "0-1" {
                result = -1.0;
            }
            boards.push((Board::from_fen(&line, &zobrist).unwrap(), result));
        }
        boards
    };

    println!("Found {} positions", boards.len());

    let tune = Tune::new(boards);
    tune.tune().unwrap();
}
