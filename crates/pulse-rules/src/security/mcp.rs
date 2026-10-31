//! MCP config security: AI-SEC-004 literal credentials, AI-SEC-005 unpinned server packages,
//! AI-SEC-006 insecure transport. Kiro JSON agents' inline `mcpServers` are checked too.

use std::net::Ipv4Addr;

use pulse_core::parse::Transport;
use pulse_core::{Asset, Category, Finding, KindSet, Rule, RuleCtx, RuleMeta, Severity, Span};

use crate::lexicon::mcp::{
    CREDENTIAL_KEY, DOCKER_VALUE_FLAGS, LOOPBACK_HOSTS, NON_SECRET_KEY_SUFFIXES,
    NPM_AUTO_INSTALL_FLAGS, NPM_PACKAGE_FLAGS, NPM_VALUE_FLAGS, PY_PACKAGE_FLAGS, PY_VALUE_FLAGS,
};
use crate::lexicon::secrets::is_placeholder;
use crate::util::{json_views, truncate};

/// Chars of a name, package or URL quoted in a message.
const QUOTED: usize = 40;

pub static META_004: RuleMeta = RuleMeta {
    id: "AI-SEC-004",
    name: "MCP credential not externalized",
    category: Category::Security,
    default_severity: Severity::High,
    kinds: KindSet::MCP_CONFIG.union(KindSet::AGENT),
    summary: "An MCP server credential (env, header or CLI flag) is written as a literal value.",
    remediation: "Reference an environment variable (${env:X}) or a VS Code input with password: true.",
    inline_suppressible: true,
};

pub static META_005: RuleMeta = RuleMeta {
    id: "AI-SEC-005",
    name: "Unpinned MCP server package",
    category: Category::Security,
    default_severity: Severity::High,
    kinds: KindSet::MCP_CONFIG.union(KindSet::AGENT),
    summary: "MCP server package is not pinned to an exact version.",
    remediation: "Pin an exact version (npx pkg@1.2.3, uvx pkg==1.2.3, image@sha256:...).",
    inline_suppressible: true,
};

pub static META_006: RuleMeta = RuleMeta {
    id: "AI-SEC-006",
    name: "Insecure MCP transport",
    category: Category::Security,
    default_severity: Severity::High,
    kinds: KindSet::MCP_CONFIG.union(KindSet::AGENT),
    summary: "A remote MCP server is reached over plain http://.",
    remediation: "Use https:// (plain http is fine only for localhost).",
    inline_suppressible: true,
};

/// AI-SEC-004.
pub struct LiteralMcpCredential;

impl Rule for LiteralMcpCredential {
    fn meta(&self) -> &'static RuleMeta {
        &META_004
    }

    fn applies_to(&self, asset: &Asset) -> bool {
        json_views(&META_004, asset)
    }

    fn check(&self, asset: &Asset, _ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
        for server in &asset.mcp().servers {
            let maps = [
                ("env", &server.env),
                ("header", &server.headers),
                ("oauth", &server.oauth),
            ];
            for (what, map) in maps {
                for (key, value, span) in map {
                    if credential_key(key) && literal(value) {
                        let span = value_span(asset, *span, value, 0, value.len());
                        out.push(credential(asset, span, what, key, &server.name));
                    }
                }
            }
            let args = &server.args;
            for (i, (arg, span)) in args.iter().enumerate() {
                let prev = i.checked_sub(1).map(|p| args[p].0.as_str());
                // `docker run -e KEY=value`, `--env=KEY=value`.
                let env_pair = if matches!(prev, Some("-e" | "--env")) {
                    Some((arg.as_str(), 0))
                } else {
                    arg.strip_prefix("--env=")
                        .or_else(|| arg.strip_prefix("-e="))
                        .map(|rest| (rest, arg.len() - rest.len()))
                };
                if let Some((pair, off)) = env_pair {
                    if let Some((k, v)) = pair.split_once('=')
                        && credential_key(k)
                        && literal(v)
                    {
                        let vspan = value_span(asset, *span, arg, off + k.len() + 1, v.len());
                        out.push(credential(asset, vspan, "env", k, &server.name));
                    }
                    continue;
                }
                let Some(flag) = arg.strip_prefix('-') else {
                    continue;
                };
                let (name, inline) = match flag.split_once('=') {
                    Some((n, v)) => (n, Some(v)),
                    None => (flag, None),
                };
                if !credential_key(name) {
                    continue;
                }
                let (value, vspan) = match inline {
                    Some(v) => (
                        v,
                        value_span(asset, *span, arg, arg.len() - v.len(), v.len()),
                    ),
                    None => match args.get(i + 1) {
                        Some((v, s)) if !v.starts_with('-') => {
                            (v.as_str(), value_span(asset, *s, v, 0, v.len()))
                        }
                        _ => continue,
                    },
                };
                if literal(value) {
                    let flag = arg.split('=').next().unwrap_or(arg);
                    out.push(credential(asset, vspan, "flag", flag, &server.name));
                }
            }
        }
    }
}

