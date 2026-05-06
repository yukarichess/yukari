use std::simd::{cmp::SimdOrd, i8x32, i16x32, i32x32, num::SimdInt};

use super::feature;
use crate::{Colour, File, Piece, Square};

pub const HORIZONTAL_MIRROR: bool = true;
const INPUTS: usize = (2 * 6 * 64) + 2 * feature::MAX_OFFSET;
const HIDDEN_SIZE: usize = 512;
const OUTPUT_BUCKETS: usize = 8;
const DIVISOR: usize = 32_usize.div_ceil(OUTPUT_BUCKETS);
const SCALE: i32 = 400;
const QA: i16 = 255;
const QB: i16 = 64;

/// This is the quantised format that yukari uses.
#[repr(C)]
pub struct Network {
    /// Column-Major `HIDDEN_SIZE x INPUTS` matrix.
    feature_threat_weights: [[i8; HIDDEN_SIZE]; 60144],
    feature_pst_weights:    [Accumulator; 768],
    /// Vector with dimension `HIDDEN_SIZE`.
    feature_bias:           Accumulator,
    /// Row-Major `OUTPUT_BUCKETS x (2 * HIDDEN_SIZE)` matrix.
    output_weights:         [[Accumulator; 2]; OUTPUT_BUCKETS],
    /// Scalar output biases.
    output_bias:            [i16; OUTPUT_BUCKETS],
}

static NNUE: Network =
    unsafe { std::mem::transmute::<[u8; std::mem::size_of::<Network>()], Network>(*include_bytes!(env!("EVALFILE"))) };

