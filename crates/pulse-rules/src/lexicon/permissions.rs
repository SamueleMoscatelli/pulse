//! AI-SEC-007 word lists: tool permissions that grant shell, network or write access.

/// `permissions.allow` entries that allow every shell command.
pub const UNRESTRICTED_BASH: &[&str] = &["Bash", "Bash(*)", "Bash(:*)"];

/// `permissions.allow` entries that allow fetching any URL.
pub const UNRESTRICTED_WEBFETCH: &[&str] = &["WebFetch", "WebFetch(*)"];

/// Frontmatter tools that reach the network (with Bash: an exfiltration chain).
pub const NETWORK_TOOLS: &[&str] = &["WebFetch", "WebSearch"];

/// Kiro `allowedTools` entries that pre-approve every shell command (https://kiro.dev/docs/tools/).
pub const KIRO_SHELL_TOOLS: &[&str] = &["*", "@builtin", "execute_bash", "execute_cmd", "shell"];

/// Kiro `allowedTools` entries that pre-approve web access.
pub const KIRO_WEB_TOOLS: &[&str] = &[
    "*",
    "@builtin",
    "remote_web_search",
    "web",
    "web_fetch",
    "web_search",
];

/// Kiro `allowedTools` entries that pre-approve file writes.
pub const KIRO_WRITE_TOOLS: &[&str] = &[
    "*",
    "@builtin",
    "fs_append",
    "fs_write",
    "str_replace",
    "write",
];

/// Kiro `toolsSettings` keys that restrict shell commands (`allowedCommands`).
pub const KIRO_SHELL_SETTINGS: &[&str] = &["execute_bash", "execute_cmd", "shell"];

/// Kiro `toolsSettings` keys that restrict written paths (`allowedPaths`).
pub const KIRO_WRITE_SETTINGS: &[&str] = &["fs_write", "write"];

/// Kiro `permissions.rules` capabilities that cover shell commands.
pub const KIRO_SHELL_CAPABILITIES: &[&str] = &["all", "builtin", "shell"];

/// Kiro `permissions.rules` capabilities that cover file writes.
pub const KIRO_WRITE_CAPABILITIES: &[&str] = &["all", "builtin", "filesystem", "fs_write"];

/// Command-line flags that disable permission prompts.
pub const BYPASS_FLAGS: &[&str] = &["--dangerously-skip-permissions", "--yolo"];

/// Words (lowercase) in an MCP tool name that mark it as writing or executing.
pub const WRITE_EXEC_WORDS: &[&str] = &[
    "add", "apply", "bash", "command", "commit", "create", "delete", "deploy", "drop", "edit",
    "exec", "execute", "insert", "install", "kill", "merge", "modify", "move", "patch", "post",
    "push", "put", "remove", "rename", "replace", "rm", "run", "send", "set", "shell", "terminal",
    "update", "upload", "write",
];
