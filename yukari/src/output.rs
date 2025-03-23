use std::time::Duration;

use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use tinyvec::ArrayVec;
use yukari_movegen::{Board, Move};

pub trait Output {
    fn new_pv(&mut self, board: &Board, depth: i32, seldepth: i32, score: i32, time: Duration, nodes: u64, pv: &[Move]);
    fn new_move(&mut self, board: &Board, depth: i32, seldepth: i32, time: Duration, nodes: u64, m: Move);
    #[allow(clippy::too_many_arguments)]
    fn complete(
        &mut self, board: &Board, depth: i32, seldepth: i32, score: i32, time: Duration, nodes: u64, pv: &[Move], success: bool, fail_high: bool,
    );
    fn abort(&mut self);
}

pub struct Human {
    progress: ProgressBar,
}

impl Human {
    pub fn start(board: &Board) -> Self {
        let mut moves = ArrayVec::new();
        board.generate(&mut moves);
        let progress = ProgressBar::new(moves.len() as u64);
        progress.set_style(ProgressStyle::with_template("[{bar:40.magenta/red}] {msg:30!}").unwrap().progress_chars("━╸ "));
        Self { progress }
    }
}

impl Output for Human {
    fn new_pv(&mut self, board: &Board, depth: i32, seldepth: i32, score: i32, time: Duration, nodes: u64, pv: &[Move]) {
        let nodes = if nodes > 1_000_000_000 { format!("{:>8}k", nodes / 1_000) } else { format!("{nodes:>9}") };

        let score = if score >= 9500 {
            let score = ((10000 - score) as u32).div_ceil(2);
            format!("+#{score}").green()
        } else if score <= -9500 {
            let score = ((10000 + score) as u32).div_ceil(2);
            format!("-#{score}").red()
        } else {
            let score = (score as f32) / 100.0;
            format!("{score:+7.2}").normal()
        };
        self.progress.println(format!("{depth:>2}/{:<2} {score:>9} {:>8.3} {nodes}\t{}", seldepth.to_string().dimmed(), time.as_secs_f32(), board.pv_to_san(pv)));
    }

    fn new_move(&mut self, board: &Board, _depth: i32, _seldepth: i32, _time: Duration, nodes: u64, m: Move) {
        self.progress.inc(1);
        self.progress.set_message(format!("{} ({} nodes)", board.to_san(m), nodes));
    }

    fn complete(
        &mut self, board: &Board, depth: i32, seldepth: i32, score: i32, time: Duration, nodes: u64, pv: &[Move], success: bool, fail_high: bool,
    ) {
        self.progress.finish_and_clear();
        let nodes = if nodes > 1_000_000_000 { format!("{:>8}k", nodes / 1_000) } else { format!("{nodes:>9}") };
        let score = if score >= 9500 {
            let score = ((10000 - score) as u32).div_ceil(2);
            format!("+#{score}").green()
        } else if score <= -9500 {
            let score = ((10000 + score) as u32).div_ceil(2);
            format!("-#{score}").red()
        } else {
            let score = (score as f32) / 100.0;
            format!("{score:+7.2}").normal()
        };
        if success {
            println!("{:>2}/{:<2} {score:>9} {:>8.3} {nodes}\t{}", depth.to_string().bold(), seldepth.to_string().dimmed(), time.as_secs_f32(), board.pv_to_san(pv));
        } else if fail_high {
            println!("{:>2}/{:<2} {score:>9} {:>8.3} {nodes}\t{}", depth.to_string().green(), seldepth.to_string().dimmed(), time.as_secs_f32(), board.pv_to_san(pv));
        } else {
            println!("{:>2}/{:<2} {score:>9} {:>8.3} {nodes}\t{}", depth.to_string().red(), seldepth.to_string().dimmed(), time.as_secs_f32(), board.pv_to_san(pv));
        }
    }

    fn abort(&mut self) {
        self.progress.finish_and_clear();
    }
}

pub struct Xboard {
    movecount: usize,
    movesleft: usize,
}

impl Xboard {
    pub fn start(board: &Board) -> Self {
        let mut moves = ArrayVec::new();
        board.generate(&mut moves);
        Self { movecount: moves.len(), movesleft: moves.len() }
    }
}

