use std::time::Duration;

use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use tinyvec::ArrayVec;
use yukari_movegen::{Board, Move};

/// Normalise score to a centipawn scale where 100 = 50% chance to win.
fn normalise_centipawn(board: &Board, score: i32) -> i32 {
    if score.abs() > 9500 || score == 0 { 
        return score;
    }

    let piecemask = board.data().piecemask();
    let material = piecemask.pawns().count_ones() + 3 * piecemask.knights().count_ones() + 3 * piecemask.bishops().count_ones() + 5 * piecemask.rooks().count_ones() + 9 * piecemask.queens().count_ones();
    let material = f64::from(material.clamp(17, 78)) / 58.0;

    let a = [102.66983070_f64, -222.01728527, 41.66355835, 311.16910753];
    let b = [9.81632640_f64, -23.57246193, 67.77015515, 42.50511876];

    let a = a[0].mul_add(material, a[1]).mul_add(material, a[2]).mul_add(material, a[3]);

    ((100.0 * f64::from(score)) / a).round() as i32
}

/// Normalise score to a likelihood of winning, where 1000 = 100% chance to win.
fn normalise_winrate(board: &Board, score: i32) -> i32 {
    if score > 9500 { 
        return 1000;
    }
    if score == 0 || score < -9500 {
        return 0;
    }

    let piecemask = board.data().piecemask();
    let material = piecemask.pawns().count_ones() + 3 * piecemask.knights().count_ones() + 3 * piecemask.bishops().count_ones() + 5 * piecemask.rooks().count_ones() + 9 * piecemask.queens().count_ones();
    let material = f64::from(material.clamp(17, 78)) / 58.0;

    let a = [102.66983070_f64, -222.01728527, 41.66355835, 311.16910753];
    let b = [9.81632640_f64, -23.57246193, 67.77015515, 42.50511876];

    let a = a[0].mul_add(material, a[1]).mul_add(material, a[2]).mul_add(material, a[3]);
    let b = b[0].mul_add(material, b[1]).mul_add(material, b[2]).mul_add(material, b[3]);

    (0.5 + 1000.0 / (1.0 + ((a - f64::from(score)) / b).exp())) as i32
}

pub trait Output {
    #[allow(clippy::too_many_arguments)]
    fn new_pv(&mut self, board: &Board, depth: i32, seldepth: i32, score: i32, time: Duration, nodes: u64, pv: &[Move]);
    fn new_move(&mut self, board: &Board, depth: i32, seldepth: i32, time: Duration, nodes: u64, m: Move);
    #[allow(clippy::too_many_arguments)]
    fn complete(
        &mut self, board: &Board, depth: i32, seldepth: i32, score: i32, time: Duration, nodes: u64, pv: &[Move], success: bool,
        fail_high: bool,
    );
    fn abort(&mut self);
}

pub struct Human {
    progress: ProgressBar,
}

impl Human {
    #[must_use]
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
        // Normalise score for display.
        let win = normalise_winrate(board, score);
        let loss = normalise_winrate(board, -score);
        let draw = 1000 - win - loss;

        let score = normalise_centipawn(board, score);

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

        let highlight_win = win > draw && win > loss;
        let highlight_draw = draw > win && draw > loss;
        let highlight_loss = loss > win && loss > draw;

        let win = format!("{:>5.1}", f64::from(win) / 10.0);
        let draw = format!("{:>5.1}", f64::from(draw) / 10.0);
        let loss = format!("{:>5.1}", f64::from(loss) / 10.0);
        
        let win = if highlight_win { win.green().to_string() } else { win };
        let draw = if highlight_draw { draw.bold().to_string() } else { draw };
        let loss = if highlight_loss { loss.red().to_string() } else { loss };

