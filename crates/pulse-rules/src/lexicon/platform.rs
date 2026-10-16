//! Platform vocabularies (AI-STR-005, AI-STR-006): hook events, hook types and built-in tool
//! names. Lists follow the vendors' docs as of 2026-10 (ADR "Hook events and tool lists").

/// Claude Code hook events (settings `hooks`, plugin `hooks/hooks.json`).
pub const CLAUDE_HOOK_EVENTS: &[&str] = &[
    "ConfigChange",
    "CwdChanged",
    "DirectoryAdded",
    "Elicitation",
    "ElicitationResult",
    "FileChanged",
    "InstructionsLoaded",
    "MessageDisplay",
    "Notification",
    "PermissionDenied",
    "PermissionRequest",
    "PostCompact",
    "PostModelSwitch",
    "PostToolBatch",
    "PostToolUse",
    "PostToolUseFailure",
    "PreCompact",
    "PreModelSwitch",
    "PreToolUse",
    "SessionEnd",
    "SessionStart",
    "Setup",
    "Stop",
    "StopFailure",
    "SubagentStart",
    "SubagentStop",
    "TaskCompleted",
    "TaskCreated",
    "TeammateIdle",
    "UserPromptExpansion",
    "UserPromptSubmit",
    "WorktreeCreate",
    "WorktreeRemove",
];

/// Cursor hook events (`.cursor/hooks.json`).
pub const CURSOR_HOOK_EVENTS: &[&str] = &[
    "afterAgentResponse",
    "afterAgentThought",
    "afterFileEdit",
    "afterMCPExecution",
    "afterShellExecution",
    "afterTabFileEdit",
    "beforeMCPExecution",
    "beforeReadFile",
    "beforeShellExecution",
    "beforeSubmitPrompt",
    "beforeTabFileRead",
    "postToolUse",
    "postToolUseFailure",
    "preCompact",
    "preToolUse",
    "sessionEnd",
    "sessionStart",
    "stop",
    "subagentStart",
    "subagentStop",
    "workspaceOpen",
];

/// Claude Code hook handler `type`s.
pub const CLAUDE_HOOK_TYPES: &[&str] = &["agent", "command", "http", "mcp_tool", "prompt"];

/// Cursor hook `type`s.
pub const CURSOR_HOOK_TYPES: &[&str] = &["command", "prompt"];

/// Claude Code built-in tools (permission rules, agent `tools`, command `allowed-tools`),
/// including names still accepted from older releases (`Task`, `MultiEdit`, `LS`, …).
pub const CLAUDE_TOOLS: &[&str] = &[
    "Agent",
    "AskUserQuestion",
    "Bash",
    "BashOutput",
    "CronCreate",
    "CronDelete",
    "CronList",
    "Edit",
    "EnterPlanMode",
    "EnterWorktree",
    "ExitPlanMode",
    "ExitWorktree",
    "Glob",
    "Grep",
    "KillShell",
    "LS",
    "LSP",
    "ListMcpResourcesTool",
    "Monitor",
    "MultiEdit",
    "NotebookEdit",
    "NotebookRead",
    "PowerShell",
    "Read",
    "ReadMcpResourceTool",
    "SendMessage",
    "Skill",
    "SlashCommand",
    "Task",
    "TaskCreate",
    "TaskGet",
    "TaskList",
    "TaskOutput",
    "TaskStop",
    "TaskUpdate",
    "TodoWrite",
    "ToolSearch",
    "WebFetch",
    "WebSearch",
    "Write",
];

/// MCP server name prefixes Claude Code provides itself (IDE bridge, claude.ai connectors,
/// plugin servers); never defined in a project MCP config.
pub const CLAUDE_IMPLICIT_SERVERS: &[&str] = &["ide", "claude_ai_", "plugin_"];

/// GitHub Copilot (VS Code) built-in tools and tool sets for prompt/agent/chat mode `tools`.
/// Extensions add more, so only near-misses of these are reported.
pub const COPILOT_TOOLS: &[&str] = &[
    "agent",
    "changes",
    "codebase",
    "edit",
    "editFiles",
    "execute",
    "extensions",
    "fetch",
    "findTestFiles",
    "githubRepo",
    "new",
    "openSimpleBrowser",
    "problems",
    "read",
    "runCommands",
    "runInTerminal",
    "runNotebooks",
    "runSubagent",
    "runTasks",
    "runTests",
    "search",
    "searchResults",
    "terminalLastCommand",
    "terminalSelection",
    "testFailure",
    "think",
    "todo",
    "todos",
    "usages",
    "vscodeAPI",
    "web",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_are_sorted_and_unique() {
        for list in [
            CLAUDE_HOOK_EVENTS,
            CURSOR_HOOK_EVENTS,
            CLAUDE_HOOK_TYPES,
            CURSOR_HOOK_TYPES,
            CLAUDE_TOOLS,
            COPILOT_TOOLS,
        ] {
            assert!(list.windows(2).all(|w| w[0] < w[1]), "{list:?}");
        }
    }
}
