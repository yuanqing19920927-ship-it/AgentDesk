//! Module 11 — project health monitor.
//!
//! Aggregates cheap signals into a single `ProjectHealth` snapshot:
//!
//! * git commit counts for the last 7 / 30 days + last commit age
//! * number of JSONL session files touched in the last 7 days
//! * memory index size (from `memory_indexer::read_report`)
//! * currently running agent count (passed in by the caller)
//!
//! Everything is derived on demand. There is no health database —
//! the source of truth is git + filesystem + the existing memory
//! index — so a recompute is cheap and always reflects reality.

use crate::models::{HealthStatus, ProjectHealth};
use crate::services::{approved_projects, memory_indexer};
use chrono::{Duration, Utc};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::SystemTime;

/// Compute a fresh health snapshot for a project. Safe to call from a
/// blocking task — does a handful of small git invocations and a
/// directory scan, nothing expensive.
pub fn compute(
    project_root: &Path,
    claude_dir_names: &[String],
    active_agents: usize,
) -> ProjectHealth {
    let (commits_7d, commits_30d, last_commit_age_days) = git_activity(project_root);
    let sessions_7d = recent_session_count(claude_dir_names, 7);
    let (memory_entries, memory_enabled) = memory_state(project_root);

    let mut hints: Vec<String> = Vec::new();

    // Score each positive signal. The overall status is a function of
    // how many of these are firing — this keeps the heuristic legible
    // and tunable without magic thresholds buried in branches.
    let mut positive = 0u8;

    if let Some(age) = last_commit_age_days {
        if age <= 7 {
            positive += 1;
        } else if age <= 30 {
            hints.push(format!("最近一次提交在 {} 天前", age));
        } else {
            hints.push(format!("已有 {} 天没有提交", age));
        }
    } else {
        hints.push("尚未检测到 git 提交".to_string());
    }

    if sessions_7d > 0 {
        positive += 1;
    } else {
        hints.push("近 7 天无会话记录".to_string());
    }

    if memory_enabled && memory_entries > 0 {
        positive += 1;
    } else if !memory_enabled {
        hints.push("项目记忆未启用".to_string());
    } else {
        hints.push("项目记忆为空".to_string());
    }

    if active_agents > 0 {
        positive += 1;
    }

    let overall = match positive {
        3..=4 => HealthStatus::Green,
        2 => HealthStatus::Yellow,
        _ => HealthStatus::Red,
    };

    ProjectHealth {
        overall,
        commits_7d,
        commits_30d,
        last_commit_age_days,
        memory_entries,
        memory_enabled,
        sessions_7d,
        active_agents,
        hints,
    }
}

// ──────────────────────── git ────────────────────────

/// Run `git log --since=... --format=%h` and count lines. Returns
/// `(commits_7d, commits_30d, last_commit_age_days)`. All three default
/// to zero / None if the directory is not a git repo.
fn git_activity(project_root: &Path) -> (u64, u64, Option<u64>) {
    if !project_root.join(".git").exists() {
        return (0, 0, None);
    }
    let commits_7d = count_commits_since(project_root, "7.days.ago");
    let commits_30d = count_commits_since(project_root, "30.days.ago");
    let age = last_commit_age(project_root);
    (commits_7d, commits_30d, age)
}

fn count_commits_since(project_root: &Path, since: &str) -> u64 {
    let output = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .arg("log")
        .arg(format!("--since={}", since))
        .arg("--format=%h")
        .output();
    let Ok(out) = output else { return 0 };
    if !out.status.success() {
        return 0;
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count() as u64
}

fn last_commit_age(project_root: &Path) -> Option<u64> {
    let out = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .arg("log")
        .arg("-1")
        .arg("--format=%ct")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let ts_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if ts_str.is_empty() {
        return None;
    }
    let ts: i64 = ts_str.parse().ok()?;
    let commit_time = chrono::DateTime::<Utc>::from_timestamp(ts, 0)?;
    let delta = Utc::now().signed_duration_since(commit_time);
    if delta < Duration::zero() {
        Some(0)
    } else {
        Some(delta.num_days() as u64)
    }
}

// ──────────────────────── sessions ────────────────────────

/// Count JSONL session files under the project's Claude project dirs
/// whose mtime is within the given window. Uses file mtime rather than
/// parsing each file — good enough for an at-a-glance signal and
/// avoids re-reading every JSONL on every dashboard open.
fn recent_session_count(claude_dir_names: &[String], days: i64) -> u64 {
    let Some(home) = dirs::home_dir() else { return 0 };
    let cutoff = SystemTime::now()
        .checked_sub(std::time::Duration::from_secs((days * 86_400) as u64))
        .unwrap_or(SystemTime::UNIX_EPOCH);

    let mut count = 0u64;
    for name in claude_dir_names {
        let dir = home.join(".claude").join("projects").join(name);
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            let Ok(mtime) = meta.modified() else { continue };
            if mtime >= cutoff {
                count += 1;
            }
        }
    }
    count
}

// ──────────────────────── memory ────────────────────────

fn memory_state(project_root: &Path) -> (usize, bool) {
    let enabled = approved_projects::is_approved(project_root);
    let entries = memory_indexer::read_report(project_root)
        .map(|r| r.total_entries)
        .unwrap_or(0);
    (entries, enabled)
}
