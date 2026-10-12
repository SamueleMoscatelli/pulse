//! JSON views (MCP, settings/hooks, tools) over the fixtures in `tests/fixtures/json/`.

use std::path::PathBuf;

use pulse_core::parse::Transport;
use pulse_core::{Asset, AssetKind, LoadMode, Span};

/// An asset over `tests/fixtures/json/<name>`, embedded at compile time.
macro_rules! fixture {
    ($name:literal, $kind:expr) => {
        Asset::new(
            PathBuf::from($name),
            $name,
            $kind,
            LoadMode::Always,
            include_str!(concat!("fixtures/json/", $name)).to_owned(),
        )
    };
}

fn line(a: &Asset, span: Span) -> u32 {
    a.span_to_location(span).line
}

fn text(a: &Asset, span: Span) -> &str {
    &a.source[span.start as usize..span.end as usize]
}

#[test]
fn claude_mcp_extracts_servers_and_nested_env_line() {
    let a = fixture!("claude.mcp.json", AssetKind::McpConfig);
    let mcp = a.mcp();
    assert!(mcp.errors.is_empty(), "{:?}", mcp.errors);
    let names: Vec<_> = mcp.servers.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["github", "remote", "events"]);

    let gh = &mcp.servers[0];
    assert!(matches!(&gh.transport, Transport::Stdio { command } if command.0 == "npx"));
    assert_eq!(gh.args.len(), 2);
    let (k, v, span) = &gh.env[0];
    assert_eq!(
        (k.as_str(), v.as_str()),
        ("GITHUB_PERSONAL_ACCESS_TOKEN", "${GITHUB_TOKEN}")
    );
    assert_eq!(line(&a, *span), 7);
    assert_eq!(
        a.json()
            .span_of("/mcpServers/github/env/GITHUB_PERSONAL_ACCESS_TOKEN"),
        Some(*span)
    );
    assert_eq!(line(&a, gh.name_span), 3);

    assert!(
        matches!(&mcp.servers[1].transport, Transport::Http { url } if url.0.ends_with("/mcp"))
    );
    assert_eq!(mcp.servers[1].headers[0].0, "Authorization");
    assert!(matches!(&mcp.servers[2].transport, Transport::Sse { .. }));
}

#[test]
fn vscode_mcp_handles_comments_inputs_and_shape_errors() {
    let a = fixture!("vscode.mcp.json", AssetKind::McpConfig);
    assert!(a.json().errors.is_empty(), "{:?}", a.json().errors);
    let mcp = a.mcp();
    assert_eq!(mcp.inputs.len(), 1);
    assert_eq!(mcp.inputs[0].id, "db-password");
    assert!(mcp.inputs[0].password);
    assert_eq!(mcp.servers.len(), 1);
    let db = &mcp.servers[0];
    assert_eq!(db.name, "db");
    assert_eq!(line(&a, db.env[0].2), 17);
    assert_eq!(text(&a, db.env[0].2), "\"${input:db-password}\"");
    assert_eq!(mcp.errors.len(), 1);
    assert_eq!(line(&a, mcp.errors[0].span), 19);
}

#[test]
fn settings_extracts_permissions_flags_and_hook_command_line() {
    let a = fixture!("claude.settings.json", AssetKind::Settings);
    let s = a.settings();
    assert!(s.errors.is_empty(), "{:?}", s.errors);
    assert_eq!(s.allow.len(), 2);
    assert_eq!(s.deny[0].0, "Read(./.env)");
    assert_eq!(line(&a, s.deny[0].1), 4);
    assert_eq!(s.ask[0].0, "Bash(git push:*)");
    assert_eq!(
        s.default_mode.as_ref().map(|m| m.0.as_str()),
        Some("acceptEdits")
    );
    assert_eq!(s.enable_all_project_mcp.map(|f| f.0), Some(true));
    assert_eq!(s.env[0].0, "NODE_ENV");

    assert_eq!(s.hooks.len(), 2);
    let pre = &s.hooks[0];
    assert_eq!(pre.event, "PreToolUse");
    assert_eq!(line(&a, pre.event_span), 11);
    assert_eq!(pre.matcher.as_deref(), Some("Bash"));
    let cmd = &pre.hooks[0];
    assert_eq!(cmd.kind, "command");
    assert_eq!(
        cmd.command,
        "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/check.sh"
    );
    assert_eq!(line(&a, cmd.command_span), 17);
    assert_eq!(cmd.timeout, Some(30.0));

    let stop = &s.hooks[1];
    assert_eq!(stop.matcher, None);
    assert_eq!(stop.hooks[0].kind, "prompt");
    assert_eq!(stop.hooks[0].command, "Check the tests pass.");
}

