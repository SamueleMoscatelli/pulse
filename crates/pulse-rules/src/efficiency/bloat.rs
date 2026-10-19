//! Context bloat: inlined bulk content (AI-EFF-004), MCP server/tool sprawl (AI-EFF-005) and
//! oversized skills or agents (AI-EFF-006).

use std::collections::{HashMap, HashSet};

use pulse_core::parse::Transport;
use pulse_core::text::tokens;
use pulse_core::{
    Asset, AssetKind, Category, Finding, KindSet, LoadMode, ProjectRule, Rule, RuleCtx, RuleMeta,
    Severity, Span,
};

use crate::util::truncate;

pub static META_BULK: RuleMeta = RuleMeta {
    id: "AI-EFF-004",
    name: "Inlined bulk content",
    category: Category::Efficiency,
    default_severity: Severity::Medium,
    // Skill and agent bodies are loaded on demand; only their description is always-on.
    kinds: KindSet::MARKDOWN
        .without(AssetKind::Skill)
        .without(AssetKind::Agent),
    summary: "A large code block, table or data dump is inlined in a loaded asset.",
    remediation: "Move it to a separate file and reference it, so it is loaded only on demand.",
    inline_suppressible: true,
};

/// Code block content lines allowed inline.
const MAX_CODE_LINES: u32 = 40;
/// JSON/YAML block content lines allowed inline.
const MAX_DATA_LINES: u32 = 60;
/// Table body rows allowed inline.
const MAX_TABLE_ROWS: u32 = 25;
/// Tokens of the reference line that replaces the block.
const REFERENCE_TOKENS: u32 = 20;

pub struct InlinedBulk;

/// Lines of the code block on file lines `first..=last`, without its fences.
fn content_lines(asset: &Asset, (first, last): (u32, u32)) -> u32 {
    let fence = |line: u32| {
        let s = asset.lines.line_span(&asset.source, line);
        let t = asset
            .source
            .get(s.start as usize..s.end as usize)
            .unwrap_or_default()
            .trim_start();
        t.starts_with("```") || t.starts_with("~~~")
    };
    let mut n = last - first + 1;
    if fence(first) {
        n -= 1;
        if last > first && fence(last) {
            n -= 1;
        }
    }
    n
}

fn is_data(lang: Option<&str>) -> bool {
    lang.is_some_and(|l| {
        ["json", "jsonc", "json5", "yaml", "yml"]
            .iter()
            .any(|d| l.eq_ignore_ascii_case(d))
    })
}

impl Rule for InlinedBulk {
    fn meta(&self) -> &'static RuleMeta {
        &META_BULK
    }

    fn check(&self, asset: &Asset, _ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
        if asset.effective_load() == LoadMode::OnDemand {
            return;
        }
        let md = asset.markdown();
        let calibration = asset.calibration();
        let mut push = |span: Span, waste: u32, message: String| {
            let first = asset
                .lines
                .line_span(&asset.source, asset.lines.line_of(span.start));
            let mut f = Finding::new(&META_BULK, asset, first, message);
            f.token_waste = waste.saturating_sub(REFERENCE_TOKENS);
            out.push(f);
        };
        for block in &md.code_blocks {
            let lines = content_lines(asset, block.lines);
            let (max, what) = if is_data(block.lang.as_deref()) {
                (MAX_DATA_LINES, "data dump")
            } else {
                (MAX_CODE_LINES, "code block")
            };
            if lines <= max {
                continue;
            }
            let text = asset
                .source
                .get(block.span.start as usize..block.span.end as usize)
                .unwrap_or_default();
            let len = u32::try_from(text.len()).unwrap_or(u32::MAX);
            let waste = tokens::estimate_markdown(text, &[Span::new(0, len)], calibration);
            push(
                block.span,
                waste,
                format!("Inlined {what} of {lines} lines (> {max}); move it to a referenced file"),
            );
        }
        for table in md.tables.iter().filter(|t| t.rows > MAX_TABLE_ROWS) {
            let text = asset
                .source
                .get(table.span.start as usize..table.span.end as usize)
                .unwrap_or_default();
            push(
                table.span,
                tokens::estimate(text, calibration),
                format!(
                    "Inlined table of {} rows (> {MAX_TABLE_ROWS}); move it to a referenced file",
                    table.rows
                ),
            );
        }
    }
}

