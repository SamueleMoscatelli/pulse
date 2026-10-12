//! Text utilities shared by parsing and rules.

pub mod lines;
pub mod normalize;
pub mod tokens;
pub mod unicode;

pub use lines::LineIndex;
pub use normalize::{Normalized, normalize};
