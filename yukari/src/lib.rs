#![cfg_attr(target_arch = "aarch64", feature(stdarch_aarch64_prefetch))]
#![warn(clippy::imprecise_flops, clippy::suboptimal_flops)]

pub mod datagen;
pub mod engine;
pub mod output;
mod search;

pub use search::{Search, SearchParams, TtEntry, is_repetition_draw};
