//! The findings baseline (spec 04-scoring-gate → Fingerprints).

use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};

use pulse_core::Finding;
use serde::{Deserialize, Serialize};

/// Default baseline location, relative to the scan root.
pub const BASELINE_FILE: &str = ".pulse/baseline.json";

/// The only baseline format version this build reads and writes.
const VERSION: u32 = 1;

/// Error reading or writing a baseline file.
#[derive(Debug, thiserror::Error)]
pub enum BaselineError {
    #[error("cannot read {}: {source}", path.display())]
    Read { path: PathBuf, source: io::Error },
    #[error("cannot write {}: {source}", path.display())]
    Write { path: PathBuf, source: io::Error },
    #[error("{}: {message}", path.display())]
    Invalid { path: PathBuf, message: String },
}

/// Fingerprints of accepted findings; anything else is new.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Baseline {
    fingerprints: BTreeSet<u64>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    version: u32,
    fingerprints: Vec<String>,
}

impl Baseline {
    /// The baseline of `findings`.
    pub fn from_findings(findings: &[Finding]) -> Self {
        Self {
            fingerprints: findings.iter().map(|f| f.fingerprint).collect(),
        }
    }

    /// Whether `f` is not in the baseline.
    pub fn is_new(&self, f: &Finding) -> bool {
        !self.fingerprints.contains(&f.fingerprint)
    }

    pub fn len(&self) -> usize {
        self.fingerprints.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fingerprints.is_empty()
    }

    /// Reads the baseline at `path`; `Ok(None)` when the file does not exist.
    pub fn load(path: &Path) -> Result<Option<Self>, BaselineError> {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::parse(&text, path).map(Some),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(BaselineError::Read {
                path: path.into(),
                source,
            }),
        }
    }

    /// Parses baseline JSON; `path` is only used in errors.
    pub fn parse(text: &str, path: &Path) -> Result<Self, BaselineError> {
        let invalid = |message: String| BaselineError::Invalid {
            path: path.into(),
            message,
        };
        let file: File = serde_json::from_str(text).map_err(|e| invalid(e.to_string()))?;
        if file.version != VERSION {
            return Err(invalid(format!(
                "unsupported baseline version {} (expected {VERSION}; regenerate it with `pulse baseline`)",
                file.version
            )));
        }
        let fingerprints = file
            .fingerprints
            .iter()
            .map(|hex| {
                if hex.len() == 16 {
                    u64::from_str_radix(hex, 16).ok()
                } else {
                    None
                }
                .ok_or_else(|| {
                    invalid(format!(
                        "invalid fingerprint `{hex}` (expected 16 hex digits)"
                    ))
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { fingerprints })
    }

    /// The baseline as pretty-printed JSON with sorted fingerprints and a trailing newline.
    pub fn to_json(&self) -> String {
        let file = File {
            version: VERSION,
            fingerprints: self
                .fingerprints
                .iter()
                .map(|f| format!("{f:016x}"))
                .collect(),
        };
        // Serializing strings and integers cannot fail.
        let mut out = serde_json::to_string_pretty(&file).unwrap_or_default();
        out.push('\n');
        out
    }

    /// Writes the baseline to `path`, creating parent directories.
    pub fn write(&self, path: &Path) -> Result<(), BaselineError> {
        let write = || -> io::Result<()> {
            if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(path, self.to_json())
        };
        write().map_err(|source| BaselineError::Write {
            path: path.into(),
            source,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finding(fp: u64) -> Finding {
        Finding {
            rule_id: "AI-STR-002",
            severity: pulse_core::Severity::Low,
            location: pulse_core::Location {
                path: std::sync::Arc::from(Path::new("a.md")),
                line: 1,
                col: 1,
                end_line: 1,
                end_col: 2,
            },
            message: String::new(),
            remediation: None,
            token_waste: 0,
            fingerprint: fp,
        }
    }

    #[test]
    fn round_trips_sorted_hex() {
        let b = Baseline::from_findings(&[finding(0xff), finding(1), finding(0xff)]);
        let json = b.to_json();
        assert_eq!(
            json,
            "{\n  \"version\": 1,\n  \"fingerprints\": [\n    \"0000000000000001\",\n    \"00000000000000ff\"\n  ]\n}\n"
        );
        assert_eq!(Baseline::parse(&json, Path::new("b")).unwrap(), b);
        assert!(!b.is_new(&finding(1)));
        assert!(b.is_new(&finding(2)));
    }

    #[test]
    fn rejects_other_versions_and_bad_fingerprints() {
        let p = Path::new("b.json");
        let err = Baseline::parse(r#"{"version":2,"fingerprints":[]}"#, p).unwrap_err();
        assert!(
            err.to_string().contains("unsupported baseline version 2"),
            "{err}"
        );
        let err = Baseline::parse(r#"{"version":1,"fingerprints":["xyz"]}"#, p).unwrap_err();
        assert!(
            err.to_string().contains("invalid fingerprint `xyz`"),
            "{err}"
        );
        assert!(Baseline::parse("not json", p).is_err());
    }

    #[test]
    fn load_missing_is_none_and_write_creates_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(BASELINE_FILE);
        assert_eq!(Baseline::load(&path).unwrap(), None);
        let b = Baseline::from_findings(&[finding(7)]);
        b.write(&path).unwrap();
        assert_eq!(Baseline::load(&path).unwrap(), Some(b));
    }
}
