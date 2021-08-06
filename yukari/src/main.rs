use std::io;
use std::str::FromStr;
use std::time::{Duration, Instant};
use yukari::{Search, is_repetition_draw};
use yukari::engine::{TimeControl, TimeMode};
use yukari_movegen::{Board, Move, Square, Zobrist};
use tinyvec::ArrayVec;

#[derive(Clone, Copy, Debug)]
enum Mode {
    /// In normal mode (which is more properly probably called thinking mode), we respond
    /// to incoming moves by updating our state and then we will reply with a chosen move
    Normal,
    /// In force mode we just update our internal state, not responding with a move.
    /// xboard itself seems to use this to relay past game moves to the engine
    Force
    // TODO: Update doc comment
    // TODO: Analyze mode also exists
}

/// The main engine state
#[derive(Clone)]
pub struct Yukari {
    board: Board,
    tc: TimeControl,
    mode: Mode,
    zobrist: Zobrist,
    keystack: Vec<u64>,
}

impl Yukari {
    /// Create a new copy of the engine, starting with the typical position and unused time controls
    pub fn new() -> Self {
        let zobrist = Zobrist::new();
        Self {
            // Using startpos fixes knights
            board: Board::startpos(&zobrist),
            // Time controls are uninitialized
            tc: TimeControl::new(TimeMode::St(0)),
            // Normal move making is on by default
            mode: Mode::Normal,
            zobrist,
            keystack: Vec::new(),
        }
    }

    /// Sets the game board from FEN notation
    /// # Panics
    /// Panics when invalid FEN is input.
    pub fn set_board(&mut self, s: &str) {
        self.board = Board::from_fen(s, &self.zobrist).unwrap();
        self.keystack.clear();
    }

    /// Parses the two xboard time control setup commands and sets that as our controls
    /// # Panics
    /// Panics when invalid time controls are passed in
    pub fn parse_tc(&mut self, s: &str) {
        let mode = TimeMode::from_str(s).unwrap();
        self.tc = TimeControl::new(mode);
    }

    /// Update with a new remaining time directly from the GUI
    /// Expects a value in centiseconds
    pub fn set_remaining(&mut self, csec: f32) {
        self.tc.set_remaining(csec);
    }

    /// Generates valid moves for current position then finds the attempted
    /// move in the list
    pub fn find_move(&self, from: Square, dest: Square) -> Option<Move> {
        let moves: [Move; 256] = [Move::default(); 256];
        let mut moves = ArrayVec::from(moves);
        moves.set_len(0);
        self.board.generate(&mut moves);
        for m in moves {
            if m.from == from && m.dest == dest {
                return Some(m);
            }
        }
        None
    }

    /// Real search, falls back to dumb search in extreme time constraints
    pub fn search(&mut self, best_pv: &mut ArrayVec<[Move; 32]>) {
        let start = Instant::now();
        let stop_after = start + Duration::from_secs_f32(self.tc.search_time());
        let mut s = Search::new(Some(stop_after), &self.zobrist);
        // clone another to use inside the loop
        // Use a seperate backing data to record the current move set
        let mut depth = 1;
        let mut pv: ArrayVec<[Move; 32]> = ArrayVec::new();
        while depth < 20 {
            pv.set_len(0);
            // FIXME: We want to search one depth without time controls
            let score = s.search_root(&self.board, depth, &mut pv, &self.keystack);
            // If we have bailed out stop the loop
            if Instant::now() >= stop_after {
                break;
            }
            // If we have a pv that's not just empty from bailing out use that as our best moves
            best_pv.clone_from(&pv);
            let now = Instant::now().duration_since(start);
            print!(
                "{} {:.2} {} {} ",
                depth,
                score,
                now.as_millis() / 10,
                s.nodes() + s.qnodes()
            );
            eprint!(
                "{} {:.2} {} {} ",
                depth,
                score,
                now.as_millis() / 10,
                s.nodes() + s.qnodes()
            );
            for m in pv.iter() {
                print!("{} ", m);
                eprint!("{} ", m);
            }
            println!();
            eprintln!();
            depth += 1;
        }
        println!(
            "# QS: {:.3}%",
            (100 * s.qnodes()) as f64 / (s.nodes() as f64 + s.qnodes() as f64)
        );
        println!(
            "# Branching factor: {:.3}",
            ((s.nodes() + s.qnodes()) as f64).powf(1.0 / depth as f64)
        );
        self.tc.increment_moves();
    }
}

impl Default for Yukari {
    fn default() -> Self {
        Self::new()
    }
}

