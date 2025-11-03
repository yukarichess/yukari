use std::time::Duration;

use colored::Colorize;
use yukari_movegen::{Board, Move};

const NORMALISE_A: [f64; 4] = [102.66983070_f64, -222.01728527, 41.66355835, 311.16910753];
const NORMALISE_B: [f64; 4] = [9.81632640_f64, -23.57246193, 67.77015515, 42.50511876];

/// Normalise score to a centipawn scale where 100 = 50% chance to win.
fn normalise_centipawn(board: &Board, score: i32) -> i32 {
    if score.abs() > 9500 || score == 0 {
        return score;
    }

    let piecemask = board.data().piecemask();
    let material = piecemask.pawns().count_ones()
        + 3 * piecemask.knights().count_ones()
        + 3 * piecemask.bishops().count_ones()
        + 5 * piecemask.rooks().count_ones()
        + 9 * piecemask.queens().count_ones();
    let material = f64::from(material.clamp(17, 78)) / 58.0;

    let a = NORMALISE_A[0].mul_add(material, NORMALISE_A[1]).mul_add(material, NORMALISE_A[2]).mul_add(material, NORMALISE_A[3]);

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
    let material = piecemask.pawns().count_ones()
        + 3 * piecemask.knights().count_ones()
        + 3 * piecemask.bishops().count_ones()
        + 5 * piecemask.rooks().count_ones()
        + 9 * piecemask.queens().count_ones();
    let material = f64::from(material.clamp(17, 78)) / 58.0;

    let a = NORMALISE_A[0].mul_add(material, NORMALISE_A[1]).mul_add(material, NORMALISE_A[2]).mul_add(material, NORMALISE_A[3]);
    let b = NORMALISE_B[0].mul_add(material, NORMALISE_B[1]).mul_add(material, NORMALISE_B[2]).mul_add(material, NORMALISE_B[3]);

    (0.5 + 1000.0 / (1.0 + ((a - f64::from(score)) / b).exp())) as i32
}

pub trait Output {
    #[allow(clippy::too_many_arguments)]
    fn complete(
        &mut self, board: &Board, depth: i32, seldepth: usize, score: i32, time: Duration, nodes: u64, pv: &[Move], success: bool,
        fail_high: bool,
    );
}

pub struct Human;

impl Output for Human {
    fn complete(
        &mut self, board: &Board, depth: i32, seldepth: usize, score: i32, time: Duration, nodes: u64, pv: &[Move], success: bool,
        fail_high: bool,
    ) {
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
}

pub struct Xboard;

impl Output for Xboard {
    fn complete(
        &mut self, board: &Board, depth: i32, _seldepth: usize, score: i32, time: Duration, nodes: u64, pv: &[Move], success: bool,
        fail_high: bool,
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
}

pub struct Uci;

impl Output for Uci {
    fn complete(
        &mut self, board: &Board, depth: i32, seldepth: usize, score: i32, time: Duration, nodes: u64, pv: &[Move], success: bool,
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
}
