//! MCP config word lists: credential key names and package-launcher flags.

use std::sync::LazyLock;

use regex::Regex;

/// AI-SEC-004: env/header keys and `--flag` names that hold a credential.
#[allow(clippy::expect_used)] // static pattern (rust.md)
pub static CREDENTIAL_KEY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)key|token|secret|password|auth|bearer").expect("static pattern")
});

/// AI-SEC-004: key suffixes that name where a credential lives or how it is used
/// (`API_KEY_FILE`, `AUTH_URL`), not the credential itself.
pub const NON_SECRET_KEY_SUFFIXES: &[&str] = &[
    "file", "path", "dir", "url", "uri", "type", "enabled", "mode",
];

/// npm-family launchers (`npx`, `bunx`, `pnpm dlx`, `yarn dlx`): flags that take a value.
pub const NPM_VALUE_FLAGS: &[&str] = &[
    "-p",
    "--package",
    "-c",
    "--call",
    "--registry",
    "--cache",
    "--userconfig",
];

/// npm-family flags whose value is the package to install.
pub const NPM_PACKAGE_FLAGS: &[&str] = &["-p", "--package"];

/// npm-family flags that skip the install prompt.
pub const NPM_AUTO_INSTALL_FLAGS: &[&str] = &["-y", "--yes"];

/// Python launchers (`uvx`, `uv tool run`, `pipx run`): flags that take a value.
pub const PY_VALUE_FLAGS: &[&str] = &[
    "--from",
    "--spec",
    "--with",
    "--with-editable",
    "--with-requirements",
    "--python",
    "-p",
    "--index",
    "--index-url",
    "-i",
    "--extra-index-url",
    "--default-index",
    "--pip-args",
    "--directory",
    "--project",
    "--config-file",
    "--cache-dir",
];

/// Python launcher flags whose value is the package to install.
pub const PY_PACKAGE_FLAGS: &[&str] = &["--from", "--spec"];

/// `docker run` flags that take a value (boolean flags such as `-i`, `--rm` don't).
pub const DOCKER_VALUE_FLAGS: &[&str] = &[
    "-a",
    "--attach",
    "--add-host",
    "--cap-add",
    "--cap-drop",
    "--cgroup-parent",
    "--cidfile",
    "--cpus",
    "--cpu-shares",
    "--device",
    "--dns",
    "-e",
    "--env",
    "--env-file",
    "--entrypoint",
    "--expose",
    "--gpus",
    "--group-add",
    "-h",
    "--hostname",
    "--ipc",
    "-l",
    "--label",
    "--link",
    "--log-driver",
    "--log-opt",
    "-m",
    "--memory",
    "--mount",
    "--name",
    "--network",
    "--net",
    "-p",
    "--publish",
    "--pid",
    "--platform",
    "--pull",
    "--restart",
    "--runtime",
    "--security-opt",
    "--shm-size",
    "--stop-signal",
    "--tmpfs",
    "-u",
    "--user",
    "--ulimit",
    "-v",
    "--volume",
    "--volumes-from",
    "-w",
    "--workdir",
];

/// AI-SEC-006: hosts that never leave the machine.
pub const LOOPBACK_HOSTS: &[&str] = &["localhost", "127.0.0.1", "::1"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_key_matches_spec_words() {
        for k in [
            "GITHUB_TOKEN",
            "apiKey",
            "Authorization",
            "DB_PASSWORD",
            "x-secret",
        ] {
            assert!(CREDENTIAL_KEY.is_match(k), "{k}");
        }
        for k in ["LOG_LEVEL", "PORT", "REGION"] {
            assert!(!CREDENTIAL_KEY.is_match(k), "{k}");
        }
    }
}
