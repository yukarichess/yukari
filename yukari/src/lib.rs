#![warn(clippy::imprecise_flops, clippy::suboptimal_flops)]

mod eval;
mod search;
pub mod engine;

pub use search::Search;
pub use search::is_repetition_draw;
