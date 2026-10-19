//! Text utilities shared by parsing and rules.

pub mod entropy;
pub mod levenshtein;
pub mod lines;
pub mod normalize;
pub mod shingles;
pub mod tokens;
pub mod unicode;

pub use entropy::shannon_entropy;
pub use levenshtein::levenshtein_within;
pub use lines::LineIndex;
pub use normalize::{Normalized, normalize};