/// Whether an env/header key or flag name holds a credential.
fn credential_key(key: &str) -> bool {
    // AI-SEC-004 FP guard: keys that name where a credential lives or how it is used
    // (`API_KEY_FILE`, `AUTH_URL`, `--key-file`), not the credential itself.
    CREDENTIAL_KEY.is_match(key)
        && !NON_SECRET_KEY_SUFFIXES.iter().any(|s| {
            key.len() > s.len()
                && key
                    .get(key.len() - s.len()..)
                    .is_some_and(|end| end.eq_ignore_ascii_case(s))
        })
}

/// Whether a credential value is written inline: not empty, not a reference (`${env:X}`,
/// `${input:X}`, `${X}`, `$X`, also inside e.g. `Bearer ${env:X}`), not a placeholder.
fn literal(value: &str) -> bool {
    let v = value.trim();
    if v.is_empty() || has_reference(v) || is_placeholder(v) {
        return false;
    }
    // AI-SEC-004 FP guard: switches and limits (`AUTH_ENABLED=true`, `MAX_TOKENS=4096`).
    let switch = ["true", "false", "yes", "no", "on", "off", "none"]
        .iter()
        .any(|w| v.eq_ignore_ascii_case(w));
    !(switch || v.bytes().all(|b| b.is_ascii_digit()) || path_or_url(v))
}

/// AI-SEC-004 FP guard: a file path, or a URL without userinfo (which could hold a password).
fn path_or_url(v: &str) -> bool {
    let b = v.as_bytes();
    let drive = b.len() > 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\';
    v.starts_with(['/', '~'])
        || v.starts_with("./")
        || v.starts_with("../")
        || drive
        || (v.contains("://") && !v.contains('@'))
}

/// Whether `v` contains `${…}`, or a `$NAME` token (at the start or after whitespace, so a
/// `$` inside a literal password doesn't count).
fn has_reference(v: &str) -> bool {
    let b = v.as_bytes();
    b.windows(2).enumerate().any(|(i, w)| {
        w[0] == b'$'
            && (w[1] == b'{'
                || ((w[1] == b'_' || w[1].is_ascii_alphabetic())
                    && (i == 0 || b[i - 1].is_ascii_whitespace())))
    })
}

fn credential(asset: &Asset, span: Span, what: &str, key: &str, server: &str) -> Finding {
    let message = format!(
        "Literal MCP credential in {what} \"{}\" (server \"{}\")",
        truncate(key, QUOTED),
        truncate(server, 24)
    );
    Finding::new(&META_004, asset, span, message)
}

/// AI-SEC-005.
pub struct UnpinnedMcpPackage;

impl Rule for UnpinnedMcpPackage {
    fn meta(&self) -> &'static RuleMeta {
        &META_005
    }

    fn applies_to(&self, asset: &Asset) -> bool {
        json_views(&META_005, asset)
    }

    fn check(&self, asset: &Asset, _ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
        for server in &asset.mcp().servers {
            let Transport::Stdio { command } = &server.transport else {
                continue;
            };
            let words: Vec<&str> = server.args.iter().map(|(a, _)| a.as_str()).collect();
            let Some((eco, skip)) = launcher(&command.0, &words) else {
                continue;
            };
            let Some(launch) = package_args(eco, &words[skip..]) else {
                continue;
            };
            for (i, off) in launch.packages {
                let (arg, span) = &server.args[skip + i];
                let spec = &arg[off..];
                if pinned(eco, spec) != Some(false) {
                    continue;
                }
                let what = if eco == Eco::Docker {
                    "image"
                } else {
                    "package"
                };
                let auto = if launch.auto_install {
                    " (auto-install)"
                } else {
                    ""
                };
                let message = format!("Unpinned MCP {what} \"{}\"{auto}", truncate(spec, QUOTED));
                let span = value_span(asset, *span, arg, off, spec.len());
                out.push(Finding::new(&META_005, asset, span, message));
            }
        }
    }
}