pub static META_SPRAWL: RuleMeta = RuleMeta {
    id: "AI-EFF-005",
    name: "MCP server/tool sprawl",
    category: Category::Efficiency,
    default_severity: Severity::Medium,
    kinds: KindSet::MCP_CONFIG.union(KindSet::TOOLS),
    summary: "Too many MCP servers or tools are enabled, or a server is defined twice.",
    remediation: "Disable servers and tools the project does not need; keep one definition per server.",
    inline_suppressible: true,
};

pub struct McpSprawl;

/// Identity of a server's launch target: the command's file name plus args (npx `-y`
/// dropped), or the URL with a lowercase scheme and host and no trailing `/`.
fn server_key(transport: &Transport, args: &[(String, Span)]) -> Option<String> {
    match transport {
        Transport::Stdio { command } => {
            let cmd = command.0.trim();
            let base = cmd.rsplit(['/', '\\']).next().unwrap_or(cmd).to_lowercase();
            let base = base
                .strip_suffix(".exe")
                .or_else(|| base.strip_suffix(".cmd"))
                .unwrap_or(&base)
                .to_owned();
            let mut key = base;
            for (arg, _) in args {
                let arg = arg.trim();
                if arg == "-y" || arg == "--yes" {
                    continue;
                }
                key.push('\u{1f}');
                key.push_str(arg);
            }
            Some(key)
        }
        Transport::Http { url } | Transport::Sse { url } => {
            let url = url.0.trim().trim_end_matches('/');
            let host_end = url.find("://").map_or(0, |i| {
                url[i + 3..].find('/').map_or(url.len(), |j| i + 3 + j)
            });
            Some(format!(
                "{}{}",
                url[..host_end].to_lowercase(),
                &url[host_end..]
            ))
        }
        Transport::Unknown => None,
    }
}

impl ProjectRule for McpSprawl {
    fn meta(&self) -> &'static RuleMeta {
        &META_SPRAWL
    }

    fn check_project(&self, assets: &[Asset], ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
        let mut sorted: Vec<&Asset> = assets.iter().filter(|a| a.skipped.is_none()).collect();
        sorted.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
        let budgets = &ctx.config.budgets;

        // Servers: enabled, counted once per name across clients' configs.
        let mut names: HashSet<&str> = HashSet::new();
        let mut by_key: HashMap<String, (&str, &str)> = HashMap::new();
        let mut over: Option<Finding> = None;
        for asset in sorted.iter().filter(|a| a.kind == AssetKind::McpConfig) {
            for s in asset.mcp().servers.iter().filter(|s| !s.disabled) {
                if names.insert(&s.name)
                    && names.len() as u64 == budgets.mcp_servers.saturating_add(1)
                {
                    over = Some(Finding::new(
                        &META_SPRAWL,
                        asset,
                        s.name_span,
                        String::new(),
                    ));
                }
                let Some(key) = server_key(&s.transport, &s.args) else {
                    continue;
                };
                match by_key.get(&key) {
                    Some(&(first, path)) if first != s.name => {
                        let mut f = Finding::new(
                            &META_SPRAWL,
                            asset,
                            s.name_span,
                            format!(
                                "Duplicate MCP server \"{}\": same target as \"{}\" in {}",
                                truncate(&s.name, 20),
                                truncate(first, 20),
                                truncate(path, 30)
                            ),
                        );
                        f.severity = Severity::High;
                        out.push(f);
                    }
                    Some(_) => {}
                    None => {
                        by_key.insert(key, (&s.name, &asset.rel_path));
                    }
                }
            }
        }
        if let Some(mut f) = over {
            f.message = format!(
                "MCP server sprawl: {} enabled servers > {}",
                names.len(),
                budgets.mcp_servers
            );
            out.push(f);
        }

        // Tools: every definition; the waste is the schemas past the budget.
        let tools: Vec<_> = sorted
            .iter()
            .filter(|a| a.kind == AssetKind::Tools)
            .flat_map(|a| a.tools().tools.iter().map(move |t| (*a, t)))
            .collect();
        let budget = usize::try_from(budgets.tools_count).unwrap_or(usize::MAX);
        if let Some(&(asset, first)) = tools.get(budget) {
            let waste: u32 = tools[budget..]
                .iter()
                .map(|(a, t)| {
                    let text = a
                        .source
                        .get(t.span.start as usize..t.span.end as usize)
                        .unwrap_or_default();
                    tokens::estimate(text, a.calibration())
                })
                .sum();
            let mut f = Finding::new(
                &META_SPRAWL,
                asset,
                first.name_span,
                format!(
                    "Tool sprawl: {} tools > {}",
                    tools.len(),
                    budgets.tools_count
                ),
            );
            f.token_waste = waste;
            out.push(f);
        }
    }
}

