//! Tiny shell-word scanner shared by AI-SEC-008/009: splits a command into pipelines, stages
//! and words, tracking quotes and `$VAR` / `${…}` / `$(…)` / backtick / `<(…)` expansions.
//! Not a parser: no here-docs, no control-flow semantics, no alias or glob expansion.

use std::borrow::Cow;

use crate::lexicon::shell::WRAPPERS;

/// What an [`Expansion`] expands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpKind {
    /// `$NAME`, `${NAME…}`, `$1`.
    Var,
    /// `$(…)` or a backtick substitution.
    Command,
    /// `<(…)` or `>(…)`.
    Process,
}

/// One expansion inside a word.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expansion<'a> {
    pub kind: ExpKind,
    /// Variable name, or the inner command text.
    pub inner: &'a str,
    /// Byte offset of `inner` in the scanned text.
    pub inner_start: usize,
    /// Byte range of the whole expansion in the scanned text.
    pub start: usize,
    pub end: usize,
    /// Inside double quotes (no word splitting or globbing).
    pub quoted: bool,
}

/// One shell word, quotes included.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Word<'a> {
    pub raw: &'a str,
    pub start: usize,
    pub end: usize,
    pub expansions: Vec<Expansion<'a>>,
}

impl<'a> Word<'a> {
    /// The word with quotes and backslashes removed (expansions kept as written).
    pub fn literal(&self) -> Cow<'a, str> {
        if !self.raw.contains(['\'', '"', '\\']) {
            return Cow::Borrowed(self.raw);
        }
        let (mut out, mut sq, mut dq) = (String::new(), false, false);
        let mut chars = self.raw.chars();
        while let Some(c) = chars.next() {
            match c {
                '\'' if !dq => sq = !sq,
                '"' if !sq => dq = !dq,
                '\\' if !sq => out.extend(chars.next()),
                _ => out.push(c),
            }
        }
        Cow::Owned(out)
    }

    /// `NAME` of a `NAME=value` assignment word.
    pub fn assignment(&self) -> Option<&'a str> {
        let name = &self.raw[..self.raw.find('=')?];
        let mut bytes = name.bytes();
        let first = bytes.next()?;
        let ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
        (!first.is_ascii_digit() && ident(first) && bytes.all(ident)).then_some(name)
    }
}

/// Commands joined by `|`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pipeline<'a> {
    pub stages: Vec<Vec<Word<'a>>>,
}

/// The pipelines of `src[from..to]`; offsets are relative to `src`.
pub fn scan(src: &str, from: usize, to: usize) -> Vec<Pipeline<'_>> {
    let mut lx = Lexer {
        src,
        b: src.as_bytes(),
        end: to.min(src.len()),
        out: Vec::new(),
        stages: Vec::new(),
        stage: Vec::new(),
        word: None,
        exps: Vec::new(),
    };
    lx.run(from);
    lx.out
}

/// Index and lowercase basename of a stage's program, skipping assignments and wrappers
/// (`sudo -E`, `env X=1`).
pub fn program(stage: &[Word<'_>]) -> Option<(usize, String)> {
    let mut wrapped = false;
    for (i, w) in stage.iter().enumerate() {
        if w.assignment().is_some() {
            continue;
        }
        let lit = w.literal();
        if wrapped && lit.starts_with('-') {
            continue;
        }
        let base = lit.rsplit(['/', '\\']).next().unwrap_or("");
        let name = base
            .strip_suffix(".exe")
            .unwrap_or(base)
            .to_ascii_lowercase();
        if WRAPPERS.contains(&name.as_str()) {
            wrapped = true;
            continue;
        }
        return Some((i, name));
    }
    None
}

struct Lexer<'a> {
    src: &'a str,
    b: &'a [u8],
    end: usize,
    out: Vec<Pipeline<'a>>,
    stages: Vec<Vec<Word<'a>>>,
    stage: Vec<Word<'a>>,
    word: Option<usize>,
    exps: Vec<Expansion<'a>>,
}

impl<'a> Lexer<'a> {
    fn at(&self, i: usize) -> Option<u8> {
        (i < self.end).then(|| self.b[i])
    }

