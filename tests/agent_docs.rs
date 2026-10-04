//! Keeps the docs for agents current: generated files must match the code,
//! and every relative link must resolve. Run
//! `cargo run -p bevaru-mcp -- gen-docs` to fix a failure here.

use std::path::{Path, PathBuf};

use bevaru::agent::docs::{GENERATE_COMMAND, generated_files};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn generated_docs_are_current() {
    let root = root();
    let mut stale = Vec::new();
    for (path, expected) in generated_files(&root) {
        let actual = std::fs::read_to_string(root.join(path)).unwrap_or_default();
        if actual != expected {
            let line = actual
                .lines()
                .zip(expected.lines())
                .position(|(a, e)| a != e)
                .unwrap_or_else(|| actual.lines().count().min(expected.lines().count()));
            let expected_line = expected.lines().nth(line).unwrap_or("<end of file>");
            let actual_line = actual.lines().nth(line).unwrap_or("<end of file>");
            stale.push(format!(
                "  {path} (first difference at line {}):\n    committed: {actual_line}\n    generated: {expected_line}",
                line + 1
            ));
        }
    }
    assert!(
        stale.is_empty(),
        "docs for agents are out of date with the code:\n{}\n\nRegenerate them with:\n  {GENERATE_COMMAND}",
        stale.join("\n")
    );
}

/// Markdown files whose relative links are checked.
fn checked_files(root: &Path) -> Vec<PathBuf> {
    let mut files = vec![root.join("llms.txt"), root.join("AGENTS.md")];
    let mut dirs = vec![root.join("docs/agents")];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let p = entry.path();
            if p.is_dir() {
                dirs.push(p);
            } else if p.extension().is_some_and(|e| e == "md") {
                files.push(p);
            }
        }
    }
    files
}

/// Targets of `[text](target)` links, minus URLs and pure anchors.
fn relative_links(text: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("](") {
        rest = &rest[i + 2..];
        let Some(end) = rest.find(')') else { break };
        let target = &rest[..end];
        if !target.contains("://") && !target.starts_with('#') && !target.starts_with("mailto:") {
            links.push(target.split('#').next().unwrap_or(target).to_string());
        }
        rest = &rest[end..];
    }
    links
}

#[test]
fn agent_doc_links_resolve() {
    let root = root();
    let mut broken = Vec::new();
    for file in checked_files(&root) {
        let text = std::fs::read_to_string(&file).unwrap();
        let dir = file.parent().unwrap();
        for link in relative_links(&text) {
            if !dir.join(&link).exists() {
                broken.push(format!(
                    "  {} → {link}",
                    file.strip_prefix(&root).unwrap().display()
                ));
            }
        }
    }
    assert!(
        broken.is_empty(),
        "broken links in docs for agents:\n{}",
        broken.join("\n")
    );
}

#[test]
fn llms_txt_links_every_agent_doc() {
    let root = root();
    let llms = std::fs::read_to_string(root.join("llms.txt")).unwrap();
    for file in checked_files(&root) {
        let rel = file
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        if rel.starts_with("docs/agents/") {
            assert!(llms.contains(&rel), "llms.txt does not link {rel}");
        }
    }
}
