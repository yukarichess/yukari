use std::{
    ops::Index, simd::{
        Simd, cmp::SimdOrd, i8x32, i16x32, num::{SimdFloat, SimdInt}
    }
};

use super::feature;
use crate::{Colour, File, Piece, Square};

pub const HORIZONTAL_MIRROR: bool = true;
const L1_SIZE: usize = 256;
const L2_SIZE: usize = 16;
const OUTPUT_BUCKETS: usize = 8;
const DIVISOR: u8 = 32_u8.div_ceil(OUTPUT_BUCKETS as u8);
const SCALE: i32 = 400;
const QA: i16 = 255;
const QB: i16 = 64;

type I16xL2 = Simd<i16, L2_SIZE>;
type I32xL2 = Simd<i32, L2_SIZE>;
type F32xL2 = Simd<f32, L2_SIZE>;

#[derive(Clone, Copy)]
pub struct OutputBucket(u8);

impl<T> Index<OutputBucket> for [T] {
    type Output = T;

    fn index(&self, index: OutputBucket) -> &Self::Output {
        &self[usize::from(index.0)] // TODO: compiler doesn't know if this is in bounds
    }
}

impl OutputBucket {
    pub const fn new(output_bucket: u8) -> Option<Self> {
        if output_bucket >= OUTPUT_BUCKETS as u8 {
            return None;
        }
        Some(Self(output_bucket))
    }
}

/// This is the quantised format that yukari uses.
#[repr(C, align(64))]
pub struct Network {
    // (768+60144)*1 -> L1
    feature_embeddings_threat_weights: [[i8; L1_SIZE]; 60144],
    feature_embeddings_pst_weights: [[i16; L1_SIZE]; 768],
    feature_embeddings_bias: [i16; L1_SIZE],
    // 2*L1 -> OUTPUT_BUCKETS*L2
    layer1_weights: [[[[i16; L1_SIZE]; 2]; L2_SIZE]; OUTPUT_BUCKETS],
    layer1_bias: [[i16; L2_SIZE]; OUTPUT_BUCKETS],
    // L2 -> OUTPUT_BUCKETS*1
    layer2_weights: [[f32; L2_SIZE]; OUTPUT_BUCKETS],
    layer2_bias: [f32; OUTPUT_BUCKETS],
}

static NNUE: Network =
    unsafe { std::mem::transmute::<[u8; std::mem::size_of::<Network>()], Network>(*include_bytes!(env!("EVALFILE"))) };

impl Network {
    #[inline(never)]
    fn forward_l1_half(&self, us: &Accumulator, output_bucket: OutputBucket, index: usize) -> I32xL2 {
        let mut layer1 = [0_i32; L2_SIZE];

        // Dot product weights with screlu-activated input accumulator.
        for lane in 0..L2_SIZE {
            const N: usize = 64;
            let (us, []) = us.vals.as_chunks::<N>() else { unreachable!() };
            let (weights, []) = self.layer1_weights[output_bucket][lane][index].as_chunks::<N>() else { unreachable!() };
            let mut acc = Simd::<i32, N>::splat(0);
            for (us, weights) in us.iter().zip(weights) {
                const ZERO_VEC: Simd<i16, N> = Simd::splat(0);
                const QA_VEC: Simd<i16, N> = Simd::splat(QA);
                let us = Simd::from_array(*us).simd_clamp(ZERO_VEC, QA_VEC);
                let weights = Simd::from_array(*weights);
                let weights = us * weights;
                acc += us.cast::<i32>() * weights.cast::<i32>();
            }
            layer1[lane] = acc.reduce_sum();
        }

        I32xL2::from_array(layer1)
    }

    #[inline]
    fn forward_l1(&self, us: &Accumulator, them: &Accumulator, output_bucket: OutputBucket) -> F32xL2 {
        let mut layer1 = I32xL2::splat(0);

        // L1: Matrix multiply weights with screlu-activated input accumulator.
        layer1 += self.forward_l1_half(us, output_bucket, 0);
        layer1 += self.forward_l1_half(them, output_bucket, 1);

        // L1: Remove extra QA introduced by screlu.
        layer1 /= I32xL2::splat(QA.into());

        // L1: Add bias
        // TODO: compiler doesn't know if `output_bucket` is in bounds.
        layer1 += I16xL2::from_array(self.layer1_bias[output_bucket]).cast::<i32>();

        // L1: Cast to f32 and remove quantisation.
        layer1.cast::<f32>() / F32xL2::splat((QA * QB).into())
    }

    #[inline]
    fn forward_l2(&self, layer1: F32xL2, output_bucket: OutputBucket) -> f32 {
        // L2: Matrix multiply weights with crelu-activated L1.
        let layer1 = layer1.simd_clamp(F32xL2::splat(0.0), F32xL2::splat(1.0));
        let weight = F32xL2::from_array(self.layer2_weights[output_bucket]);
        let layer2 = (layer1 * weight).reduce_sum();

        // L2: Add bias
        layer2 + self.layer2_bias[output_bucket]
    }

    /// Calculates the output of the network, starting from the already
    /// calculated hidden layer (done efficiently during makemoves).
    #[inline]
    pub fn evaluate(&self, us: &Accumulator, them: &Accumulator, output_bucket: OutputBucket) -> i32 {
        let layer1 = self.forward_l1(us, them, output_bucket);
        let mut layer2 = self.forward_l2(layer1, output_bucket);

        // Apply eval scale.
        layer2 *= SCALE as f32;
        layer2 as i32
    }
}

/// A column of the feature-weights matrix.
/// Note the `align(64)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C, align(64))]
pub struct Accumulator {
    vals: [i16; L1_SIZE],
}

impl Accumulator {
    /// Initialised with bias so we can just efficiently
    /// operate on it afterwards.
    pub const fn new(net: &Network) -> Self {
        Self { vals: net.feature_embeddings_bias }
    }

    /// Add a feature to an accumulator.
    #[inline(never)]
    pub fn add_feature(&mut self, feature_idx: usize, net: &Network) {
        let (acc_chunks, []) = self.vals.as_chunks_mut::<32>() else { unreachable!() };
        if feature_idx < 768 {
            let (weight_chunks, []) = net.feature_embeddings_pst_weights[feature_idx].as_chunks::<32>() else {
                unreachable!()
            };
            for (i, d) in acc_chunks.iter_mut().zip(weight_chunks.iter()) {
                *i = (i16x32::from_array(*i) + i16x32::from_array(*d)).to_array();
            }
        } else {
            let feature_idx = feature_idx - 768;
            let (weight_chunks, []) = net.feature_embeddings_threat_weights[feature_idx].as_chunks::<32>() else {
                unreachable!()
            };
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
            let (weight_chunks, []) = net.feature_embeddings_pst_weights[feature_idx].as_chunks::<32>() else {
                unreachable!()
            };
            for (i, d) in acc_chunks.iter_mut().zip(weight_chunks.iter()) {
                *i = (i16x32::from_array(*i) - i16x32::from_array(*d)).to_array();
            }
        } else {
            let feature_idx = feature_idx - 768;
            let (weight_chunks, []) = net.feature_embeddings_threat_weights[feature_idx].as_chunks::<32>() else {
                unreachable!()
            };
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

    pub fn get(&self, piece_count: u8, colour: Colour) -> i32 {
        let output_bucket = OutputBucket::new((piece_count - 2) / DIVISOR).unwrap();
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