impl Output for Xboard {
    fn new_pv(&mut self, _board: &Board, depth: i32, _seldepth: i32, mut score: i32, time: Duration, nodes: u64, pv: &[Move]) {
        if score >= 9500 {
            score = 100000 + (10000 - score) / 2;
        }
        if score <= -9500 {
            score = -100000 - (-10000 - score) / 2;
        }
        print!("{depth} {score} {} {nodes}", time.as_millis() / 10);
        for m in pv {
            print!(" {m}");
        }
        println!();
    }

    fn new_move(&mut self, _board: &Board, depth: i32, _seldepth: i32, time: Duration, nodes: u64, m: Move) {
        println!("stat01: {} {} {} {} {} {}", time.as_millis() / 10, nodes, depth, self.movesleft, self.movecount, m);
        self.movesleft -= 1;
    }

    fn complete(
        &mut self, _board: &Board, depth: i32, _seldepth: i32, mut score: i32, time: Duration, nodes: u64, pv: &[Move], success: bool,
        fail_high: bool,
    ) {
        if score >= 9500 {
            score = 100000 + (10000 - score) / 2;
        }
        if score <= -9500 {
            score = -100000 + (-10000 - score) / 2;
        }
        print!("{depth} {score} {} {nodes}", time.as_millis() / 10);
        for m in pv {
            print!(" {m}");
        }
        if success {
            println!();
        } else if fail_high {
            println!("!");
        } else {
            println!("?");
        }
    }

    fn abort(&mut self) {
        /* no-op */
    }
}

pub struct Uci {
    moves: u32,
}

impl Uci {
    pub fn start(_board: &Board) -> Self {
        Self { moves: 1 }
    }
}

impl Output for Uci {
    fn new_pv(&mut self, _board: &Board, depth: i32, seldepth: i32, score: i32, time: Duration, nodes: u64, pv: &[Move]) {
        print!("info depth {depth} seldepth {seldepth} score ");
        if score >= 9500 {
            print!("mate {} ", 10000 - score);
        } else if score <= -9500 {
            print!("mate {} ", -10000 + score);
        } else {
            print!("cp {score} ");
        }
        print!("time {} nodes {nodes} pv", time.as_millis());
        for m in pv {
            print!(" {m}");
        }
        println!();
    }

    fn new_move(&mut self, _board: &Board, depth: i32, seldepth: i32, time: Duration, nodes: u64, m: Move) {
        println!("info depth {depth} seldepth {seldepth} time {} nodes {nodes} currmove {m} currmovenumber {}", time.as_millis(), self.moves);
        self.moves += 1;
    }

    fn complete(
        &mut self, _board: &Board, depth: i32, seldepth: i32, score: i32, time: Duration, nodes: u64, pv: &[Move], success: bool, fail_high: bool,
    ) {
        print!("info depth {depth} seldepth {seldepth} score ");
        if score >= 9500 {
            print!("mate {} ", 10000 - score);
        } else if score <= -9500 {
            print!("mate {} ", -10000 + score);
        } else {
            print!("cp {score} ");
        }
        if success {
        } else if fail_high {
            print!("lowerbound ");
        } else {
            print!("upperbound ");
        }
        print!("time {} nodes {nodes}", time.as_millis());
        if !pv.is_empty() {
            print!(" pv");
            for m in pv {
                print!(" {m}");
            }
        }
        println!();
    }

    fn abort(&mut self) {
        /* no-op */
    }
}

pub struct NoOp;

impl Output for NoOp {
    fn new_pv(&mut self, _board: &Board, _depth: i32, _seldepth: i32, _score: i32, _time: Duration, _nodes: u64, _pv: &[Move]) {
        /* no-op */
    }

    fn new_move(&mut self, _board: &Board, _depth: i32, _seldepth: i32, _time: Duration, _nodes: u64, _m: Move) {
        /* no-op */
    }

    fn complete(
        &mut self, _board: &Board, _depth: i32, _seldepth: i32, _score: i32, _time: Duration, _nodes: u64, _pv: &[Move], _success: bool,
        _fail_high: bool,
    ) {
        /* no-op */
    }

    fn abort(&mut self) {
        /* no-op */
    }
}
