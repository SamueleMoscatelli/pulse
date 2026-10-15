//! AI-SEC-007 word lists: tool permissions that grant shell, network or write access.

/// `permissions.allow` entries that allow every shell command.
pub const UNRESTRICTED_BASH: &[&str] = &["Bash", "Bash(*)", "Bash(:*)"];

/// `permissions.allow` entries that allow fetching any URL.
pub const UNRESTRICTED_WEBFETCH: &[&str] = &["WebFetch", "WebFetch(*)"];

/// Frontmatter tools that reach the network (with Bash: an exfiltration chain).
pub const NETWORK_TOOLS: &[&str] = &["WebFetch", "WebSearch"];

/// Command-line flags that disable permission prompts.
pub const BYPASS_FLAGS: &[&str] = &["--dangerously-skip-permissions", "--yolo"];

/// Words (lowercase) in an MCP tool name that mark it as writing or executing.
pub const WRITE_EXEC_WORDS: &[&str] = &[
    "add", "apply", "bash", "command", "commit", "create", "delete", "deploy", "drop", "edit",
    "exec", "execute", "insert", "install", "kill", "merge", "modify", "move", "patch", "post",
    "push", "put", "remove", "rename", "replace", "rm", "run", "send", "set", "shell", "terminal",
    "update", "upload", "write",
];