/// Package ecosystem of a launcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Eco {
    Npm,
    Python,
    Docker,
}

/// The ecosystem `command` installs from, and how many leading args belong to the launcher
/// (`pnpm dlx` → 1), or `None` when `command` doesn't fetch packages.
pub fn launcher(command: &str, args: &[&str]) -> Option<(Eco, usize)> {
    let base = command.rsplit(['/', '\\']).next().unwrap_or(command);
    let base = base
        .strip_suffix(".cmd")
        .or_else(|| base.strip_suffix(".exe"))
        .unwrap_or(base);
    let first = args.first().copied();
    match (base, first) {
        ("npx" | "bunx", _) => Some((Eco::Npm, 0)),
        ("pnpm" | "yarn", Some("dlx")) => Some((Eco::Npm, 1)),
        ("uvx", _) => Some((Eco::Python, 0)),
        ("uv", Some("tool")) if args.get(1) == Some(&"run") => Some((Eco::Python, 2)),
        ("pipx", Some("run")) => Some((Eco::Python, 1)),
        ("docker", Some("run")) => Some((Eco::Docker, 1)),
        _ => None,
    }
}

/// Packages found in a launcher's args.
#[derive(Debug, PartialEq, Eq)]
pub struct Launch {
    /// `(arg index, byte offset of the spec in the arg)`.
    pub packages: Vec<(usize, usize)>,
    /// `-y`/`--yes` was given.
    pub auto_install: bool,
}

/// The package specs in `args` (after the launcher words): `--package`/`--from`/`--spec`
/// values when present, else the first positional argument.
pub fn package_args(eco: Eco, args: &[&str]) -> Option<Launch> {
    let (value_flags, package_flags): (&[&str], &[&str]) = match eco {
        Eco::Npm => (NPM_VALUE_FLAGS, NPM_PACKAGE_FLAGS),
        Eco::Python => (PY_VALUE_FLAGS, PY_PACKAGE_FLAGS),
        Eco::Docker => (DOCKER_VALUE_FLAGS, &[]),
    };
    let mut launch = Launch {
        packages: Vec::new(),
        auto_install: false,
    };
    let mut positional = None;
    let mut i = 0;
    while i < args.len() {
        let arg = args[i];
        if arg == "--" {
            i += 1;
            continue;
        }
        if !arg.starts_with('-') || arg == "-" {
            positional = Some(i);
            break;
        }
        if eco == Eco::Npm && NPM_AUTO_INSTALL_FLAGS.contains(&arg) {
            launch.auto_install = true;
        }
        match arg.split_once('=') {
            Some((name, _)) => {
                if package_flags.contains(&name) {
                    launch.packages.push((i, name.len() + 1));
                }
            }
            None if value_flags.contains(&arg) => {
                if package_flags.contains(&arg) && i + 1 < args.len() {
                    launch.packages.push((i + 1, 0));
                }
                i += 1;
            }
            None => {}
        }
        i += 1;
    }
    if launch.packages.is_empty() {
        launch.packages.push((positional?, 0));
    }
    Some(launch)
}

/// Whether `spec` pins an exact version; `None` for what isn't a registry package (local
/// paths, URLs, git and variable references).
pub fn pinned(eco: Eco, spec: &str) -> Option<bool> {
    if spec.is_empty()
        || spec.starts_with(['.', '/', '~', '$'])
        || spec.contains("://")
        || spec.contains("${")
    {
        return None;
    }
    match eco {
        Eco::Npm => npm_pinned(spec),
        Eco::Python => python_pinned(spec),
        Eco::Docker => Some(docker_pinned(spec)),
    }
}

