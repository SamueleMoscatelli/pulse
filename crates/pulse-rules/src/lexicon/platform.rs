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

/// GitHub Copilot hook events (`.github/hooks/*.json`, `version: 1`).
pub const COPILOT_HOOK_EVENTS: &[&str] = &[
    "agentStop",
    "errorOccurred",
    "postToolUse",
    "preToolUse",
    "sessionEnd",
    "sessionStart",
    "subagentStop",
    "userPromptSubmitted",
];

/// Kiro v1 hook `trigger`s (`.kiro/hooks/*.json`), with the camelCase aliases kept by
/// `/upgrade-agent` (kiro.dev/docs/hooks/types, ide/whats-new-v1/hooks, cli/v3/hooks-migration).
pub const KIRO_HOOK_TRIGGERS: &[&str] = &[
    "AgentSpawn",
    "Manual",
    "PostFileCreate",
    "PostFileDelete",
    "PostFileSave",
    "PostTaskExec",
    "PostToolUse",
    "PreTaskExec",
    "PreToolUse",
    "SessionEnd",
    "SessionStart",
    "Stop",
    "UserPromptSubmit",
    "agentSpawn",
    "postToolUse",
    "preToolUse",
    "stop",
    "userPromptSubmit",
];

/// Kiro CLI 2.x agent `hooks` events (the object form, kiro.dev/docs/custom-agents/
/// configuration-reference and cli/v3/hooks-migration).
pub const KIRO_AGENT_HOOK_EVENTS: &[&str] = &[
    "agentSpawn",
    "postToolUse",
    "preToolUse",
    "stop",
    "userPromptSubmit",
];

/// Kiro CLI 2.x agent hook types: a hook runs a `command`.
pub const KIRO_AGENT_HOOK_TYPES: &[&str] = &["command"];

/// Kiro agent `tools`/`allowedTools` names (https://kiro.dev/docs/tools/): built-in tool IDs,
/// category tags (`read`, `write`, `shell`, `web`, `subagent`), the `*` selector and CLI 2.x
/// names still found in agents (`fs_read`, `execute_cmd`, `use_aws`, `todo_list`, …).
/// `@builtin`, `@mcp`, `@server`, `@server/tool` and `subagent/<agent>` are selectors.
pub const KIRO_TOOLS: &[&str] = &[
    "*",
    "aws",
    "code",
    "control_bash_process",
    "control_pwsh_process",
    "createHook",
    "delegate",
    "delete_file",
    "disclose_context",
    "execute_bash",
    "execute_cmd",
    "file_search",
    "fs_append",
    "fs_read",
    "fs_write",
    "get_process_output",
    "glob",
    "goal",
    "grep",
    "grep_search",
    "introspect",
    "invoke_sub_agent",
    "kiro_powers",
    "knowledge",
    "list_directory",
    "list_processes",
    "read",
    "read_file",
    "remote_web_search",
    "session",
    "shell",
    "str_replace",
    "subagent",
    "task",
    "thinking",
    "todo",
    "todo_list",
    "tool_search",
    "use_aws",
    "web",
    "web_fetch",
    "web_search",
    "write",
];

/// Kiro IDE 0.x `*.kiro.hook` `when.type`s, plus `manual` (in use in the corpus).
pub const KIRO_LEGACY_HOOK_EVENTS: &[&str] = &[
    "agentSpawn",
    "agentStop",
    "fileCreated",
    "fileDeleted",
    "fileEdited",
    "manual",
    "postTaskExecution",
    "postToolUse",
    "preTaskExecution",
    "preToolUse",
    "promptSubmit",
    "userTriggered",
];

/// Kiro v1 hook `action.type`s.
pub const KIRO_HOOK_TYPES: &[&str] = &["agent", "command"];

/// Kiro IDE 0.x `then.type`s.
pub const KIRO_LEGACY_HOOK_TYPES: &[&str] = &["askAgent", "runCommand"];

/// Claude Code hook handler `type`s.
pub const CLAUDE_HOOK_TYPES: &[&str] = &["agent", "command", "http", "mcp_tool", "prompt"];

/// Cursor hook `type`s.
pub const CURSOR_HOOK_TYPES: &[&str] = &["command", "prompt"];

/// GitHub Copilot hook `type`s.
pub const COPILOT_HOOK_TYPES: &[&str] = &["command"];

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
    "vscode",
    "vscodeAPI",
    "web",
];

/// Copilot tool aliases (Claude Code and other agents' names for `execute`, `read`, `edit`,
/// `search`, `agent`, `web`, `todo`); Copilot matches tool names case-insensitively (P8.7).
pub const COPILOT_TOOL_ALIASES: &[&str] = &[
    "bash",
    "custom-agent",
    "edit",
    "glob",
    "grep",
    "multiedit",
    "notebookedit",
    "notebookread",
    "powershell",
    "read",
    "shell",
    "task",
    "todowrite",
    "webfetch",
    "websearch",
    "write",
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
            COPILOT_HOOK_EVENTS,
            KIRO_HOOK_TRIGGERS,
            KIRO_AGENT_HOOK_EVENTS,
            KIRO_AGENT_HOOK_TYPES,
            KIRO_TOOLS,
            KIRO_LEGACY_HOOK_EVENTS,
            KIRO_HOOK_TYPES,
            KIRO_LEGACY_HOOK_TYPES,
            CLAUDE_TOOLS,
            COPILOT_TOOLS,
            COPILOT_TOOL_ALIASES,
        ] {
            assert!(list.windows(2).all(|w| w[0] < w[1]), "{list:?}");
        }
    }
}
