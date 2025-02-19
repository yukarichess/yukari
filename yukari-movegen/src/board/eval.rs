use std::simd::{cmp::SimdOrd, i16x64, i32x64, num::SimdInt};

use crate::{Colour, File, Piece, Square};

pub const HORIZONTAL_MIRROR: bool = true;
const HIDDEN_SIZE: usize = 1024;
const OUTPUT_BUCKETS: usize = 8;
const DIVISOR: usize = 32_usize.div_ceil(OUTPUT_BUCKETS);
const SCALE: i32 = 400;
const QA: i16 = 255;
const QB: i16 = 64;

/// This is the quantised format that yukari uses.
#[repr(C)]
pub struct Network {
    /// Column-Major `HIDDEN_SIZE x 768` matrix.
    feature_weights: [Accumulator; 768],
    /// Vector with dimension `HIDDEN_SIZE`.
    feature_bias: Accumulator,
    /// Row-Major `OUTPUT_BUCKETS x (2 * HIDDEN_SIZE)` matrix.
    output_weights: [[Accumulator; 2]; OUTPUT_BUCKETS],
    /// Scalar output biases.
    output_bias: [i16; OUTPUT_BUCKETS],
}

static NNUE: Network = unsafe {
    std::mem::transmute::<[u8; std::mem::size_of::<Network>()], Network>(*include_bytes!("../../../yukari_8bd20941.bin"))
};

impl Network {
    /// Calculates the output of the network, starting from the already
    /// calculated hidden layer (done efficiently during makemoves).
    pub fn evaluate(&self, us: &Accumulator, them: &Accumulator, output_bucket: usize) -> i32 {
        // Initialise output with bias.
        let mut output = i32x64::splat(0);
        let min = i16x64::splat(0);
        let max = i16x64::splat(QA);

        // Side-To-Move Accumulator -> Output.
        for (input, weight) in us.vals.array_chunks::<64>().zip(self.output_weights[output_bucket][0].vals.array_chunks::<64>()) {
            // Squared Clipped `ReLU` - Activation Function.
            // Note that this takes the i16s in the accumulator to i32s.
            let input = i16x64::from_array(*input).simd_clamp(min, max);
            let weight = input * i16x64::from_array(*weight);
            output += input.cast::<i32>() * weight.cast::<i32>();
        }

        // Not-Side-To-Move Accumulator -> Output.
        for (input, weight) in them.vals.array_chunks::<64>().zip(self.output_weights[output_bucket][1].vals.array_chunks::<64>()) {
            let input = i16x64::from_array(*input).simd_clamp(min, max);
            let weight = input * i16x64::from_array(*weight);
            output += input.cast::<i32>() * weight.cast::<i32>();
        }

        let mut output = (output.reduce_sum() / i32::from(QA)) + i32::from(self.output_bias[output_bucket]);

        // Apply eval scale.
        output *= SCALE;

        // Remove quantisation.
        output / (i32::from(QA) * i32::from(QB))
    }
}

/// A column of the feature-weights matrix.
/// Note the `align(64)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C, align(64))]
pub struct Accumulator {
    vals: [i16; HIDDEN_SIZE],
}

impl Accumulator {
    /// Initialised with bias so we can just efficiently
    /// operate on it afterwards.
    pub const fn new(net: &Network) -> Self {
        net.feature_bias
    }

    /// Add a feature to an accumulator.
    pub fn add_feature(&mut self, feature_idx: usize, net: &Network) {
        for (i, d) in self.vals.iter_mut().zip(&net.feature_weights[feature_idx].vals) {
            *i += *d;
        }
    }