/// `@scope/pkg@1.2.3` → pinned; no version, `@latest`, ranges and dist-tags are not.
fn npm_pinned(spec: &str) -> Option<bool> {
    // Protocols such as `github:`, `file:`, `git+ssh:`.
    if spec.contains(':') {
        return None;
    }
    let at = if let Some(rest) = spec.strip_prefix('@') {
        rest.find('@').map(|i| i + 1)
    } else {
        spec.find('@')
    };
    Some(at.is_some_and(|at| exact_semver(&spec[at + 1..])))
}

/// `MAJOR.MINOR.PATCH`, optionally followed by a `-prerelease` or `+build` suffix.
fn exact_semver(v: &str) -> bool {
    let core = v.split(['-', '+']).next().unwrap_or(v);
    let parts: Vec<&str> = core.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

/// `pkg==1.2.3` or `pkg@1.2.3` → pinned; bare names, `@latest` and ranges are not.
fn python_pinned(spec: &str) -> Option<bool> {
    if spec.starts_with("git+") {
        return None;
    }
    let version = if let Some((_, v)) = spec.split_once("==") {
        v.trim_start_matches('=')
    } else if let Some((_, v)) = spec.split_once('@') {
        // `@` takes a version, not a range: `pkg@1.2.3`, not `pkg@1` or `pkg@latest`.
        return Some(
            v.split('.').count() >= 3
                && v.split('.')
                    .all(|p| p.starts_with(|c: char| c.is_ascii_digit())),
        );
    } else {
        return Some(false);
    };
    let v = version.trim();
    Some(!v.is_empty() && !v.contains(['*', ',', '<', '>', '~', '!']))
}

/// `image@sha256:…` or `image:tag` (not `latest`) → pinned.
fn docker_pinned(image: &str) -> bool {
    if image.contains('@') {
        return true;
    }
    let name = image.rsplit('/').next().unwrap_or(image);
    name.split_once(':')
        .is_some_and(|(_, tag)| !tag.is_empty() && tag != "latest")
}

/// AI-SEC-006.
pub struct InsecureMcpTransport;

impl Rule for InsecureMcpTransport {
    fn meta(&self) -> &'static RuleMeta {
        &META_006
    }

    fn applies_to(&self, asset: &Asset) -> bool {
        json_views(&META_006, asset)
    }

    fn check(&self, asset: &Asset, _ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
        for server in &asset.mcp().servers {
            let (Transport::Http { url } | Transport::Sse { url }) = &server.transport else {
                continue;
            };
            let Some(severity) = insecure_url(&url.0) else {
                continue;
            };
            let message = format!(
                "Insecure MCP transport \"{}\" (server \"{}\")",
                truncate(&url.0, QUOTED),
                truncate(&server.name, 24)
            );
            let span = value_span(asset, url.1, &url.0, 0, url.0.len());
            let mut f = Finding::new(&META_006, asset, span, message);
            f.severity = severity;
            out.push(f);
        }
    }
}

/// Severity of a plain-http `url`: `None` for https, loopback hosts or variable hosts,
/// Medium for a private (RFC 1918) IPv4 host, else High.
pub fn insecure_url(url: &str) -> Option<Severity> {
    let scheme = url.get(..7)?;
    if !scheme.eq_ignore_ascii_case("http://") {
        return None;
    }
    let rest = &url[7..];
    let authority = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    let host_port = authority.rsplit('@').next().unwrap_or(authority);
    let host = if let Some(v6) = host_port.strip_prefix('[') {
        v6.split(']').next().unwrap_or(v6)
    } else {
        host_port.split(':').next().unwrap_or(host_port)
    };
    if host.is_empty() || host.contains(['$', '{']) {
        return None;
    }
    let lower = host.to_ascii_lowercase();
    if LOOPBACK_HOSTS.contains(&lower.as_str()) || lower.ends_with(".localhost") {
        return None;
    }
    match host.parse::<Ipv4Addr>() {
        Ok(ip) if ip.is_loopback() => None,
        Ok(ip) if ip.is_private() => Some(Severity::Medium),
        _ => Some(Severity::High),
    }
}

