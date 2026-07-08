//! `timeback::cli` — Command-line interface (Git-like).
//!
//! The welcome screen's command loop dispatches unknown commands to
//! `timeback::cli::run(cmd)` which parses and executes them.

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use super::commit::Author;
use super::hash::Hash;
use super::repo;

const HELP: &str = "\
TimeBack — Git for kernel state
  tb init <path>       Create repo at path (e.g. T:/TIMEBACK)
  tb add <path>        Stage a file
  tb commit <msg>      Create commit from staged files
  tb log               Show commit history
  tb status            Show working tree status
  tb branch <name>     Create branch
  tb checkout <ref>    Switch branch/commit
  tb branches          List branches
  tb diff <a> <b>      Diff two commits (use short hashes)
  tb save [name]       Snapshot current state
  tb restore <id>      Rollback to a commit
";

/// Run a TimeBack command. Returns the output text.
pub fn run(cmd: &str) -> String {
    let mut out = String::new();
    let parts: Vec<&str> = cmd.trim().split_whitespace().collect();
    if parts.is_empty() {
        out.push_str(HELP);
        return out;
    }

    // Allow `tb <cmd>` or just `<cmd>`.
    let argv: &[&str] = if parts[0] == "tb" || parts[0] == "timeback" {
        &parts[1..]
    } else {
        &parts[..]
    };

    if argv.is_empty() {
        out.push_str(HELP);
        return out;
    }

    match argv[0] {
        "help" | "-h" | "--help" => out.push_str(HELP),
        "init" => {
            let path = if argv.len() > 1 { argv[1] } else { "T:/TIMEBACK" };
            if repo::init(path) {
                out.push_str(&format!("Initialized repo at {}\n", path));
            } else {
                out.push_str("Init failed (SSD not available)\n");
            }
        }
        "add" => {
            if argv.len() < 2 {
                out.push_str("usage: tb add <path>\n");
            } else {
                // Demo: add a synthetic blob with the path as content
                let content = format!("content of {}\n", argv[1]);
                if let Some(h) = repo::add(argv[1], content.as_bytes()) {
                    out.push_str(&format!("Added {} ({})\n", argv[1], h.short()));
                } else {
                    out.push_str("Add failed\n");
                }
            }
        }
        "commit" => {
            let msg = if argv.len() > 1 {
                argv[1..].join(" ")
            } else {
                "auto-commit".to_string()
            };
            let author = Author::kernel();
            match repo::commit(&msg, author) {
                Some(h) => out.push_str(&format!("[{}] {}\n", h.short(), msg)),
                None => out.push_str("Commit failed\n"),
            }
        }
        "log" => {
            let commits = repo::log();
            if commits.is_empty() {
                out.push_str("(no commits yet)\n");
            } else {
                for c in &commits {
                    out.push_str(&format!(
                        "commit {}\n  Author: {} <{}>\n  Date:   {}\n\n    {}\n\n",
                        c.hash.short(),
                        c.author.name,
                        c.author.email,
                        c.timestamp_ns,
                        c.message
                    ));
                }
            }
        }
        "status" => out.push_str(&repo::status()),
        "branch" => {
            if argv.len() < 2 {
                out.push_str("usage: tb branch <name>\n");
            } else if repo::branch(argv[1]) {
                out.push_str(&format!("Created branch {}\n", argv[1]));
            } else {
                out.push_str("Branch failed\n");
            }
        }
        "branches" => {
            let branches = repo::branches();
            if branches.is_empty() {
                out.push_str("(no branches)\n");
            } else {
                for b in &branches {
                    out.push_str(&format!("  {} -> {}\n", b.name, b.hash.short()));
                }
            }
        }
        "checkout" => {
            if argv.len() < 2 {
                out.push_str("usage: tb checkout <branch|hash>\n");
            } else if repo::checkout(argv[1]) {
                out.push_str(&format!("Switched to {}\n", argv[1]));
            } else {
                out.push_str("Checkout failed\n");
            }
        }
        "diff" => {
            if argv.len() < 3 {
                out.push_str("usage: tb diff <hashA> <hashB>\n");
            } else {
                // Pad short hashes to full (40 chars)
                let ha = pad_hash(argv[1]);
                let hb = pad_hash(argv[2]);
                match (Hash::from_hex(&ha), Hash::from_hex(&hb)) {
                    (Some(a), Some(b)) => {
                        let diffs = repo::diff(a, b);
                        if diffs.is_empty() {
                            out.push_str("(no changes)\n");
                        } else {
                            for (path, op) in &diffs {
                                let op_str = match op {
                                    repo::DiffOp::Added => "added",
                                    repo::DiffOp::Removed => "removed",
                                    repo::DiffOp::Modified => "modified",
                                };
                                out.push_str(&format!("  {} {}\n", op_str, path));
                            }
                        }
                    }
                    _ => out.push_str("Invalid hash\n"),
                }
            }
        }
        "save" => {
            let name = if argv.len() > 1 { argv[1] } else { "snapshot" };
            repo::add(&format!("{}.snap", name), b"snapshot data");
            let author = Author::kernel();
            match repo::commit(name, author) {
                Some(h) => out.push_str(&format!("Saved snapshot {} ({})\n", name, h.short())),
                None => out.push_str("Save failed\n"),
            }
        }
        "restore" => {
            if argv.len() < 2 {
                out.push_str("usage: tb restore <hash>\n");
            } else {
                let full = pad_hash(argv[1]);
                if let Some(h) = Hash::from_hex(&full) {
                    if repo::checkout(&h.to_hex()) {
                        out.push_str(&format!("Restored to {}\n", h.short()));
                    } else {
                        out.push_str("Restore failed\n");
                    }
                } else {
                    out.push_str("Invalid hash\n");
                }
            }
        }
        _ => {
            out.push_str(&format!("Unknown command: {}\n\n", argv[0]));
            out.push_str(HELP);
        }
    }

    out
}

/// Pad a short hash to a full 40-char hash by right-padding with zeros.
fn pad_hash(short: &str) -> String {
    if short.len() >= 40 { return short.to_string(); }
    let mut s = String::from(short);
    while s.len() < 40 { s.push('0'); }
    s
}
