//! Text utilities shared by parsing and rules.

pub mod entropy;
pub mod lines;
pub mod normalize;
pub mod tokens;
pub mod unicode;

pub use entropy::shannon_entropy;
pub use lines::LineIndex;
pub use normalize::{Normalized, normalize};