/// The span of `len` bytes at `off` in a JSON string `value` whose node covers `span`,
/// without quotes. Falls back to the whole string when escapes shift the offsets.
pub(crate) fn value_span(asset: &Asset, span: Span, value: &str, off: usize, len: usize) -> Span {
    let src = asset.source.as_bytes();
    let quoted =
        span.end >= span.start + 2 && matches!(src.get(span.start as usize), Some(b'"' | b'\''));
    if !quoted {
        return span;
    }
    let inner = Span::new(span.start + 1, span.end - 1);
    if asset.source.get(inner.start as usize..inner.end as usize) != Some(value) {
        return inner;
    }
    match (u32::try_from(off), u32::try_from(len)) {
        (Ok(o), Ok(l)) => Span::new(inner.start + o, inner.start + o + l),
        _ => inner,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_skips_references_placeholders_and_switches() {
        for v in [
            "",
            "${env:GITHUB_TOKEN}",
            "${input:token}",
            "${TOKEN}",
            "$TOKEN",
            "Bearer ${env:API_KEY}",
            "<your-token>",
            "true",
            "4096",
        ] {
            assert!(!literal(v), "{v}");
        }
        for v in [
            "/run/secrets/token",
            "./key.pem",
            "C:\\keys\\a.pem",
            "https://auth.example.com",
        ] {
            assert!(!literal(v), "{v}");
        }
        for v in [
            "ghp_abc123",
            "Bearer abc123def",
            "hunter2",
            "pa$word",
            "https://u:pw@h",
        ] {
            assert!(literal(v), "{v}");
        }
    }

    #[test]
    fn credential_key_skips_location_and_mode_keys() {
        for k in ["GITHUB_TOKEN", "-api-key", "Authorization", "apiKey"] {
            assert!(credential_key(k), "{k}");
        }
        for k in [
            "API_KEY_FILE",
            "-key-file",
            "AUTH_URL",
            "AUTH_TYPE",
            "AUTH_ENABLED",
            "LOG_LEVEL",
        ] {
            assert!(!credential_key(k), "{k}");
        }
    }

    #[test]
    fn launcher_recognizes_commands_and_subcommands() {
        assert_eq!(launcher("npx", &[]), Some((Eco::Npm, 0)));
        assert_eq!(launcher("C:\\node\\npx.cmd", &[]), Some((Eco::Npm, 0)));
        assert_eq!(launcher("/usr/bin/bunx", &[]), Some((Eco::Npm, 0)));
        assert_eq!(launcher("pnpm", &["dlx", "x"]), Some((Eco::Npm, 1)));
        assert_eq!(launcher("yarn", &["dlx", "x"]), Some((Eco::Npm, 1)));
        assert_eq!(launcher("pnpm", &["install"]), None);
        assert_eq!(launcher("uvx", &[]), Some((Eco::Python, 0)));
        assert_eq!(launcher("uv", &["tool", "run"]), Some((Eco::Python, 2)));
        assert_eq!(launcher("uv", &["run", "x"]), None);
        assert_eq!(launcher("pipx", &["run", "x"]), Some((Eco::Python, 1)));
        assert_eq!(launcher("docker", &["run", "x"]), Some((Eco::Docker, 1)));
        assert_eq!(launcher("docker", &["exec", "x"]), None);
        assert_eq!(launcher("node", &["server.js"]), None);
    }

    fn pkgs(eco: Eco, args: &[&str]) -> (Vec<String>, bool) {
        let l = package_args(eco, args).unwrap_or(Launch {
            packages: vec![],
            auto_install: false,
        });
        let specs = l
            .packages
            .iter()
            .map(|&(i, off)| args[i][off..].to_owned())
            .collect();
        (specs, l.auto_install)
    }

    #[test]
    fn package_args_finds_specs_after_flags() {
        assert_eq!(
            pkgs(Eco::Npm, &["-y", "@scope/pkg@1.2.3", "--port", "3"]),
            (vec!["@scope/pkg@1.2.3".into()], true)
        );
        assert_eq!(
            pkgs(Eco::Npm, &["--yes", "--package", "a@1", "-p=b", "bin"]),
            (vec!["a@1".into(), "b".into()], true)
        );
        assert_eq!(
            pkgs(Eco::Npm, &["--registry", "https://r", "--", "pkg"]),
            (vec!["pkg".into()], false)
        );
        assert_eq!(
            pkgs(Eco::Python, &["--from", "mcp-x==1.0", "mcp-x"]),
            (vec!["mcp-x==1.0".into()], false)
        );
        assert_eq!(
            pkgs(Eco::Python, &["--python", "3.12", "mcp-server-fetch"]),
            (vec!["mcp-server-fetch".into()], false)
        );
        assert_eq!(
            pkgs(
                Eco::Docker,
                &[
                    "-i",
                    "--rm",
                    "-e",
                    "TOKEN",
                    "-v=/a:/b",
                    "ghcr.io/o/img:1.0",
                    "--flag"
                ]
            ),
            (vec!["ghcr.io/o/img:1.0".into()], false)
        );
        assert_eq!(pkgs(Eco::Docker, &["-i", "--rm"]), (vec![], false));
    }

    #[test]
    fn pinned_npm_needs_exact_semver() {
        for (spec, want) in [
            ("@scope/pkg@1.2.3", Some(true)),
            ("pkg@1.2.3-beta.1", Some(true)),
            ("pkg@0.6.2+build.7", Some(true)),
            ("@scope/pkg", Some(false)),
            ("pkg", Some(false)),
            ("pkg@latest", Some(false)),
            ("pkg@^1", Some(false)),
            ("pkg@~1.2.3", Some(false)),
            ("pkg@1", Some(false)),
            ("pkg@1.x", Some(false)),
            ("pkg@next", Some(false)),
            ("./local", None),
            ("github:o/r", None),
            ("https://x/y.tgz", None),
        ] {
            assert_eq!(pinned(Eco::Npm, spec), want, "{spec}");
        }
    }

    #[test]
    fn pinned_python_needs_exact_version() {
        for (spec, want) in [
            ("mcp-server-fetch==2025.1.17", Some(true)),
            ("mcp-x[cli]==1.0", Some(true)),
            ("mcp-x@0.4.1", Some(true)),
            ("mcp-x", Some(false)),
            ("mcp-x@latest", Some(false)),
            ("mcp-x@1", Some(false)),
            ("mcp-x@1.2", Some(false)),
            ("mcp-x>=1.0", Some(false)),
            ("mcp-x~=1.0", Some(false)),
            ("mcp-x==1.*", Some(false)),
            ("git+https://github.com/o/r", None),
            ("/opt/mcp", None),
        ] {
            assert_eq!(pinned(Eco::Python, spec), want, "{spec}");
        }
    }

    #[test]
    fn pinned_docker_needs_tag_or_digest() {
        for (spec, want) in [
            ("ghcr.io/github/github-mcp-server@sha256:abc", Some(true)),
            ("mcp/fetch:1.2.0", Some(true)),
            ("localhost:5000/mcp:2", Some(true)),
            ("mcp/fetch", Some(false)),
            ("mcp/fetch:latest", Some(false)),
            ("localhost:5000/mcp", Some(false)),
            ("${IMAGE}", None),
        ] {
            assert_eq!(pinned(Eco::Docker, spec), want, "{spec}");
        }
    }

    #[test]
    fn insecure_url_grades_hosts() {
        for (url, want) in [
            ("https://api.example.com/mcp", None),
            ("http://localhost:3000/mcp", None),
            ("HTTP://127.0.0.1/sse", None),
            ("http://[::1]:8080/mcp", None),
            ("http://mcp.localhost/mcp", None),
            ("http://127.0.0.2:9000/mcp", None),
            ("http://${env:HOST}/mcp", None),
            ("http://api.example.com/mcp", Some(Severity::High)),
            ("http://user@8.8.8.8:80", Some(Severity::High)),
            ("http://10.0.0.5:8080/mcp", Some(Severity::Medium)),
            ("http://172.16.4.1/mcp", Some(Severity::Medium)),
            ("http://192.168.1.10/sse", Some(Severity::Medium)),
            ("http://172.32.0.1/mcp", Some(Severity::High)),
        ] {
            assert_eq!(insecure_url(url), want, "{url}");
        }
    }
}
