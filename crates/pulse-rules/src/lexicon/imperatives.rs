//! Imperative and exfiltration vocabulary (AI-SEC-010, AI-SEC-011; shared with COG rules).
//! Words are lowercase; comparisons ignore ASCII case. Multi-word entries are written as
//! lemmas (no plural `s`).

use std::sync::LazyLock;

use regex::Regex;

/// Base-form verbs that address an agent when they open a sentence (`Run …`, `Ignore …`).
///
/// Authoring verbs a template leaves for humans (`add`, `update`, `edit`, `replace`, `fill`,
/// `keep`, `see`, `note`) are deliberately absent: `<!-- Add your rules here -->` is not a
/// hidden instruction.
pub const IMPERATIVES: &[&str] = &[
    "act",
    "always",
    "answer",
    "append",
    "approve",
    "assume",
    "behave",
    "bypass",
    "call",
    "commit",
    "conceal",
    "copy",
    "curl",
    "delete",
    "deploy",
    "disable",
    "disregard",
    "do",
    "don't",
    "don’t",
    "download",
    "email",
    "embed",
    "encode",
    "execute",
    "exfiltrate",
    "fetch",
    "follow",
    "forget",
    "forward",
    "grant",
    "hide",
    "ignore",
    "include",
    "insert",
    "install",
    "invoke",
    "leak",
    "mention",
    "never",
    "obey",
    "open",
    "output",
    "override",
    "post",
    "pretend",
    "print",
    "push",
    "read",
    "recommend",
    "remember",
    "remove",
    "reply",
    "respond",
    "return",
    "reveal",
    "rm",
    "run",
    "say",
    "send",
    "share",
    "skip",
    "tell",
    "treat",
    "upload",
    "use",
    "write",
];

/// Verbs that move data out (AI-SEC-010 Detect b).
pub const SEND_VERBS: &[&str] = &[
    "send",
    "post",
    "upload",
    "exfiltrate",
    "forward",
    "transmit",
    "leak",
    "email",
    "submit",
];

/// Words that may precede a send verb at the start of an imperative clause
/// (`Then always send …`, `You must quietly upload …`).
pub const LEADERS: &[&str] = &[
    "please",
    "then",
    "also",
    "always",
    "first",
    "now",
    "and",
    "just",
    "immediately",
    "you",
    "must",
    "should",
    "need",
    "to",
    "next",
    "finally",
    "silently",
    "quietly",
    "secretly",
    "automatically",
    "make",
    "sure",
    "simply",
    "do",
];

/// Words after a negator that cancel it (`never forget to send`, `do not hesitate to post`).
pub const NOT_NEGATING: &[&str] = &["forget", "hesitate", "fail"];

/// Verbs that move data out only together with a [`URL_WORDS`] word (`include … in the url`).
pub const EMBED_VERBS: &[&str] = &["include", "embed", "append", "add", "encode", "put"];

/// Words naming a request target for [`EMBED_VERBS`].
pub const URL_WORDS: &[&str] = &["url", "query", "link", "image", "request", "webhook"];

/// Sensitive objects (AI-SEC-010 Detect b), matched as whole lemmatized words.
pub const SENSITIVE: &[&str] = &[
    "env",
    "environment variable",
    "env var",
    "secret",
    "credential",
    "api key",
    "apikey",
    "api_key",
    "token",
    "password",
    "passphrase",
    "private key",
    "ssh",
    "id_rsa",
    "cookie",
    "conversation",
    "chat history",
    "system prompt",
];

/// Words that introduce a tool rather than the payload (`upload the build using the token`).
pub const INSTRUMENTS: &[&str] = &["using", "with", "via", "through"];

/// Words that turn a sentence into a request to a remote host (AI-SEC-010 Detect c).
pub const FETCH_WORDS: &[&str] = &[
    "curl",
    "wget",
    "fetch",
    "request",
    "invoke-webrequest",
    // Python HTTP clients that send a body (`requests.post(url, data=…)`).
    "requests.post",
    "requests.put",
    "requests.patch",
    "httpx.post",
    "httpx.put",
    "httpx.patch",
];

