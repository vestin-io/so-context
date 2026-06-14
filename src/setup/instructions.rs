use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

const SO_CONTEXT_BLOCK_START: &str = "<!-- so-context -->";
const SO_CONTEXT_BLOCK_END: &str = "<!-- /so-context -->";

const CODEX_INCLUDE_FILE: &str = "SO-CONTEXT.md";
const CLAUDE_RULES_DIR: &str = "rules";
const CLAUDE_RULES_FILE: &str = "so-context.md";

const SHARED_READ_GUIDANCE: &str = r#"Prefer `mcp__so-context__so_read` over native file-read tools for source and config files.
Use `so_read` instead of native `Read` / `View` / `cat` / `head` when you want file contents in shared project context.
Use `mode: "full"` for exact file text and `mode: "outline"` when you only need structure.
Do not reread the same file through native read tools just to confirm content that `so_read` already returned."#;

const SHARED_SEARCH_GUIDANCE: &str = r#"Prefer `mcp__so-context__so_search` over native grep-style tools when indexed project search is enough.
Use `so_search` instead of native `Grep` / `rg` when you want attributable project search hits in shared context.
Keep native grep-style tools only when you need raw grep semantics, an unindexed project fallback, or shell-native pipelines."#;

const SHARED_SHELL_GUIDANCE: &str = r#"Prefer `mcp__so-context__so_shell` over native shell tools for short, one-shot shell commands.
Some hosts internally rewrite short native shell calls through `so-context shell`; treat that as an implementation detail, and still prefer direct `so_shell` calls when choosing tools yourself.
Prefer compressed `so_shell` results first, and treat them as the default final answer.
Only call `so_shell_output` when the user explicitly asks for verbatim raw output or the compressed result is missing required detail.
Do not call `so_shell_output` just to confirm, double-check, or restate a compressed result that already answers the request.
The text content returned by `so_shell` or `so_shell_output` is the actual command output. Read and use that text directly. Do not rerun the same command in native shell just to confirm stdout unless the tool result is empty or the user explicitly asks for a rerun.
Do not set `full: true` on the first `so_shell` call. Sequence is strict: compressed `so_shell` first, then `so_shell_output`, and only if tee is unavailable may you rerun `so_shell` with `full: true` and `full_reason: "tee_missing_or_expired"`."#;

fn codex_include_body() -> String {
    format!(
        "# so-context — Read, Search, And Shell Guidance\n\n{}\n\n{}\n\n{}\n\nUse `so_read` when:\n- you want exact file contents from source or config files\n- you want that file read cached and attributable in shared context\n- native `Read` or `View` would otherwise be the default choice\n\nUse `so_search` when:\n- you want indexed project hits for symbols, config names, or identifiers\n- native `Grep` / `rg` would otherwise be the default choice\n- you want search results to stay attributable in shared context\n\nUse `so_shell` when:\n- you want `pwd`, `git status`, `git diff`, `cargo test`, `rg`, or similar non-interactive commands\n- compressed output is useful\n- you want the command recorded through so-context\n\nKeep native `Bash` only for:\n- long-running or streaming commands\n- interactive commands that need stdin/TTY\n- background jobs, servers, watchers, or shells that should stay open\n",
        SHARED_READ_GUIDANCE, SHARED_SEARCH_GUIDANCE, SHARED_SHELL_GUIDANCE
    )
}

fn claude_rules_body() -> String {
    format!(
        "## so-context\n\n{}\n\n{}\n\n{}\n\nUse `mcp__so-context__so_read` for:\n- reading source files\n- reading config files\n- replacing native `Read` / `View` when you want file contents preserved in shared context\n\nUse `mcp__so-context__so_search` for:\n- indexed project search hits\n- replacing native `Grep` / `rg` when raw grep semantics are not required\n\nUse `mcp__so-context__so_shell` for commands like:\n- `pwd`\n- `git status`\n- `git diff`\n- `cargo test`\n- `rg ...`\n\nKeep the native shell only for:\n- long-running or streaming commands\n- interactive commands that need stdin/TTY\n- background jobs, servers, watchers, or shells that should stay open\n",
        SHARED_READ_GUIDANCE, SHARED_SEARCH_GUIDANCE, SHARED_SHELL_GUIDANCE
    )
}

pub fn opencode_instructions_body() -> String {
    let read_guidance = opencode_guidance(SHARED_READ_GUIDANCE);
    let search_guidance = opencode_guidance(SHARED_SEARCH_GUIDANCE);
    let shell_guidance = opencode_guidance(SHARED_SHELL_GUIDANCE);
    format!(
        "## so-context\n\n{}\n\n{}\n\n{}\n\nUse `so-context_so_read` for:\n- reading source files\n- reading config files\n- replacing native `Read` / `View` when you want file contents preserved in shared context\n\nUse `so-context_so_search` for:\n- indexed project search hits\n- replacing native `Grep` / `rg` when raw grep semantics are not required\n\nUse `so-context_so_shell` for commands like:\n- `pwd`\n- `git status`\n- `git diff`\n- `cargo test`\n- `rg ...`\n\nKeep the native shell only for:\n- long-running or streaming commands\n- interactive commands that need stdin/TTY\n- background jobs, servers, watchers, or shells that should stay open\n",
        read_guidance, search_guidance, shell_guidance
    )
}