impl Network {
    /// Calculates the output of the network, starting from the already
    /// calculated hidden layer (done efficiently during makemoves).
    pub fn evaluate(&self, us: &Accumulator, them: &Accumulator, output_bucket: usize) -> i32 {
        // Initialise output with bias.
        let mut output = i32x32::splat(0);
        let min = i16x32::splat(0);
        let max = i16x32::splat(QA);

        // Side-To-Move Accumulator -> Output.
        let (us_vals, []) = us.vals.as_chunks::<32>() else { unreachable!() };
        let (output_weights, []) = self.output_weights[output_bucket][0].vals.as_chunks::<32>() else {
            unreachable!()
        };
        for (input, weight) in us_vals.iter().zip(output_weights.iter()) {
            // Squared Clipped `ReLU` - Activation Function.
            // Note that this takes the i16s in the accumulator to i32s.
            let input = i16x32::from_array(*input).simd_clamp(min, max);
            let weight = input * i16x32::from_array(*weight);
            output += input.cast::<i32>() * weight.cast::<i32>();
        }

        // Not-Side-To-Move Accumulator -> Output.
        let (them_vals, []) = them.vals.as_chunks::<32>() else { unreachable!() };
        let (output_weights, []) = self.output_weights[output_bucket][1].vals.as_chunks::<32>() else {
            unreachable!()
        };
        for (input, weight) in them_vals.iter().zip(output_weights.iter()) {
            let input = i16x32::from_array(*input).simd_clamp(min, max);
            let weight = input * i16x32::from_array(*weight);
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
    #[inline(never)]
    pub fn add_feature(&mut self, feature_idx: usize, net: &Network) {
        let (acc_chunks, []) = self.vals.as_chunks_mut::<32>() else { unreachable!() };
        if feature_idx < 768 {
            let (weight_chunks, []) = net.feature_pst_weights[feature_idx].vals.as_chunks::<32>() else { unreachable!() };
            for (i, d) in acc_chunks.iter_mut().zip(weight_chunks.iter()) {
                *i = (i16x32::from_array(*i) + i16x32::from_array(*d)).to_array();
            }
        } else {
            let feature_idx = feature_idx - 768;
            let (weight_chunks, []) = net.feature_threat_weights[feature_idx].as_chunks::<32>() else { unreachable!() };
            for (i, d) in acc_chunks.iter_mut().zip(weight_chunks.iter()) {
                *i = (i16x32::from_array(*i) + i8x32::from_array(*d).cast::<i16>()).to_array();
            }
        }
    }

    /// Remove a feature from an accumulator.
    #[inline(never)]
    pub fn remove_feature(&mut self, feature_idx: usize, net: &Network) {
        let (acc_chunks, []) = self.vals.as_chunks_mut::<32>() else { unreachable!() };
        if feature_idx < 768 {
            let (weight_chunks, []) = net.feature_pst_weights[feature_idx].vals.as_chunks::<32>() else { unreachable!() };
            for (i, d) in acc_chunks.iter_mut().zip(weight_chunks.iter()) {
                *i = (i16x32::from_array(*i) - i16x32::from_array(*d)).to_array();
            }
        } else {
            let feature_idx = feature_idx - 768;
            let (weight_chunks, []) = net.feature_threat_weights[feature_idx].as_chunks::<32>() else { unreachable!() };
            for (i, d) in acc_chunks.iter_mut().zip(weight_chunks.iter()) {
                *i = (i16x32::from_array(*i) - i8x32::from_array(*d).cast::<i16>()).to_array();
            }
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
        if HORIZONTAL_MIRROR && File::from(king) >= File::E { 7 } else { 0 }
    }

    pub fn add_piece_for_acc(
        &mut self, piece: Piece, square: Square, colour: Colour, white_king: Square, black_king: Square, white_acc: bool,
    ) {
        if white_acc {
            self.white
                .add_feature(feature::index_pst(piece, square, white_king, colour == Colour::White), &NNUE);
        } else {
            self.black
                .add_feature(feature::index_pst(piece, square.flip(), black_king, colour == Colour::Black), &NNUE);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_threat_for_acc(
        &mut self, from_piece: Piece, from_square: Square, to_piece: Piece, to_square: Square, from_colour: Colour,
        to_colour: Option<Colour>, white_king: Square, black_king: Square, white_acc: bool,
    ) {
        let Some(to_colour) = to_colour else { return };
        if white_acc {
            //print!("+ ");
            let Some(feature_idx) = feature::index_threat(
                from_piece,
                from_square,
                to_piece,
                to_square,
                white_king,
                from_colour == Colour::White,
                to_colour == Colour::Black,
                false,
            ) else {
                return;
            };
            self.white.add_feature(feature_idx, &NNUE);
        } else {
            //print!("+ ");
            let Some(feature_idx) = feature::index_threat(
                from_piece,
                from_square.flip(),
                to_piece,
                to_square.flip(),
                black_king,
                from_colour == Colour::Black,
                to_colour == Colour::White,
                true,
            ) else {
                return;
            };
            self.black.add_feature(feature_idx, &NNUE);
        }
    }

    pub fn add_piece(&mut self, piece: Piece, square: Square, colour: Colour, white_king: Square, black_king: Square) {
        self.white
            .add_feature(feature::index_pst(piece, square, white_king, colour == Colour::White), &NNUE);
        self.black
            .add_feature(feature::index_pst(piece, square.flip(), black_king, colour == Colour::Black), &NNUE);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_threat(
        &mut self, from_piece: Piece, from_square: Square, to_piece: Option<Piece>, to_square: Square, from_colour: Colour,
        to_colour: Option<Colour>, white_king: Square, black_king: Square,
    ) {
        let Some(to_piece) = to_piece else { return };
        let Some(to_colour) = to_colour else { return };
        //print!("+ ");
        let white_feature_idx = feature::index_threat(
            from_piece,
            from_square,
            to_piece,
            to_square,
            white_king,
            from_colour == Colour::White,
            to_colour == Colour::Black,
            false,
        );
        //print!("+ ");
        let black_feature_idx = feature::index_threat(
            from_piece,
            from_square.flip(),
            to_piece,
            to_square.flip(),
            black_king,
            from_colour == Colour::Black,
            to_colour == Colour::White,
            true,
        );

        if let Some(white_feature_idx) = white_feature_idx {
            self.white.add_feature(white_feature_idx, &NNUE);
        }
        if let Some(black_feature_idx) = black_feature_idx {
            self.black.add_feature(black_feature_idx, &NNUE);
        }
    }

    pub fn remove_piece_for_acc(
        &mut self, piece: Piece, square: Square, colour: Colour, white_king: Square, black_king: Square, white_acc: bool,
    ) {
        if white_acc {
            self.white
                .remove_feature(feature::index_pst(piece, square, white_king, colour == Colour::White), &NNUE);
        } else {
            self.black
                .remove_feature(feature::index_pst(piece, square.flip(), black_king, colour == Colour::Black), &NNUE);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn remove_threat_for_acc(
        &mut self, from_piece: Piece, from_square: Square, to_piece: Piece, to_square: Square, from_colour: Colour,
        to_colour: Option<Colour>, white_king: Square, black_king: Square, white_acc: bool,
    ) {
        let Some(to_colour) = to_colour else { return };
        if white_acc {
            //print!("- ");
            let Some(feature_idx) = feature::index_threat(
                from_piece,
                from_square,
                to_piece,
                to_square,
                white_king,
                from_colour == Colour::White,
                to_colour == Colour::Black,
                false,
            ) else {
                return;
            };
            self.white.remove_feature(feature_idx, &NNUE);
        } else {
            //print!("- ");
            let Some(feature_idx) = feature::index_threat(
                from_piece,
                from_square.flip(),
                to_piece,
                to_square.flip(),
                black_king,
                from_colour == Colour::Black,
                to_colour == Colour::White,
                true,
            ) else {
                return;
            };
            self.black.remove_feature(feature_idx, &NNUE);
        }
    }

    pub fn remove_piece(&mut self, piece: Piece, square: Square, colour: Colour, white_king: Square, black_king: Square) {
        self.white
            .remove_feature(feature::index_pst(piece, square, white_king, colour == Colour::White), &NNUE);
        self.black
            .remove_feature(feature::index_pst(piece, square.flip(), black_king, colour == Colour::Black), &NNUE);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn remove_threat(
        &mut self, from_piece: Piece, from_square: Square, to_piece: Option<Piece>, to_square: Square, from_colour: Colour,
        to_colour: Option<Colour>, white_king: Square, black_king: Square,
    ) {
        let Some(to_piece) = to_piece else { return };
        let Some(to_colour) = to_colour else { return };
        //print!("- ");
        let white_feature_idx = feature::index_threat(
            from_piece,
            from_square,
            to_piece,
            to_square,
            white_king,
            from_colour == Colour::White,
            to_colour == Colour::Black,
            false,
        );
        //print!("- ");
        let black_feature_idx = feature::index_threat(
            from_piece,
            from_square.flip(),
            to_piece,
            to_square.flip(),
            black_king,
            from_colour == Colour::Black,
            to_colour == Colour::White,
            true,
        );

        if let Some(white_feature_idx) = white_feature_idx {
            self.white.remove_feature(white_feature_idx, &NNUE);
        }
        if let Some(black_feature_idx) = black_feature_idx {
            self.black.remove_feature(black_feature_idx, &NNUE);
        }
    }

    pub fn move_piece(
        &mut self, piece: Piece, from_square: Square, to_square: Square, colour: Colour, white_king: Square, black_king: Square,
    ) {
        self.remove_piece(piece, from_square, colour, white_king, black_king);
        self.add_piece(piece, to_square, colour, white_king, black_king);
    }
}
