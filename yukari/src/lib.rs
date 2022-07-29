#![warn(clippy::imprecise_flops, clippy::suboptimal_flops)]

pub mod engine;
mod eval;
mod search;

pub use search::is_repetition_draw;
pub use search::Search;