#[test]
fn cursor_hooks_file_uses_flat_commands() {
    let a = fixture!("cursor.hooks.json", AssetKind::Hooks);
    let hooks = a.hooks();
    assert_eq!(hooks.len(), 3);
    assert_eq!(hooks[0].event, "beforeShellExecution");
    assert_eq!(hooks[0].hooks[0].command, "./hooks/audit.sh");
    assert_eq!(line(&a, hooks[0].hooks[0].command_span), 4);
    assert!(hooks[2].hooks.is_empty());
    assert_eq!(a.settings().errors.len(), 1);
}

#[test]
fn anthropic_tools_array() {
    let a = fixture!("anthropic.tools.json", AssetKind::Tools);
    let t = a.tools();
    assert_eq!(t.tools.len(), 1);
    let w = &t.tools[0];
    assert_eq!(w.name, "get_weather");
    assert_eq!(
        w.description.as_deref(),
        Some("Get the current weather for a city.")
    );
    assert_eq!(w.desc_span.map(|s| line(&a, s)), Some(4));
    assert_eq!(w.schema_span.map(|s| line(&a, s)), Some(5));
    assert_eq!(t.errors.len(), 1, "missing name");
}

#[test]
fn openai_tools_nested_and_flat() {
    let a = fixture!("openai.tools.json", AssetKind::Tools);
    let t = a.tools();
    assert!(t.errors.is_empty(), "{:?}", t.errors);
    let names: Vec<_> = t.tools.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, ["search_docs", "flat_tool"]);
    assert_eq!(t.tools[0].desc_span.map(|s| line(&a, s)), Some(7));
    assert!(t.tools[1].schema_span.is_some());
}

#[test]
fn invalid_json_yields_empty_views_and_one_error() {
    let a = Asset::new(
        PathBuf::from(".mcp.json"),
        ".mcp.json",
        AssetKind::McpConfig,
        LoadMode::Always,
        "{ \"mcpServers\": { ".to_owned(),
    );
    assert_eq!(a.json().errors.len(), 1);
    assert!(a.mcp().servers.is_empty() && a.mcp().errors.is_empty());
    assert!(a.settings().hooks.is_empty());
    assert!(a.tools().tools.is_empty());
    assert!(std::ptr::eq(a.json(), a.json()));
}

fn inline(kind: AssetKind, src: &str) -> Asset {
    Asset::new(
        PathBuf::from("x.json"),
        "x.json",
        kind,
        LoadMode::Always,
        src.to_owned(),
    )
}

#[test]
fn hook_types_without_command_are_not_errors() {
    let a = inline(
        AssetKind::Settings,
        r#"{"hooks": {"Stop": [{"hooks": [
            {"type": "http", "url": "https://h.example"},
            {"type": "command"}
        ]}]}}"#,
    );
    let s = a.settings();
    assert_eq!(s.hooks[0].hooks.len(), 1);
    assert_eq!(s.hooks[0].hooks[0].kind, "http");
    assert!(s.hooks[0].hooks[0].command.is_empty());
    assert_eq!(s.errors.len(), 1, "command hook without command");
}

#[test]
fn mcp_type_without_its_endpoint_is_an_error() {
    let a = inline(
        AssetKind::McpConfig,
        r#"{"mcpServers": {"a": {"type": "stdio", "url": "https://x"}, "b": {}}}"#,
    );
    let mcp = a.mcp();
    assert!(
        mcp.servers
            .iter()
            .all(|s| s.transport == Transport::Unknown)
    );
    assert_eq!(mcp.errors.len(), 1, "{:?}", mcp.errors);
}