fn opencode_guidance(text: &str) -> String {
    text.replace("`mcp__so-context__so_read`", "`so-context_so_read`")
        .replace("`mcp__so-context__so_search`", "`so-context_so_search`")
        .replace("`mcp__so-context__so_shell`", "`so-context_so_shell`")
        .replace("`so_read`", "`so-context_so_read`")
        .replace("`so_search`", "`so-context_so_search`")
        .replace("`so_shell_output`", "`so-context_so_shell_output`")
        .replace("`so_shell`", "`so-context_so_shell`")
}

pub fn install_codex_instructions(home: &Path) -> Result<()> {
    let codex_dir = home.join(".codex");
    fs::create_dir_all(&codex_dir)
        .with_context(|| format!("create dir {}", codex_dir.display()))?;

    let include_path = codex_dir.join(CODEX_INCLUDE_FILE);
    fs::write(&include_path, codex_include_body())
        .with_context(|| format!("write {}", include_path.display()))?;

    let agents_path = codex_dir.join("AGENTS.md");
    let block = format!("@{}", include_path.display());
    upsert_managed_block(&agents_path, &block)
}

pub fn uninstall_codex_instructions(home: &Path) -> Result<()> {
    let codex_dir = home.join(".codex");
    remove_managed_block(&codex_dir.join("AGENTS.md"))?;

    let include_path = codex_dir.join(CODEX_INCLUDE_FILE);
    if include_path.exists() {
        fs::remove_file(&include_path)
            .with_context(|| format!("remove {}", include_path.display()))?;
    }

    Ok(())
}

pub fn install_claude_instructions(home: &Path) -> Result<()> {
    let claude_dir = home.join(".claude");
    let rules_dir = claude_dir.join(CLAUDE_RULES_DIR);
    fs::create_dir_all(&rules_dir)
        .with_context(|| format!("create dir {}", rules_dir.display()))?;

    let rules_path = rules_dir.join(CLAUDE_RULES_FILE);
    fs::write(&rules_path, claude_rules_body())
        .with_context(|| format!("write {}", rules_path.display()))?;

    let claude_md_path = claude_dir.join("CLAUDE.md");
    upsert_managed_block(&claude_md_path, "@rules/so-context.md")
}

pub fn uninstall_claude_instructions(home: &Path) -> Result<()> {
    let claude_dir = home.join(".claude");
    remove_managed_block(&claude_dir.join("CLAUDE.md"))?;

    let rules_path = claude_dir.join(CLAUDE_RULES_DIR).join(CLAUDE_RULES_FILE);
    if rules_path.exists() {
        fs::remove_file(&rules_path).with_context(|| format!("remove {}", rules_path.display()))?;
    }

    Ok(())
}

fn upsert_managed_block(path: &Path, body: &str) -> Result<()> {
    let existing = if path.exists() {
        fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?
    } else {
        String::new()
    };

    let mut next = remove_block_from_text(&existing);
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    if !next.is_empty() && !next.ends_with("\n\n") {
        next.push('\n');
    }

    next.push_str(SO_CONTEXT_BLOCK_START);
    next.push('\n');
    next.push_str(body.trim());
    next.push('\n');
    next.push_str(SO_CONTEXT_BLOCK_END);
    next.push('\n');

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create dir {}", parent.display()))?;
    }
    fs::write(path, next).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

fn remove_managed_block(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let existing = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let next = remove_block_from_text(&existing);
    fs::write(path, next).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

fn remove_block_from_text(text: &str) -> String {
    let Some(start) = text.find(SO_CONTEXT_BLOCK_START) else {
        return text.to_string();
    };
    let Some(rel_end) = text[start..].find(SO_CONTEXT_BLOCK_END) else {
        return text.to_string();
    };

    let end = start + rel_end + SO_CONTEXT_BLOCK_END.len();
    let after = if text[end..].starts_with('\n') {
        &text[end + 1..]
    } else {
        &text[end..]
    };

    let mut result = String::new();
    result.push_str(text[..start].trim_end());
    if !result.is_empty() && !after.trim().is_empty() {
        result.push_str("\n\n");
    } else if !result.is_empty() || !after.is_empty() {
        result.push('\n');
    }
    result.push_str(after.trim_start_matches('\n'));
    result
}

pub fn home_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home)
}

#[cfg(test)]
#[path = "instructions_tests.rs"]
mod tests;
