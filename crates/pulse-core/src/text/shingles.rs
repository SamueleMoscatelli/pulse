//! Near-duplicate detection over normalized text (ADR-029): words → xxh3, 5-word shingles,
//! and a one-permutation MinHash signature whose agreement estimates the shingle-set Jaccard.

use xxhash_rust::xxh3::xxh3_64;

/// Words per shingle.
pub const SHINGLE: usize = 5;
/// Bins of a signature (a power of two).
pub const SIG_LEN: usize = 32;
/// Hash bits selecting a bin.
const BIN_BITS: u32 = SIG_LEN.trailing_zeros();

/// A MinHash signature: per bin, the minimum hash of the shingles falling in it.
pub type Signature = [u32; SIG_LEN];

/// Appends the xxh3 hash of every word of `text` to `out`.
///
/// `text` is expected to be normalized (lowercased, see [`super::normalize`]). A word is a
/// run of ASCII alphanumerics or non-ASCII bytes, so punctuation and Markdown syntax
/// separate words and never belong to one. No allocation besides `out`'s growth.
pub fn word_hashes(text: &str, out: &mut Vec<u64>) {
    for_each_word(text, |_, h| out.push(h));
}

/// Like [`word_hashes`], also appending each word's byte offset in `text` to `starts`.
pub fn word_hashes_at(text: &str, starts: &mut Vec<u32>, out: &mut Vec<u64>) {
    for_each_word(text, |start, h| {
        starts.push(u32::try_from(start).unwrap_or(u32::MAX));
        out.push(h);
    });
}

fn for_each_word(text: &str, mut f: impl FnMut(usize, u64)) {
    let bytes = text.as_bytes();
    let mut start = None;
    for (i, &b) in bytes.iter().enumerate() {
        let word = b.is_ascii_alphanumeric() || !b.is_ascii();
        match (word, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                f(s, xxh3_64(&bytes[s..i]));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        f(s, xxh3_64(&bytes[s..]));
    }
}

/// One-permutation MinHash of the [`SHINGLE`]-word shingles of `words` (one shingle of all
/// words when there are fewer): each shingle hash picks a bin by its top bits and keeps the
/// bin minimum. Empty bins borrow a filled bin chosen by a fixed hash sequence (optimal
/// densification), so equal bins stay an unbiased Jaccard estimate. All `u32::MAX` when
/// `words` is empty.
pub fn minhash(words: &[u64]) -> Signature {
    let mut sig = [u32::MAX; SIG_LEN];
    if words.is_empty() {
        return sig;
    }
    let mut filled = 0u64;
    for w in words.windows(SHINGLE.min(words.len())) {
        let h = shingle_hash(w);
        let bin = (h >> (64 - BIN_BITS)) as usize;
        sig[bin] = sig[bin].min(h as u32);
        filled |= 1 << bin;
    }
    let source = sig;
    for (i, slot) in sig.iter_mut().enumerate() {
        if filled & (1 << i) != 0 {
            continue;
        }
        // At least one bin is filled; the bound only keeps the loop finite on paper.
        let mut j = filled.trailing_zeros() as usize;
        for attempt in 1..=256u64 {
            let k = (mix(((i as u64) << 32) | attempt) >> (64 - BIN_BITS)) as usize;
            if filled & (1 << k) != 0 {
                j = k;
                break;
            }
        }
        *slot = source[j];
    }
    sig
}

/// How many bins two signatures agree on; `agreement / SIG_LEN` estimates Jaccard.
pub fn agreement(a: &Signature, b: &Signature) -> usize {
    a.iter().zip(b).filter(|(x, y)| x == y).count()
}

/// Order-sensitive hash of a whole word sequence (exact-match detection).
pub fn sequence_hash(words: &[u64]) -> u64 {
    mix(words
        .iter()
        .fold(words.len() as u64, |h, &w| h.rotate_left(23) ^ mix(w)))
}

/// Order-sensitive combination of up to [`SHINGLE`] word hashes.
fn shingle_hash(words: &[u64]) -> u64 {
    mix(words.iter().fold(0, |h, &w| h.rotate_left(17) ^ w))
}

/// The murmur3 64-bit finalizer: a cheap, well-mixed bijection.
fn mix(mut x: u64) -> u64 {
    x ^= x >> 33;
    x = x.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    x ^= x >> 33;
    x = x.wrapping_mul(0xC4CE_B9FE_1A85_EC53);
    x ^ (x >> 33)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &str) -> Vec<u64> {
        let mut w = Vec::new();
        word_hashes(text, &mut w);
        w
    }

    fn sig(text: &str) -> Signature {
        minhash(&words(text))
    }

    const PARA: &str = "always run the full test suite before you open a pull request, and \
        never push directly to the main branch; ask for a review from the owning team when \
        the change touches the public api or the database schema";

    #[test]
    fn words_split_on_punctuation() {
        assert_eq!(
            words("run `cargo test`, then: **review**"),
            words("run cargo test then review")
        );
        assert_eq!(words("run cargo test then review").len(), 5);
        assert_eq!(words("café über").len(), 2);
        let (mut starts, mut hashes) = (Vec::new(), Vec::new());
        word_hashes_at("a, bc  d", &mut starts, &mut hashes);
        assert_eq!(starts, [0, 3, 7]);
        assert_eq!(hashes, words("a bc d"));
    }

    #[test]
    fn identical_words_agree_fully() {
        assert_eq!(
            agreement(&sig(PARA), &sig(&PARA.replace(", ", " "))),
            SIG_LEN
        );
    }

    #[test]
    fn light_rewording_agrees_mostly_and_other_text_barely() {
        for reworded in [
            PARA.replace("always run", "always execute"),
            PARA.replace("full", "complete")
                .replace("owning", "responsible"),
        ] {
            let a = agreement(&sig(PARA), &sig(&reworded));
            assert!(a >= SIG_LEN / 2, "{a}: {reworded}");
        }
        let other = "the database schema lives in migrations; write a new migration for every \
            change and keep each one reversible, with a down step that restores the previous \
            state and a test that applies it twice";
        assert!(agreement(&sig(PARA), &sig(other)) <= 4);
    }

    #[test]
    fn short_and_empty_inputs() {
        assert_eq!(minhash(&[]), [u32::MAX; SIG_LEN]);
        // One shingle fills one bin; densification copies it everywhere.
        let s = sig("two words");
        assert!(s.iter().all(|&v| v == s[0]));
    }

    #[test]
    fn sequence_hash_is_order_sensitive() {
        assert_ne!(sequence_hash(&[1, 2]), sequence_hash(&[2, 1]));
        assert_eq!(sequence_hash(&[1, 2]), sequence_hash(&[1, 2]));
        assert_ne!(sequence_hash(&[0]), sequence_hash(&[0, 0]));
    }
}