pub static META_OVERSIZED: RuleMeta = RuleMeta {
    id: "AI-EFF-006",
    name: "Oversized skill or agent",
    category: Category::Efficiency,
    default_severity: Severity::Medium,
    kinds: KindSet::SKILL.union(KindSet::AGENT),
    summary: "A skill body is too long, or a skill/agent description exceeds its budget.",
    remediation: "Shorten the description to when-to-use; move skill detail into referenced files.",
    inline_suppressible: true,
};

/// Skill body lines allowed.
const MAX_SKILL_LINES: u32 = 500;

pub struct Oversized;

impl Rule for Oversized {
    fn meta(&self) -> &'static RuleMeta {
        &META_OVERSIZED
    }

    fn check(&self, asset: &Asset, ctx: &RuleCtx<'_>, out: &mut Vec<Finding>) {
        let md = asset.markdown();
        let calibration = asset.calibration();
        if asset.kind == AssetKind::Skill && (md.body.start as usize) < asset.source.len() {
            let first = asset.lines.line_of(md.body.start);
            let last = asset
                .lines
                .line_of(md.body.end.saturating_sub(1).max(md.body.start));
            let lines = last - first + 1;
            if lines > MAX_SKILL_LINES {
                let span = asset
                    .lines
                    .line_span(&asset.source, first + MAX_SKILL_LINES);
                let rest = asset.source.get(span.start as usize..).unwrap_or_default();
                let mut f = Finding::new(
                    &META_OVERSIZED,
                    asset,
                    span,
                    format!(
                        "Skill body is {lines} lines (> {MAX_SKILL_LINES}); split it into referenced files"
                    ),
                );
                f.token_waste = tokens::estimate(rest, calibration);
                out.push(f);
            }
        }
        let Some(fm) = md.frontmatter.as_ref() else {
            return;
        };
        let Some(desc) = fm.get("description").and_then(|v| v.as_str()) else {
            return;
        };
        let budget = ctx.config.budgets.description;
        let n = u64::from(tokens::estimate(desc, calibration));
        if n <= budget {
            return;
        }
        let span = fm
            .key_lines
            .get("description")
            .map_or(fm.span, |&l| asset.lines.line_span(&asset.source, l));
        let waste = n - budget;
        let mut f = Finding::new(
            &META_OVERSIZED,
            asset,
            span,
            format!("Description is {n} tokens > {budget} (always loaded; +{waste})"),
        );
        f.token_waste = u32::try_from(waste).unwrap_or(u32::MAX);
        out.push(f);
    }
}