        self.progress.println(format!(
            "{depth:>2}/{:<2} {score:>9} ({win}% W, {draw}% D, {loss}% L) {:>8.3} {nodes}\t{}",
            seldepth.to_string().dimmed(),
            time.as_secs_f32(),
            board.pv_to_san(pv)
        ));
    }

    fn new_move(&mut self, board: &Board, _depth: i32, _seldepth: i32, _time: Duration, nodes: u64, m: Move) {
        self.progress.inc(1);
        self.progress.set_message(format!("{} ({} nodes)", board.to_san(m), nodes));
    }

    fn complete(
        &mut self, board: &Board, depth: i32, seldepth: i32, score: i32, time: Duration, nodes: u64, pv: &[Move], success: bool,
        fail_high: bool,
    ) {
        // Normalise score for display.
        let win = normalise_winrate(board, score);
        let loss = normalise_winrate(board, -score);
        let draw = 1000 - win - loss;

        let score = normalise_centipawn(board, score);

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

        let highlight_win = win > draw && win > loss;
        let highlight_draw = draw > win && draw > loss;
        let highlight_loss = loss > win && loss > draw;

        let win = format!("{:>5.1}", f64::from(win) / 10.0);
        let draw = format!("{:>5.1}", f64::from(draw) / 10.0);
        let loss = format!("{:>5.1}", f64::from(loss) / 10.0);
        
        let win = if highlight_win { win.green().to_string() } else { win.dimmed().to_string() };
        let draw = if highlight_draw { draw.bold().to_string() } else { draw.dimmed().to_string() };
        let loss = if highlight_loss { loss.red().to_string() } else { loss.dimmed().to_string() };

        if success {
            println!(
                "{:>2}/{:<2} {score:>9} ({win}% W, {draw}% D, {loss}% L) {:>8.3} {nodes}\t{}",
                depth.to_string().bold(),
                seldepth.to_string().dimmed(),
                time.as_secs_f32(),
                board.pv_to_san(pv)
            );
        } else if fail_high {
            println!(
                "{:>2}/{:<2} {score:>9} ({win}% W, {draw}% D, {loss}% L) {:>8.3} {nodes}\t{}",
                depth.to_string().green(),
                seldepth.to_string().dimmed(),
                time.as_secs_f32(),
                board.pv_to_san(pv)
            );
        } else {
            println!(
                "{:>2}/{:<2} {score:>9} ({win}% W, {draw}% D, {loss}% L) {:>8.3} {nodes}\t{}",
                depth.to_string().red(),
                seldepth.to_string().dimmed(),
                time.as_secs_f32(),
                board.pv_to_san(pv)
            );
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
    #[must_use]
    pub fn start(board: &Board) -> Self {
        let mut moves = ArrayVec::new();
        board.generate(&mut moves);
        Self { movecount: moves.len(), movesleft: moves.len() }
    }
}

impl Output for Xboard {
    fn new_pv(&mut self, board: &Board, depth: i32, _seldepth: i32, score: i32, time: Duration, nodes: u64, pv: &[Move]) {
        // Normalise score for display.
        let mut score = normalise_centipawn(board, score);

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
        &mut self, board: &Board, depth: i32, _seldepth: i32, score: i32, time: Duration, nodes: u64, pv: &[Move],
        success: bool, fail_high: bool,
    ) {
        // Normalise score for display.
        let mut score = normalise_centipawn(board, score);

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
    #[must_use]
    pub fn start(_board: &Board) -> Self {
        Self { moves: 1 }
    }
}

impl Output for Uci {
    fn new_pv(&mut self, board: &Board, depth: i32, seldepth: i32, score: i32, time: Duration, nodes: u64, pv: &[Move]) {
        // Normalise score for display.
        let score = normalise_centipawn(board, score);

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
        println!(
            "info depth {depth} seldepth {seldepth} time {} nodes {nodes} currmove {m} currmovenumber {}",
            time.as_millis(),
            self.moves
        );
        self.moves += 1;
    }

    fn complete(
        &mut self, board: &Board, depth: i32, seldepth: i32, score: i32, time: Duration, nodes: u64, pv: &[Move], success: bool,
        fail_high: bool,
    ) {
        // Normalise score for display.
        let score = normalise_centipawn(board, score);

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
        &mut self, _board: &Board, _depth: i32, _seldepth: i32, _score: i32, _time: Duration, _nodes: u64, _pv: &[Move],
        _success: bool, _fail_high: bool,
    ) {
        /* no-op */
    }

    fn abort(&mut self) {
        /* no-op */
    }
}