    /// Remove a feature from an accumulator.
    pub fn remove_feature(&mut self, feature_idx: usize, net: &Network) {
        for (i, d) in self.vals.iter_mut().zip(&net.feature_weights[feature_idx].vals) {
            *i -= *d;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Eval {
    white: Accumulator,
    black: Accumulator,
}

impl Eval {
    pub fn new() -> Self {
        Self { white: Accumulator::new(&NNUE), black: Accumulator::new(&NNUE) }
    }

    pub fn reset_colour(&mut self, colour: Colour) {
        if colour == Colour::White {
            self.white = Accumulator::new(&NNUE);
        } else {
            self.black = Accumulator::new(&NNUE);
        }
    }

    pub fn get(&self, piece_count: usize, colour: Colour) -> i32 {
        let output_bucket = (piece_count - 2) / DIVISOR;
        if colour == Colour::White {
            NNUE.evaluate(&self.white, &self.black, output_bucket)
        } else {
            NNUE.evaluate(&self.black, &self.white, output_bucket)
        }
    }

    pub fn mirror(king: Square) -> usize {
        if HORIZONTAL_MIRROR && File::from(king) >= File::E {
            7
        } else {
            0
        }
    }

    pub fn add_piece_for_acc(&mut self, piece: Piece, square: Square, colour: Colour, white_king: Square, black_king: Square, white_acc: bool) {
        let white_square = square.into_inner() as usize ^ Self::mirror(white_king);
        let black_square = square.flip().into_inner() as usize ^ Self::mirror(black_king);

        if colour == Colour::White {
            if white_acc {
                self.white.add_feature(64 * (piece as usize) + white_square, &NNUE);
            } else {
                self.black.add_feature(64 * (6 + piece as usize) + black_square, &NNUE);
            }
        } else if white_acc {
            self.white.add_feature(64 * (6 + piece as usize) + white_square, &NNUE);
        } else {
            self.black.add_feature(64 * (piece as usize) + black_square, &NNUE);
        }
    }

    pub fn add_piece(&mut self, piece: Piece, square: Square, colour: Colour, white_king: Square, black_king: Square) {
        let white_square = square.into_inner() as usize ^ Self::mirror(white_king);
        let black_square = square.flip().into_inner() as usize ^ Self::mirror(black_king);

        if colour == Colour::White {
            self.white.add_feature(64 * (piece as usize) + white_square, &NNUE);
            self.black.add_feature(64 * (6 + piece as usize) + black_square, &NNUE);
        } else {
            self.black.add_feature(64 * (piece as usize) + black_square, &NNUE);
            self.white.add_feature(64 * (6 + piece as usize) + white_square, &NNUE);
        }
    }

    pub fn remove_piece_for_acc(&mut self, piece: Piece, square: Square, colour: Colour, white_king: Square, black_king: Square, white_acc: bool) {
        let white_square = square.into_inner() as usize ^ Self::mirror(white_king);
        let black_square = square.flip().into_inner() as usize ^ Self::mirror(black_king);

        if colour == Colour::White {
            if white_acc {
                self.white.remove_feature(64 * (piece as usize) + white_square, &NNUE);
            } else {
                self.black.remove_feature(64 * (6 + piece as usize) + black_square, &NNUE);
            }
        } else if white_acc {
            self.white.remove_feature(64 * (6 + piece as usize) + white_square, &NNUE);
        } else {
            self.black.remove_feature(64 * (piece as usize) + black_square, &NNUE);
        }
    }

    pub fn remove_piece(&mut self, piece: Piece, square: Square, colour: Colour, white_king: Square, black_king: Square) {
        let white_square = square.into_inner() as usize ^ Self::mirror(white_king);
        let black_square = square.flip().into_inner() as usize ^ Self::mirror(black_king);

        if colour == Colour::White {
            self.white.remove_feature(64 * (piece as usize) + white_square, &NNUE);
            self.black.remove_feature(64 * (6 + piece as usize) + black_square, &NNUE);
        } else {
            self.black.remove_feature(64 * (piece as usize) + black_square, &NNUE);
            self.white.remove_feature(64 * (6 + piece as usize) + white_square, &NNUE);
        }
    }

    pub fn move_piece(&mut self, piece: Piece, from_square: Square, to_square: Square, colour: Colour, white_king: Square, black_king: Square) {
        self.remove_piece(piece, from_square, colour, white_king, black_king);
        self.add_piece(piece, to_square, colour, white_king, black_king);
    }
}