    fn slice(&self, s: usize, e: usize) -> &'a str {
        self.src.get(s..e.min(self.end)).unwrap_or("")
    }

    fn start_word(&mut self, i: usize) {
        self.word.get_or_insert(i);
    }

    fn end_word(&mut self, i: usize) {
        if let Some(s) = self.word.take() {
            self.stage.push(Word {
                raw: self.slice(s, i),
                start: s,
                end: i,
                expansions: std::mem::take(&mut self.exps),
            });
        }
    }

    fn end_stage(&mut self, i: usize) {
        self.end_word(i);
        if !self.stage.is_empty() {
            self.stages.push(std::mem::take(&mut self.stage));
        }
    }

    fn end_pipeline(&mut self, i: usize) {
        self.end_stage(i);
        if !self.stages.is_empty() {
            let stages = std::mem::take(&mut self.stages);
            self.out.push(Pipeline { stages });
        }
    }

    fn run(&mut self, mut i: usize) {
        let mut dq = false;
        while let Some(c) = self.at(i) {
            if dq {
                i = match c {
                    b'"' => {
                        dq = false;
                        i + 1
                    }
                    b'\\' => i + 2,
                    b'$' | b'`' => self.expansion(i, true),
                    _ => i + 1,
                };
                continue;
            }
            let next = self.at(i + 1);
            i = match c {
                b' ' | b'\t' | b'\r' => {
                    self.end_word(i);
                    i + 1
                }
                b'\n' => {
                    self.end_pipeline(i);
                    i + 1
                }
                b'\\' if next == Some(b'\n') => {
                    self.end_word(i);
                    i + 2
                }
                b'#' if self.word.is_none() => self.skip_to(i, b'\n'),
                b'\'' => {
                    self.start_word(i);
                    self.skip_to(i + 1, b'\'') + 1
                }
                b'"' => {
                    self.start_word(i);
                    dq = true;
                    i + 1
                }
                b'\\' => {
                    self.start_word(i);
                    i + 2
                }
                b'|' if next == Some(b'|') => {
                    self.end_pipeline(i);
                    i + 2
                }
                b'|' => {
                    self.end_stage(i);
                    if next == Some(b'&') { i + 2 } else { i + 1 }
                }
                b'&' if next == Some(b'>') => {
                    self.end_word(i);
                    self.skip_redirect(i + 1)
                }
                b'&' => {
                    self.end_pipeline(i);
                    if next == Some(b'&') { i + 2 } else { i + 1 }
                }
                b';' => {
                    self.end_pipeline(i);
                    i + 1
                }
                b'<' | b'>' if next == Some(b'(') => {
                    self.start_word(i);
                    self.substitution(i, i + 2, ExpKind::Process, false)
                }
                b'<' | b'>' => {
                    self.end_word(i);
                    self.skip_redirect(i)
                }
                b'(' | b')' => {
                    self.end_word(i);
                    i + 1
                }
                b'$' | b'`' => {
                    self.start_word(i);
                    self.expansion(i, false)
                }
                _ => {
                    self.start_word(i);
                    i + 1
                }
            };
        }
        self.end_pipeline(self.end);
    }

    /// Index of the next `stop` at or after `i`, or the end.
    fn skip_to(&self, i: usize, stop: u8) -> usize {
        let rest = self.b.get(i..self.end).unwrap_or(&[]);
        memchr::memchr(stop, rest).map_or(self.end, |j| i + j)
    }

    /// Past a redirection operator (`>`, `>>`, `>&`, `<&`, `>|`).
    fn skip_redirect(&self, mut i: usize) -> usize {
        while matches!(self.at(i), Some(b'<' | b'>' | b'&' | b'|')) {
            i += 1;
        }
        i
    }

    /// Records the expansion starting at `$` or a backtick at `i`; returns the index after it.
    fn expansion(&mut self, i: usize, quoted: bool) -> usize {
        if self.at(i) == Some(b'`') {
            let mut j = i + 1;
            while let Some(c) = self.at(j) {
                match c {
                    b'\\' => j += 2,
                    b'`' => break,
                    _ => j += 1,
                }
            }
            let close = j.min(self.end);
            self.push(ExpKind::Command, i, i + 1, close, close + 1, quoted);
            return close + 1;
        }
        match self.at(i + 1) {
            Some(b'(') => self.substitution(i, i + 2, ExpKind::Command, quoted),
            Some(b'{') => {
                let close = self.skip_to(i + 2, b'}');
                let body = self.slice(i + 2, close);
                let skip = body.len() - body.trim_start_matches(['#', '!']).len();
                let len = body[skip..]
                    .bytes()
                    .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
                    .count();
                let name = i + 2 + skip;
                self.push(ExpKind::Var, i, name, name + len, close + 1, quoted);
                close + 1
            }
            Some(c) if c.is_ascii_alphabetic() || c == b'_' => {
                let mut j = i + 1;
                while self
                    .at(j)
                    .is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_')
                {
                    j += 1;
                }
                self.push(ExpKind::Var, i, i + 1, j, j, quoted);
                j
            }
            Some(c) if c.is_ascii_digit() || b"@*#?$!-".contains(&c) => {
                self.push(ExpKind::Var, i, i + 1, i + 2, i + 2, quoted);
                i + 2
            }
            _ => i + 1,
        }
    }

    /// Records a `$(…)` / `<(…)` whose body starts at `body`; returns the index after `)`.
    fn substitution(&mut self, i: usize, body: usize, kind: ExpKind, quoted: bool) -> usize {
        let close = self.close_paren(body);
        self.push(kind, i, body, close, close + 1, quoted);
        close + 1
    }

    /// Index of the `)` matching an already-open `(`, skipping quotes; the end if unclosed.
    fn close_paren(&self, mut i: usize) -> usize {
        let mut depth = 1u32;
        while let Some(c) = self.at(i) {
            match c {
                b'\\' => i += 1,
                b'\'' => i = self.skip_to(i + 1, b'\''),
                b'"' => {
                    i += 1;
                    while let Some(d) = self.at(i) {
                        match d {
                            b'\\' => i += 1,
                            b'"' => break,
                            _ => {}
                        }
                        i += 1;
                    }
                }
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return i;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        self.end
    }

    fn push(
        &mut self,
        kind: ExpKind,
        start: usize,
        inner: usize,
        inner_end: usize,
        end: usize,
        quoted: bool,
    ) {
        let end = end.min(self.end);
        self.exps.push(Expansion {
            kind,
            inner: self.slice(inner, inner_end),
            inner_start: inner,
            start,
            end,
            quoted,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(src: &str) -> Vec<Vec<Vec<String>>> {
        scan(src, 0, src.len())
            .iter()
            .map(|p| {
                p.stages
                    .iter()
                    .map(|s| s.iter().map(|w| w.raw.to_owned()).collect())
                    .collect()
            })
            .collect()
    }

    #[test]
    fn scan_splits_pipelines_stages_and_words() {
        assert_eq!(
            words("curl -fsSL 'a b' | sudo bash && echo \"x | y\"; ls 2>&1"),
            [
                vec![vec!["curl", "-fsSL", "'a b'"], vec!["sudo", "bash"]],
                vec![vec!["echo", "\"x | y\""]],
                vec![vec!["ls", "2", "1"]],
            ]
        );
        assert_eq!(
            words("a || b # c | d\ne"),
            [vec![vec!["a"]], vec![vec!["b"]], vec![vec!["e"]]]
        );
    }

    #[test]
    fn scan_records_expansions_with_quoting() {
        let src = r#"x=$(jq -r '.a)b') "$Y" ${Z:-1} `w` bash <(curl u) '$NO'"#;
        let p = scan(src, 0, src.len());
        let exps: Vec<_> = p[0].stages[0]
            .iter()
            .flat_map(|w| &w.expansions)
            .map(|e| (e.kind, e.inner, e.quoted, &src[e.start..e.end]))
            .collect();
        assert_eq!(
            exps,
            [
                (ExpKind::Command, "jq -r '.a)b'", false, "$(jq -r '.a)b')"),
                (ExpKind::Var, "Y", true, "$Y"),
                (ExpKind::Var, "Z", false, "${Z:-1}"),
                (ExpKind::Command, "w", false, "`w`"),
                (ExpKind::Process, "curl u", false, "<(curl u)"),
            ]
        );
    }

    #[test]
    fn scan_handles_unclosed_input() {
        for src in [
            "echo \"abc",
            "echo $(curl",
            "echo `x",
            "echo ${",
            "echo 'x",
            "a \\",
        ] {
            let _ = scan(src, 0, src.len());
        }
    }

    #[test]
    fn program_skips_assignments_and_wrappers() {
        let p = scan("X=1 sudo -E /usr/bin/Bash -s", 0, 28);
        assert_eq!(program(&p[0].stages[0]), Some((3, "bash".to_owned())));
        let p = scan("A=b", 0, 3);
        assert_eq!(program(&p[0].stages[0]), None);
    }

    #[test]
    fn literal_strips_quotes_and_assignment_needs_identifier() {
        let w = |s: &'static str| scan(s, 0, s.len())[0].stages[0][0].clone();
        assert_eq!(w(r#"'a'"b"\c"#).literal(), "abc");
        assert_eq!(w("f=$(x)").assignment(), Some("f"));
        assert_eq!(w("--opt=1").assignment(), None);
        assert_eq!(w("1a=2").assignment(), None);
    }
}
