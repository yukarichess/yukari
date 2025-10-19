use std::{
    io::{self, BufWriter},
    str::FromStr,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use colored::Colorize;
use indicatif::{ParallelProgressIterator, ProgressStyle};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use tinyvec::ArrayVec;
use yukari::{
    self, Search, SearchParams, TtEntry, allocate_tt, datagen,
    engine::{TimeControl, TimeMode},
    is_repetition_draw,
    output::{self, Output},
};
use yukari_movegen::{Board, Colour, Move, Piece, Square};

#[derive(Clone, Copy, Debug)]
enum Mode {
    /// In normal mode (which is more properly probably called thinking mode), we respond
    /// to incoming moves by updating our state and then we will reply with a chosen move
    Normal,
    /// In force mode we just update our internal state, not responding with a move.
    /// xboard itself seems to use this to relay past game moves to the engine
    Force, // TODO: Update doc comment
           // TODO: Analyze mode also exists
}

#[derive(Clone, Copy)]
pub enum Protocol {
    Human,
    Xboard,
    Uci,
}

/// The main engine state
#[derive(Clone)]
pub struct Yukari {
    board: Board,
    tc: TimeControl,
    max_depth: Option<i32>,
    nodes_per_second: Option<u32>,
    mode: Mode,
    keystack: Vec<u64>,
    history: [[[i16; 64]; 64]; 12],
    corrhist_p: [[i32; 16384]; 2],
    corrhist_kbn: [[i32; 16384]; 2],
    corrhist_kqr: [[i32; 16384]; 2],
    conthist: [[i16; 2 * 6 * 64]; 2 * 6 * 64],
    params: SearchParams,
}

impl Yukari {
    /// Create a new copy of the engine, starting with the typical position and unused time controls
    #[must_use]
    pub fn new() -> Self {
        Self {
            // Using startpos fixes knights
            board: Board::startpos(),
            // Time controls are uninitialized
            tc: TimeControl::new(TimeMode::MoveTime(5000)),
            max_depth: None,
            nodes_per_second: None,
            // Normal move making is on by default
            mode: Mode::Normal,
            keystack: Vec::new(),
            history: [[[0; 64]; 64]; 12],
            corrhist_p: [[0; 16384]; 2],
            corrhist_kbn: [[0; 16384]; 2],
            corrhist_kqr: [[0; 16384]; 2],
            conthist: [[0; 2 * 6 * 64]; 2 * 6 * 64],
            params: SearchParams::default(),
        }
    }

    /// Sets the game board from FEN notation
    /// # Panics
    /// Panics when invalid FEN is input.
    pub fn set_board(&mut self, s: &str) {
        self.board = Board::from_fen(s).unwrap();
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
        self.tc.set_remaining(10.0 * csec);
    }

    pub fn set_depth(&mut self, depth: i32) {
        self.max_depth = Some(depth);
    }

    pub fn set_nodes_per_second(&mut self, nodes_per_second: i32) {
        assert!(nodes_per_second > 0, "CPU time mode not supported");
        self.nodes_per_second = Some(nodes_per_second as u32);
    }

    /// Generates valid moves for current posiition then finds the attempted
    /// move in the list
    #[must_use]
    pub fn find_move(&self, from: Square, dest: Square, prom: Option<Piece>) -> Option<Move> {
        let moves: [Move; 256] = [Move::default(); 256];
        let mut moves = ArrayVec::from(moves);
        moves.set_len(0);
        self.board.generate(&mut moves);
        moves.into_iter().find(|&m| m.from == from && m.dest == dest && m.prom == prom)
    }

    /// Real search, falls back to dumb search in extreme time constraints
    pub fn search(&mut self, best_pv: &mut ArrayVec<[Move; 64]>, tt: &mut [TtEntry], protocol: Protocol) {
        let start = Instant::now();
        let (soft_limit, hard_limit) = self.tc.search_time();
        let mut soft_limit = start + Duration::from_secs_f32(soft_limit);
        let hard_limit = start + Duration::from_secs_f32(hard_limit);

        // while I love xboard protocol for its ease of parsing, the way fixed-nodes searching is implemented is *bad*.
        let (nodes, stop_after) = if let Some(nodes_per_second) = self.nodes_per_second {
            let TimeMode::MoveTime(movetime) = self.tc.mode else {
                panic!("nps is only supported in st mode");
            };
            (Some(movetime * nodes_per_second / 1000), None)
        } else {
            (None, Some(hard_limit))
        };

        let mut s = Search::new(stop_after, tt, &mut self.history, &mut self.corrhist_p, &mut self.corrhist_kbn, &mut self.corrhist_kqr, &mut self.conthist, &self.params);
        // clone another to use inside the loop
        // Use a seperate backing data to record the current move set
        let mut depth = 1;
        let mut score = 0;
        let mut pv = ArrayVec::new();
        let max_depth = self.max_depth.unwrap_or(63);
        while depth <= max_depth {
            let mut lower_bound = 50;
            let mut upper_bound = 50;
            loop {
                pv.set_len(0);
                let lower_window = score - lower_bound;
                let upper_window = score + upper_bound;
                let output: &mut dyn output::Output = match protocol {
                    Protocol::Human => &mut output::Human,
                    Protocol::Xboard => &mut output::Xboard,
                    Protocol::Uci => &mut output::Uci,
                };
                score = s.search_root(&self.board, depth, lower_window, upper_window, &mut pv, &mut self.keystack);

                // If we have bailed out stop the loop
                if stop_after.is_some() && Instant::now() >= hard_limit {
                    break;
                }

                if score <= lower_window {
                    lower_bound *= 2;
                    output.complete(
                        &self.board,
                        depth,
                        s.seldepth(),
                        score,
                        Instant::now().duration_since(start),
                        s.nodes() + s.qnodes(),
                        &pv,
                        false,
                        false,
                    );
                    continue;
                }
                if score >= upper_window {
                    upper_bound *= 2;
                    output.complete(
                        &self.board,
                        depth,
                        s.seldepth(),
                        score,
                        Instant::now().duration_since(start),
                        s.nodes() + s.qnodes(),
                        &pv,
                        false,
                        true,
                    );
                    continue;
                }
                output.complete(
                    &self.board,
                    depth,
                    s.seldepth(),
                    score,
                    Instant::now().duration_since(start),
                    s.nodes() + s.qnodes(),
                    &pv,
                    true,
                    false,
                );
                break;
            }
            // If we have bailed out stop the loop
            if stop_after.is_some() && Instant::now() >= hard_limit {
                break;
            }
            // Modify time to search based on best move stability.
            if matches!(self.tc.mode, TimeMode::Incremental { base: _, increment: _ }) && !pv.is_empty() && !best_pv.is_empty() {
                if pv[0] == best_pv[0] {
                    let soft_limit_diff = soft_limit - start;
                    soft_limit = start + soft_limit_diff.mul_f64(0.95);
                } else {
                    let soft_limit_diff = soft_limit - start;
                    soft_limit = start + soft_limit_diff.mul_f64(1.1);
                }
            }

            // If we have a pv that's not just empty from bailing out use that as our best moves
            best_pv.clone_from(&pv);

            if stop_after.is_some() && Instant::now() >= soft_limit {
                break;
            }
            if let Some(nodes) = nodes
                && s.nodes() + s.qnodes() >= u64::from(nodes)
            {
                break;
            }
            depth += 1;
        }
        println!("# Avg AB cutoff index: {:.3}", s.beta_cutoff_index());
        println!("# Avg QS cutoff index: {:.3}", s.q_beta_cutoff_index());
        println!("# NMP success: {:.3}%", s.nullmove_success());
        println!("# QS nodes: {} {:.3}%", s.qnodes(), (100 * s.qnodes()) as f64 / (s.nodes() as f64 + s.qnodes() as f64));
        println!("# ZW AB nodes: {:.3}%", s.zw_nodes());
        println!("# ZW QS nodes: {:.3}%", s.zw_qnodes());
        println!("# TT hit rate: {:.3}%", s.tt_hit_rate());
        println!("# TT cutoff rate: {:.3}%", s.tt_cutoff_rate());
        println!("# Branching factor: {:.3}", ((s.nodes() + s.qnodes()) as f64).powf(1.0 / f64::from(depth)));
    }

    fn bench(&mut self, tt: &mut [TtEntry]) {
        let fens = [
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 10",
            "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 11",
            "4rrk1/pp1n3p/3q2pQ/2p1pb2/2PP4/2P3N1/P2B2PP/4RRK1 b - - 7 19",
            "rq3rk1/ppp2ppp/1bnpN3/3N2B1/4P3/7P/PPPQ1PP1/2KR3R b - - 0 14",
            "r1bq1r1k/1pp1n1pp/1p1p4/4p2Q/4PpP1/1BNP4/PPP2P1P/3R1RK1 b - g3 0 14",
            "r3r1k1/2p2ppp/p1p1bn2/8/1q2P3/2NPQN2/PPP3PP/R4RK1 b - - 2 15",
            "r1bbk1nr/pp3p1p/2n5/1N4p1/2Np1B2/8/PPP2PPP/2KR1B1R w kq - 0 13",
            "r1bq1rk1/ppp1nppp/4n3/3p3Q/3P4/1BP1B3/PP1N2PP/R4RK1 w - - 1 16",
            "4r1k1/r1q2ppp/ppp2n2/4P3/5Rb1/1N1BQ3/PPP3PP/R5K1 w - - 1 17",
            "2rqkb1r/ppp2p2/2npb1p1/1N1Nn2p/2P1PP2/8/PP2B1PP/R1BQK2R b KQ - 0 11",
            "r1bq1r1k/b1p1npp1/p2p3p/1p6/3PP3/1B2NN2/PP3PPP/R2Q1RK1 w - - 1 16",
            "3r1rk1/p5pp/bpp1pp2/8/q1PP1P2/b3P3/P2NQRPP/1R2B1K1 b - - 6 22",
            "r1q2rk1/2p1bppp/2Pp4/p6b/Q1PNp3/4B3/PP1R1PPP/2K4R w - - 2 18",
            "4k2r/1pb2ppp/1p2p3/1R1p4/3P4/2r1PN2/P4PPP/1R4K1 b - - 3 22",
            "3q2k1/pb3p1p/4pbp1/2r5/PpN2N2/1P2P2P/5PP1/Q2R2K1 b - - 4 26",
            "6k1/6p1/6Pp/ppp5/3pn2P/1P3K2/1PP2P2/3N4 b - - 0 1",
            "3b4/5kp1/1p1p1p1p/pP1PpP1P/P1P1P3/3KN3/8/8 w - - 0 1",
            "2K5/p7/7P/5pR1/8/5k2/r7/8 w - - 0 1",
            "8/6pk/1p6/8/PP3p1p/5P2/4KP1q/3Q4 w - - 0 1",
            "7k/3p2pp/4q3/8/4Q3/5Kp1/P6b/8 w - - 0 1",
            "8/2p5/8/2kPKp1p/2p4P/2P5/3P4/8 w - - 0 1",
            "8/1p3pp1/7p/5P1P/2k3P1/8/2K2P2/8 w - - 0 1",
            "8/pp2r1k1/2p1p3/3pP2p/1P1P1P1P/P5KR/8/8 w - - 0 1",
            "8/3p4/p1bk3p/Pp6/1Kp1PpPp/2P2P1P/2P5/5B2 b - - 0 1",
            "5k2/7R/4P2p/5K2/p1r2P1p/8/8/8 b - - 0 1",
            "6k1/6p1/P6p/r1N5/5p2/7P/1b3PP1/4R1K1 w - - 0 1",
            "1r3k2/4q3/2Pp3b/3Bp3/2Q2p2/1p1P2P1/1P2KP2/3N4 w - - 0 1",
            "6k1/4pp1p/3p2p1/P1pPb3/R7/1r2P1PP/3B1P2/6K1 w - - 0 1",
            "8/3p3B/5p2/5P2/p7/PP5b/k7/6K1 w - - 0 1",
            "5rk1/q6p/2p3bR/1pPp1rP1/1P1Pp3/P3B1Q1/1K3P2/R7 w - - 93 90",
            "4rrk1/1p1nq3/p7/2p1P1pp/3P2bp/3Q1Bn1/PPPB4/1K2R1NR w - - 40 21",
            "r3k2r/3nnpbp/q2pp1p1/p7/Pp1PPPP1/4BNN1/1P5P/R2Q1RK1 w kq - 0 16",
            "3Qb1k1/1r2ppb1/pN1n2q1/Pp1Pp1Pr/4P2p/4BP2/4B1R1/1R5K b - - 11 40",
            "4k3/3q1r2/1N2r1b1/3ppN2/2nPP3/1B1R2n1/2R1Q3/3K4 w - - 5 1",
            // 5-man positions
            "8/8/8/8/5kp1/P7/8/1K1N4 w - - 0 1",  // Kc2 - mate
            "8/8/8/5N2/8/p7/8/2NK3k w - - 0 1",   // Na2 - mate
            "8/3k4/8/8/8/4B3/4KB2/2B5 w - - 0 1", // draw
            // 6-man positions
            "8/8/1P6/5pr1/8/4R3/7k/2K5 w - - 0 1",  // Re5 - mate
            "8/2p4P/8/kr6/6R1/8/8/1K6 w - - 0 1",   // Ka2 - mate
            "8/8/3P3k/8/1p6/8/1P6/1K3n2 b - - 0 1", // Nd2 - draw
            // 7-man positions
            "8/R7/2q5/8/6k1/8/1P5p/K6R w - - 0 124", // Draw
            // Mate and stalemate positions
            "6k1/3b3r/1p1p4/p1n2p2/1PPNpP1q/P3Q1p1/1R1RB1P1/5K2 b - - 0 1",
            "r2r1n2/pp2bk2/2p1p2p/3q4/3PN1QP/2P3R1/P4PP1/5RK1 w - - 0 1",
            "8/8/8/8/8/6k1/6p1/6K1 w - -",
            "7k/7P/6K1/8/3B4/8/8/8 b - -",
        ];

        let mut nodes = 0;
        let start = Instant::now();
        for fen in fens {
            let board = Board::from_fen(fen).unwrap();
            let start = Instant::now();
            for piece in 0..12 {
                for from in 0..64 {
                    for dest in 0..64 {
                        self.history[piece][from][dest] = 0;
                    }
                }
            }
            let mut s = Search::new(None, tt, &mut self.history, &mut self.corrhist_p, &mut self.corrhist_kbn, &mut self.corrhist_kqr, &mut self.conthist, &self.params);
            let mut keystack = Vec::new();
            let mut pv = ArrayVec::new();
            let mut score = 0;
            let mut lower_bound = 50;
            let mut upper_bound = 50;
            loop {
                pv.set_len(0);
                let lower_window = score - lower_bound;
                let upper_window = score + upper_bound;
                let mut output = output::Xboard;
                score = s.search_root(&board, 11, lower_window, upper_window, &mut pv, &mut keystack);

                if score <= lower_window {
                    lower_bound *= 2;
                    output.complete(
                        &board,
                        11,
                        s.seldepth(),
                        score,
                        Instant::now().duration_since(start),
                        s.nodes() + s.qnodes(),
                        &pv,
                        false,
                        false,
                    );
                    continue;
                }
                if score >= upper_window {
                    upper_bound *= 2;
                    output.complete(
                        &board,
                        11,
                        s.seldepth(),
                        score,
                        Instant::now().duration_since(start),
                        s.nodes() + s.qnodes(),
                        &pv,
                        false,
                        true,
                    );
                    continue;
                }
                output.complete(
                    &board,
                    11,
                    s.seldepth(),
                    score,
                    Instant::now().duration_since(start),
                    s.nodes() + s.qnodes(),
                    &pv,
                    true,
                    false,
                );
                break;
            }
            nodes += s.nodes() + s.qnodes();
        }
        let now = Instant::now().duration_since(start);
        let nps = (nodes as f64 / now.as_secs_f64()) as u64;
        println!("{nodes} nodes {nps} nps");
    }
}

impl Default for Yukari {
    fn default() -> Self {
        Self::new()
    }
}

const YUKARI: &str = "
           @@            @=            @@     @@
   .@     @@@@*         %@     @@      @      @@
   @@  @@  *@  @@   @@@@@@@@@@  @@     @      @@
   @@ @@   :@   @@      @    @-  @@    @ @    @@
   @@@@    :@   @@     @@    @-  @@    @@@    @@
   @@@     @@   @@    @@     @:        @@     @@
   @@   @@ @@ =@@     @*     @               :@
          @@@@+      @@     @@              @@=
       @@@=         @@   @@@@           #@@@

              %@@@.        @@@@
              %@@@.        @@@@       .@@@%
     :@@@:    %@@@.        @@@@    *@@@@@@@#
     :@@@:    %@@@@@@@@@@@ @@@@ @@@@@@@@=
     :@@@:    %@@@@@@@@@@@ @@@@@@@@@.      @@@:
     :@@@:    %@@@.        @@@@@           @@@%
     :@@@:    %@@@.        @@@@           #@@@
     :@@@:    @@@@@@@@@@@+ @@@@           @@@@
  @@@@@@@@@@@@@@@@@@@@     -@@@@@@@@@@@@@@@@@
  :@@@@@@+==:       @@@      +@@@@@@@@@@@@@:
                +@@@@@%*          @:
             @@@@@-          #@@@@@@@
        @@@@@@@@@@@@@@@@@@@@@@@@@@@@:
         @@@@@@@@@@@@@@@@@@@@@@   .
                  .@@@@@@=       @@@@:
              @@@@@%               %@@@+
        %@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@
       @@@@@@@@@@@@@@@@@@@@@@@+        @@@@@
        -=    @        @@@@     :@       @@=
           @@@@@@      @@@@    *@@@@@@
       %@@@@@@@:       @@@@     :@@@@@@@@@
   %@@@@@@@@    @.     @@@@         =@@@@@@@@
    @@@@-       @@@@@@@@@@*             @@@@-
                  @@@@@@
";

fn main() -> io::Result<()> {
    let mut engine = Yukari::new();
    let mut tt = allocate_tt(16);
    let mut protocol = Protocol::Human;

    for arg in std::env::args() {
        if arg == "bench" {
            engine.bench(&mut tt);
            return Ok(());
        }
        if arg == "datagen" {
            const GAMES: usize = 500_000;

            // Try to avoid stack overflows.
            rayon::ThreadPoolBuilder::new().stack_size(32 * 1024 * 1024).build_global().unwrap();

            let f = std::fs::File::options().create(true).append(true).open("games.viriformat").unwrap();
            let f = BufWriter::new(f);
            let f = Mutex::new(f);

            let positions = AtomicUsize::new(0);
            let style = ProgressStyle::with_template("[{bar:40.magenta/red}] {pos:>6}/{len:6} ({per_sec} games/s)")
                .unwrap()
                .progress_chars("━╸ ");

            (0..GAMES).into_par_iter().progress_with_style(style).for_each_init(
                || datagen::DataGen::new(&f),
                |dg, _| {
                    positions.fetch_add(dg.play(1), Ordering::SeqCst);
                },
            );

            println!("{GAMES} games, {} positions", positions.load(Ordering::SeqCst));

            return Ok(());
        }
    }

    println!("{}", YUKARI.purple());

    let mut line = String::new();
    loop {
        line.clear();
        let count = io::stdin().read_line(&mut line)?;
        if count == 0 {
            println!("# got zero read");
            continue;
        }
        let trimmed = line.trim();
        let (mut cmd, mut args) = trimmed.split_once(' ').unwrap_or((trimmed, ""));

        #[allow(clippy::match_same_arms)]
        match cmd {
            // Identification for engines that auto switch between protocols
            "xboard" => {
                protocol = Protocol::Xboard;
            }
            "uci" => {
                protocol = Protocol::Uci;
                println!("id name Yukari 2025.4.1");
                println!("id author Hannah Ravensloft");
                println!("option name Hash type spin default 16 min 1 max 8192");
                println!("option name Threads type spin default 1 min 1 max 1");
                println!("uciok");
            }
            // This is where we send our features
            "protover" => {
                // v1 won't send this anyway and we need v2
                assert_eq!(args, "2");
                // Do features individually
                println!("feature myname=\"Yukari 2025.4.1\"");
                // No signals support
                println!("feature sigint=0 sigterm=0");
                // Ping feature helps with race conditions
                println!("feature ping=1");
                // We would rather get FEN updates of the board than white/black
                println!("feature colors=0 setboard=1");
                // Technically needed to support those # <msg> lines
                println!("feature debug=1");
                // We support hash table allocation sizing.
                println!("feature memory=1");
                // We support nps for fixed-nodes search.
                println!("feature nps=1");
                // Tunables!
                println!("feature option=\"RfpMarginBase -spin 0 0 100\"");
                println!("feature option=\"RfpMarginMul -spin 37 0 1000\"");
                println!("feature option=\"RazorMarginMul -spin 250 0 500\"");
                println!("feature option=\"LmrBase -string 1.0\"");
                println!("feature option=\"LmrMul -string 0.5\"");
                println!("feature option=\"HistBonusBase -spin 250 0 500\"");
                println!("feature option=\"HistBonusMul -spin 300 0 600\"");
                println!("feature option=\"HistPenaltyBase -spin 250 0 500\"");
                println!("feature option=\"HistPenaltyMul -spin 300 0 600\"");
                println!("feature option=\"SeePruningCapture -string 0.5\"");
                println!("feature option=\"SeePruningQuiet -string 0.0\"");
                println!("feature option=\"Hash -spin 16 1 8192\"");
                println!("feature option=\"Threads -spin 1 1 1\"");
                // Communicate that feature reporting is done
                println!("feature done=1");
            }
            // Directly update the engine's board from a FEN
            "setboard" => engine.set_board(args),
            "position" => {
                engine.mode = Mode::Force;
                engine.keystack.clear();
                (cmd, args) = args.split_once(' ').unwrap_or((args, ""));
                match cmd {
                    "startpos" => engine.board = Board::startpos(),
                    "fen" => {
                        (cmd, args) = args.split_once(' ').unwrap_or((args, ""));
                        let mut fen = cmd.to_string();
                        (cmd, args) = args.split_once(' ').unwrap_or((args, ""));
                        fen.push(' ');
                        fen.push_str(cmd);
                        (cmd, args) = args.split_once(' ').unwrap_or((args, ""));
                        fen.push(' ');
                        fen.push_str(cmd);
                        (cmd, args) = args.split_once(' ').unwrap_or((args, ""));
                        fen.push(' ');
                        fen.push_str(cmd);
                        (cmd, args) = args.split_once(' ').unwrap_or((args, ""));
                        fen.push(' ');
                        fen.push_str(cmd);
                        (cmd, args) = args.split_once(' ').unwrap_or((args, ""));
                        fen.push(' ');
                        fen.push_str(cmd);
                        engine.board = Board::from_fen(&fen).unwrap();
                    }
                    _ => unreachable!("unrecognised position subcommand"),
                }
                if !args.is_empty() {
                    (cmd, args) = args.split_once(' ').unwrap_or((args, ""));
                    assert_eq!(cmd, "moves");
                    engine.tc.reset_moves();
                    while !args.is_empty() {
                        (cmd, args) = args.split_once(' ').unwrap_or((args, ""));

                        let chars = cmd.as_bytes();
                        let from = Square::from_str(&cmd[..2]).unwrap();
                        let dest = Square::from_str(&cmd[2..4]).unwrap();
                        let prom = if chars.len() == 5 {
                            match chars[4] {
                                b'n' => Some(Piece::Knight),
                                b'b' => Some(Piece::Bishop),
                                b'r' => Some(Piece::Rook),
                                b'q' => Some(Piece::Queen),
                                _ => None,
                            }
                        } else {
                            None
                        };

                        let m = engine.find_move(from, dest, prom).unwrap_or_else(|| panic!("Attempted move {cmd} not found!?"));
                        engine.board = engine.board.make(m);
                        engine.keystack.push(engine.board.hash());
                        engine.tc.increment_moves();
                    }
                }
            }
            // Reset the entire state of the engine
            "new" | "ucinewgame" => engine = Yukari::new(),
            // Parse our two time controls from the whole commmand lines
            // TODO: This is rather xboard specific
            "level" => engine.parse_tc(trimmed),
            "st" => engine.tc.mode.fixed_time_per_move(args.parse::<f32>().unwrap()),
            "name" | "rating" | "post" => {}
            // Allocate a hash table.
            "memory" => {
                let megabytes = args.parse::<usize>().unwrap();
                tt = allocate_tt(megabytes);
            }
            "option" => {
                let (name, value) = args.split_once('=').unwrap();
                if name == "Hash" {
                    // UCIism. grumble grumble.
                    let value = value.parse::<i32>().unwrap();
                    if value >= 1 {
                        tt = allocate_tt(value as usize);
                    }
                }
                match name {
                    "RfpMarginBase" => engine.params.rfp_margin_base = value.parse::<i32>().unwrap(),
                    "RfpMarginMul" => engine.params.rfp_margin_mul = value.parse::<i32>().unwrap(),
                    "RazorMarginMul" => engine.params.razor_margin_mul = value.parse::<i32>().unwrap(),
                    "LmrBase" => engine.params.lmr_base = value.parse::<f32>().unwrap(),
                    "LmrMul" => engine.params.lmr_mul = value.parse::<f32>().unwrap(),
                    "HistBonusBase" => engine.params.hist_bonus_base = value.parse::<i32>().unwrap(),
                    "HistBonusMul" => engine.params.hist_bonus_mul = value.parse::<i32>().unwrap(),
                    "HistPenaltyBase" => engine.params.hist_pen_base = value.parse::<i32>().unwrap(),
                    "HistPenaltyMul" => engine.params.hist_pen_mul = value.parse::<i32>().unwrap(),
                    "SeePruningCapture" => engine.params.see_pruning_capture = value.parse::<f32>().unwrap(),
                    "SeePruningQuiet" => engine.params.see_pruning_quiet = value.parse::<f32>().unwrap(),
                    _ => (),
                }
            }
            "setoption" => {
                let (name, args) = args.split_once(' ').unwrap_or((args, ""));
                assert_eq!(name, "name");
                let (name, args) = args.split_once(' ').unwrap_or((args, ""));
                let (value, args) = args.split_once(' ').unwrap_or((args, ""));
                assert_eq!(value, "value");
                let (value, _) = args.split_once(' ').unwrap_or((args, ""));
                let value = value.parse::<i32>().unwrap();
                match name {
                    "RfpMarginBase" => engine.params.rfp_margin_base = value,
                    "RfpMarginMul" => engine.params.rfp_margin_mul = value,
                    "LmrBase" => engine.params.lmr_base = (value as f32) / 100.0,
                    "LmrMul" => engine.params.lmr_mul = (value as f32) / 1000.0,
                    "HistBonusBase" => engine.params.hist_bonus_base = value,
                    "HistBonusMul" => engine.params.hist_bonus_mul = value,
                    "HistPenaltyBase" => engine.params.hist_pen_base = value,
                    "HistPenaltyMul" => engine.params.hist_pen_mul = value,
                    "Hash" if value >= 1 => tt = allocate_tt(value as usize), // UCIism. grumble grumble.
                    "Threads" => (),                                          // UCIism, grumble grumble.
                    _ => (),
                }
            }
            "eval" => println!("{}", engine.board.eval(engine.board.side())),
            // Hard would turn on thinking during opponent's time, easy would turn it off
            // we don't do it, so it's unimportant
            "hard" | "easy" => {}
            "quit" => {
                break;
            }
            // Feature replies are just ignored since we don't turn anything off yet
            // TODO: Handle rejects we can't tolerate and abort early
            "accepted" | "rejected" => {}
            // Ping expects a response with the correct tag once the commands prior to the ping are done
            // That ends up being some GPU fence level synchronization nonsense if it were to send more than one
            // so for now we just "handle it" by replying with pong immediately. For now this "works" because
            // the engine is single threaded such that moves can never be passed by other commands
            "ping" => println!("pong {args}"),
            "isready" => println!("readyok"),
            // TODO: Should support randomization so we don't always play the same game
            // we can't todo!() because we cannot turn off getting this message
            "random" => {}
            // We don't implement games against computer players games differently
            "computer" => {}
            // This report gives us info about what time we have left right now directly
            // the value is in centiseconds
            "time" => engine.set_remaining(f32::from_str(args).unwrap()),
            "otim" => {}
            "sd" => engine.set_depth(i32::from_str(args).unwrap()),
            "nps" => engine.set_nodes_per_second(i32::from_str(args).unwrap()),
            "go" => {
                let mut uci = false;
                // is this a UCI go?
                while !args.is_empty() {
                    uci = true;
                    (cmd, args) = args.split_once(' ').unwrap_or((args, ""));
                    match cmd {
                        "wtime" => {
                            (cmd, args) = args.split_once(' ').unwrap_or((args, ""));
                            if engine.board.side() == Colour::White {
                                engine.set_remaining((u32::from_str(cmd).unwrap() / 10) as f32);
                            }
                        }
                        "btime" => {
                            (cmd, args) = args.split_once(' ').unwrap_or((args, ""));
                            if engine.board.side() == Colour::Black {
                                engine.set_remaining((u32::from_str(cmd).unwrap() / 10) as f32);
                            }
                        }
                        "winc" => {
                            (cmd, args) = args.split_once(' ').unwrap_or((args, ""));
                            if engine.board.side() == Colour::White {
                                engine.tc.mode.increment(u32::from_str(cmd).unwrap());
                            }
                        }
                        "binc" => {
                            (cmd, args) = args.split_once(' ').unwrap_or((args, ""));
                            if engine.board.side() == Colour::Black {
                                engine.tc.mode.increment(u32::from_str(cmd).unwrap());
                            }
                        }
                        "movetime" => {
                            (cmd, args) = args.split_once(' ').unwrap_or((args, ""));
                            engine.tc.mode.fixed_time_per_move((u32::from_str(cmd).unwrap() as f32) / 1000.0);
                        }
                        "depth" => {
                            (cmd, args) = args.split_once(' ').unwrap_or((args, ""));
                            engine.max_depth = Some(i32::from_str(cmd).unwrap());
                        }
                        "nodes" => {
                            (cmd, args) = args.split_once(' ').unwrap_or((args, ""));
                            engine.nodes_per_second = Some(u32::from_str(cmd).unwrap());
                            engine.tc.mode.fixed_time_per_move(1.0);
                        }
                        "mate" => {
                            (_, args) = args.split_once(' ').unwrap_or((args, ""));
                        }
                        "infinite" | "ponder" => {}
                        _ => {} // ignore anything we don't understand.
                    }
                }
                engine.mode = Mode::Normal;
                // When we get go we should make a move immediately
                let mut pv = ArrayVec::new();
                engine.search(&mut pv, &mut tt, protocol);
                // Choose the top move
                let m = pv[0];
                if uci {
                    println!("bestmove {m}");
                    engine.mode = Mode::Force;
                } else {
                    // We must actually make the move locally too
                    engine.board = engine.board.make(m);
                    println!("move {m}");
                    if is_repetition_draw(&engine.keystack, engine.board.hash()) {
                        println!("1/2-1/2 {{Draw by repetition}}");
                    }
                    engine.keystack.push(engine.board.hash());
                    engine.tc.increment_moves();
                }
            }
            "force" => engine.mode = Mode::Force,
            "d" => println!("{}", engine.board),
            _ => {
                // Always ascii
                let chars = trimmed.as_bytes();
                if chars[1].is_ascii_digit() && chars[3].is_ascii_digit() {
                    // This is actually a move
                    let from = Square::from_str(&cmd[..2]).unwrap();
                    let dest = Square::from_str(&cmd[2..4]).unwrap();
                    let prom = if chars.len() == 5 {
                        match chars[4] {
                            b'n' => Some(Piece::Knight),
                            b'b' => Some(Piece::Bishop),
                            b'r' => Some(Piece::Rook),
                            b'q' => Some(Piece::Queen),
                            _ => None,
                        }
                    } else {
                        None
                    };
                    match engine.mode {
                        Mode::Normal => {
                            // Find the move in the list
                            let m = engine.find_move(from, dest, prom).expect("Attempted move not found!?");
                            engine.board = engine.board.make(m);
                            engine.tc.increment_moves();
                            if is_repetition_draw(&engine.keystack, engine.board.hash()) {
                                println!("1/2-1/2 {{Draw by repetition}}");
                            }
                            engine.keystack.push(engine.board.hash());
                            // Find the next move to make
                            // TODO: Cleanups
                            let mut pv = ArrayVec::new();
                            engine.search(&mut pv, &mut tt, protocol);
                            // Choose the top move
                            let m = pv[0];
                            // We must actually make the move locally too
                            engine.board = engine.board.make(m);
                            println!("move {m}");
                            if is_repetition_draw(&engine.keystack, engine.board.hash()) {
                                println!("1/2-1/2 {{Draw by repetition}}");
                            }
                            engine.keystack.push(engine.board.hash());
                            engine.tc.increment_moves();
                        }
                        Mode::Force => {
                            let m = engine.find_move(from, dest, prom).expect("Attempted move not found!?");
                            engine.board = engine.board.make(m);
                            if is_repetition_draw(&engine.keystack, engine.board.hash()) {
                                println!("1/2-1/2 {{Draw by repetition}}");
                            }
                            engine.keystack.push(engine.board.hash());
                            engine.tc.increment_moves();
                        }
                    }
                } else {
                    // This may look like I chose the format, but it is a standard response
                    println!("Error (unknown command): {trimmed}");
                }
            }
        }
    }
    Ok(())
}