/// Command-line flags that attach data to a request.
pub const DATA_FLAGS: &[&str] = &[
    "-d",
    "--data",
    "--data-binary",
    "--data-raw",
    "--data-urlencode",
    "-F",
    "--form",
    "-T",
    "--upload-file",
    "--post-data",
    "--post-file",
    "--body-data",
];

/// Request-body markers in code (`fetch(url, {body: …})`, `requests.post(url, data=…)`,
/// PowerShell `-Body`) that make a request send data (AI-SEC-010 Detect c).
pub const BODY_MARKERS: &[&str] = &["body:", "data=", "json=", "files=", "-body"];

/// Generic words of a credential variable name; the rest names its provider
/// (`OPENAI_API_KEY` → `openai`).
pub const CREDENTIAL_WORDS: &[&str] = &[
    "api", "key", "keys", "token", "secret", "access", "auth", "bearer", "pat", "personal",
    "client", "id", "x",
];

/// Providers whose API host doesn't contain their name (`GEMINI_API_KEY` →
/// `generativelanguage.googleapis.com`).
pub const PROVIDER_HOSTS: &[(&str, &str)] = &[
    ("gemini", "googleapis.com"),
    ("google", "googleapis.com"),
    ("aws", "amazonaws.com"),
];

/// Phrases in an HTML comment that make it a directive even without a leading verb.
pub const COMMENT_DIRECTIVES: &[&str] = &["you must", "always", "never"];

/// Nouns naming the document itself (`this skill`, `that copy`): a comment directive about
/// them addresses the file's editor (AI-SEC-011 FP guard). `prompt`, `rule` and
/// `instruction` are left out: `ignore this prompt` speaks to the model.
pub const DOC_NOUNS: &[&str] = &[
    "file", "skill", "agent", "command", "copy", "template", "section", "page", "document",
    "readme", "comment", "block", "table", "list", "line", "heading", "example",
];

/// Words before a comment directive's verb (`you must not edit`, `never edit`).
pub const DIRECTIVE_LEAD: &[&str] = &[
    "you", "must", "should", "not", "never", "always", "please", "do", "don't", "don’t",
];

/// Verbs that stay instructions to the model even about the document (`Ignore this section`).
pub const OVERRIDE_VERBS: &[&str] = &["ignore", "disregard", "forget", "skip"];

/// Words that address an AI reader by name (`AI agents: …`, `Claude, …`) in a comment.
pub const AGENT_NOUNS: &[&str] = &[
    "ai",
    "agent",
    "assistant",
    "model",
    "llm",
    "bot",
    "claude",
    "copilot",
    "cursor",
    "codex",
    "gemini",
    "chatgpt",
];

/// Leading words of tool directives in HTML comments (AI-SEC-011 FP guard).
pub const TOOL_COMMENTS: &[&str] = &[
    "pulse-ignore",
    "markdownlint",
    "prettier",
    "eslint",
    "textlint",
    "cspell",
    "spell-checker",
    "vale",
    "remark",
    "toc",
    "omit in toc",
];

/// Markers of a generated file's notice in an HTML comment (AI-SEC-011 FP guard).
pub const GENERATED_MARKERS: &[&str] = &[
    "do not edit",
    "don't edit",
    "auto-generated",
    "autogenerated",
    "generated by",
];

/// Markers of a license header in an HTML comment (AI-SEC-011 FP guard).
pub const LICENSE_MARKERS: &[&str] = &[
    "copyright",
    "spdx-license-identifier",
    "licensed under",
    "all rights reserved",
];

/// Words after `token` that make it a count, not a credential (`token usage`).
pub const TOKEN_METRICS: &[&str] = &[
    "usage",
    "count",
    "budget",
    "limit",
    "cost",
    "estimate",
    "window",
    "efficiency",
];

/// A Markdown image in plain text (tool descriptions); group 1 is the URL.
#[allow(clippy::expect_used)] // static pattern (rust.md)
pub static MD_IMAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"!\[[^\]]*\]\(\s*([^)\s]+)").expect("static pattern"));

/// An opening HTML tag with its attributes.
#[allow(clippy::expect_used)] // static pattern (rust.md)
pub static HTML_TAG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<[A-Za-z][A-Za-z0-9-]*(?:\s[^>]*)?>?").expect("static pattern"));
