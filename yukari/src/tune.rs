use std::convert::TryInto;

use yukari_movegen::{Board, Colour, Piece, Square};
use revad::tape::{Tape, Var};
use argmin::{prelude::*, solver::{linesearch::{ArmijoCondition, BacktrackingLineSearch, MoreThuenteLineSearch}, quasinewton::LBFGS}};

#[derive(Clone)]
pub struct EvalState<'a> {
    pst_mg: Var<'a>,
    pst_eg: Var<'a>,
    phase: Var<'a>
}

impl<'a> EvalState<'a> {
    pub fn new(t: &'a Tape) -> Self {
        Self {
            pst_mg: t.var(0.0),
            pst_eg: t.var(0.0),
            phase: t.var(0.0),
        }
    }

    pub fn get(&self, tape: &'a Tape, colour: Colour) -> Var<'a> {
        let score = tape.var(1.0 / 24.0) * ((self.pst_mg * self.phase) + (self.pst_eg * (tape.var(24.0) - self.phase)));
        if colour == Colour::White {
            score
        } else {
            -score
        }
    }

    pub fn add_piece(&mut self, eval: &'a Eval, piece: Piece, square: Square, colour: Colour) {
        if colour == Colour::White {
            self.pst_mg = self.pst_mg + eval.pst_mg[piece as usize][square.into_inner() as usize];
            self.pst_eg = self.pst_eg + eval.pst_eg[piece as usize][square.into_inner() as usize];
        } else {
            self.pst_mg = self.pst_mg - eval.pst_mg[piece as usize][square.flip().into_inner() as usize];
            self.pst_eg = self.pst_eg - eval.pst_eg[piece as usize][square.flip().into_inner() as usize];
        }
        self.phase = self.phase + eval.phase[piece as usize];
    }
}

pub struct Eval<'a> {
    pub pst_mg: [[Var<'a>; 64]; 6],
    pub pst_eg: [[Var<'a>; 64]; 6],
    pub phase: [Var<'a>; 6],
}

impl<'a> Eval<'a> {
    pub fn from_tuning_weights(tape: &'a Tape, weights: &'a [Var<'a>]) -> Self {
        Self {
            pst_mg: [
                // Pawn
                weights[0..64].try_into().unwrap(),
                // Knight
                weights[64..128].try_into().unwrap(),
                // Bishop
                weights[128..192].try_into().unwrap(),
                // Rook
                weights[192..256].try_into().unwrap(),
                // Queen
                weights[256..320].try_into().unwrap(),
                // King
                weights[320..384].try_into().unwrap()
            ],
            pst_eg: [
                // Pawn
                weights[384..448].try_into().unwrap(),
                // Knight
                weights[448..512].try_into().unwrap(),
                // Bishop
                weights[512..576].try_into().unwrap(),
                // Rook
                weights[576..640].try_into().unwrap(),
                // Queen
                weights[640..704].try_into().unwrap(),
                // King
                weights[704..768].try_into().unwrap()
            ],
            phase: [tape.var(0.0), tape.var(1.0), tape.var(1.0), tape.var(2.0), tape.var(4.0), tape.var(0.0)]
        }
    }

    pub fn gradient(&'a self, board: &Board, tape: &'a Tape) -> Var<'a> {
        let mut score = EvalState::new(tape);

        for piece in board.pieces() {
            let square = board.square_of_piece(piece);
            score.add_piece(self, board.piece_from_bit(piece), square, piece.colour());
        }

        (tape.var(0.00255) * score.get(tape, board.side())).tanh()
    }
}

#[derive(Clone)]
pub struct Tune {
    boards: Vec<(Board, f64)>,
}

impl ArgminOp for Tune {
    type Param = Vec<f64>;

    type Output = f64;

    type Hessian = Vec<Vec<f64>>;

    type Jacobian = ();

    type Float = f64;

    fn apply(&self, param: &Self::Param) -> Result<Self::Output, Error> {
        let tape = Tape::new();

        let mut weights = Vec::with_capacity(param.len());

        for param in param {
            weights.push(tape.var(*param));
        }

        let eval = Eval::from_tuning_weights(&tape, &weights);
        let size = 1.0 / self.boards.len() as f64;

        let mut loss = 0.0;

        for (board, result) in &self.boards {
            let eval = eval.gradient(board, &tape).value();
            let diff = eval - *result;
            loss += 0.5 * size * diff * diff;
        }

        println!("{:.8}", loss);

        Ok(loss)
    }

    fn gradient(&self, param: &Self::Param) -> Result<Self::Param, Error> {
        let tape = Tape::new();

        let mut weights = Vec::with_capacity(param.len());

        for param in param {
            weights.push(tape.var(*param));
        }

        let eval = Eval::from_tuning_weights(&tape, &weights);

        let mut loss = tape.var(0.0);
        let size = tape.var(1.0 / self.boards.len() as f64);
        let a_half = tape.var(0.5);

        for (board, result) in &self.boards {
            let eval = eval.gradient(board, &tape);
            let diff = eval - tape.var(*result);
            loss = loss + (a_half * size * diff * diff);
        }

        let derivs = loss.grad();
        let mut gradients = vec![0.0; param.len()];

        for (index, gradient) in gradients.iter_mut().enumerate() {
            let deriv = derivs.wrt(weights[index]);
            *gradient = deriv;
        }

        Ok(gradients)
    }
}

impl Tune {
    pub fn new(boards: Vec<(Board, f64)>) -> Self {
        Self {
            boards
        }
    }

    pub fn tune(&self) -> Result<(), Error> {
        let init_param = vec![0.0; 768];

        //let cond = ArmijoCondition::new(0.5)?;
        //let linesearch = BacktrackingLineSearch::new(cond).rho(0.9)?;
        let linesearch = MoreThuenteLineSearch::new();
        let solver = LBFGS::new(linesearch, 7);
        let res = Executor::new(self.clone(), solver, init_param)
        .add_observer(ArgminSlogLogger::term(), ObserverMode::Always)
        .max_iters(100)
        .run()?;

        std::thread::sleep(std::time::Duration::from_secs(1));

        println!("{}", res);

        Ok(())
    }
}
