#![feature(stdio_locked)]
#![warn(clippy::imprecise_flops, clippy::suboptimal_flops)]

mod eval;
mod search;
mod tune;
pub mod proto;
pub mod engine;

pub use search::Search;
pub use search::is_repetition_draw;
pub use tune::Tune;