fn main() -> io::Result<()> {
    let mut engine = Yukari::new();
    let mut line = String::new();
    loop {
        line.clear();
        let count = io::stdin().read_line(&mut line)?;
        if count == 0 {
            println!("# got zero read");
            continue;
        }
        let trimmed = line.trim();
        let (cmd, args) = trimmed.split_once(' ').unwrap_or((trimmed, ""));

        match cmd {
            // Identification for engines that auto switch between protocols
            "xboard" => {}
            // This is where we send our features
            "protover" => {
                // v1 won't send this anyway and we need v2
                assert_eq!(args, "2");
                // Do features individually
                println!("feature myname=\"Yukari 20072021\"");
                // No signals support
                println!("feature sigint=0 sigterm=0");
                // Don't currently understand enough to reuse the engine for next game
                println!("feature reuse=0");
                // Ping feature helps with race conditions
                println!("feature ping=1");
                // We would rather get FEN updates of the board than white/black
                println!("feature colors=0 setboard=1");
                // Technically needed to support those # <msg> lines
                println!("feature debug=1");
                // Communicate that feature reporting is done
                println!("feature done=1");
            }
            // Directly update the engine's board from a FEN
            "setboard" => engine.set_board(args),
            // Reset the entire state of the engine
            "new" => engine = Yukari::new(),
            // Parse our two time controls from the whole commmand lines
            // TODO: This is rather xboard specific
            "level" | "st" => engine.parse_tc(trimmed),
            // Hard would turn on thinking during opponent's time, easy would turn it off
            // we don't do it, so it's unimportant
            "hard" | "easy" => {}
            "quit" => { break; },
            // Feature replies are just ignored since we don't turn anything off yet
            // TODO: Handle rejects we can't tolerate and abort early
            "accepted" | "rejected" => {},
            // Ping expects a response with the correct tag once the commands prior to the ping are done
            // That ends up being some GPU fence level synchronization nonsense if it were to send more than one
            // so for now we just "handle it" by replying with pong immediately. For now this "works" because
            // the engine is single threaded such that moves can never be passed by other commands
            "ping" => println!("pong {}", args),
            // TODO: Should support randomization so we don't always play the same game
            // we can't todo!() because we cannot turn off getting this message
            "random" => {}
            // We don't implement games against computer players games differently
            "computer" => {}
            // This report gives us info about what time we have left right now directly
            // the value is in centiseconds
            "time" => engine.set_remaining(f32::from_str(args).unwrap()),
            // TODO: Should we care? Right now we don't have any logic to handle opponent time seperate
            "otim" => {}
            "go" => {
                engine.mode = Mode::Normal;
                // When we get go we should make a move immediately
                let pv: [Move; 32] = [Move::default(); 32];
                let mut pv = ArrayVec::from(pv);
                pv.set_len(0);
                engine.search(&mut pv);
                // Choose the top move
                let m = pv[0];
                // We must actually make the move locally too
                engine.board = engine.board.make(m, &engine.zobrist);
                println!("move {}", m);
                if is_repetition_draw(&engine.keystack, engine.board.hash()) {
                    println!("1/2-1/2 {{Draw by repetition}}");
                }
                engine.keystack.push(engine.board.hash());
            }
            "force" => engine.mode = Mode::Force,
            _ => {
                // Always ascii
                let chars = trimmed.as_bytes();
                if chars[1].is_ascii_digit() && chars[3].is_ascii_digit() {
                    // This is actually a move
                    let from = Square::from_str(&cmd[..2]).unwrap();
                    let dest = Square::from_str(&cmd[2..4]).unwrap();
                    match engine.mode {
                        Mode::Normal => {
                            // Find the move in the list
                            let m = engine.find_move(from, dest).expect("Attempted move not found!?");
                            engine.board = engine.board.make(m, &engine.zobrist);
                            if is_repetition_draw(&engine.keystack, engine.board.hash()) {
                                println!("1/2-1/2 {{Draw by repetition}}");
                            }
                            engine.keystack.push(engine.board.hash());
                            // Find the next move to make
                            // TODO: Cleanups
                            let pv: [Move; 32] = [Move::default(); 32];
                            let mut pv = ArrayVec::from(pv);
                            pv.set_len(0);
                            engine.search(&mut pv);
                            // Choose the top move
                            let m = pv[0];
                            // We must actually make the move locally too
                            engine.board = engine.board.make(m, &engine.zobrist);
                            println!("move {}", m);
                            if is_repetition_draw(&engine.keystack, engine.board.hash()) {
                                println!("1/2-1/2 {{Draw by repetition}}");
                            }
                            engine.keystack.push(engine.board.hash());
                        }
                        Mode::Force => {
                            let m = engine.find_move(from, dest).expect("Attempted move not found!?");
                            engine.board = engine.board.make(m, &engine.zobrist);
                            if is_repetition_draw(&engine.keystack, engine.board.hash()) {
                                println!("1/2-1/2 {{Draw by repetition}}");
                            }
                            engine.keystack.push(engine.board.hash());
                        }
                    }
                } else {
                    // This may look like I chose the format, but it is a standard response
                    println!("Error (unknown command): {}", trimmed);
                }
            }
        }
    }
    Ok(())
}
