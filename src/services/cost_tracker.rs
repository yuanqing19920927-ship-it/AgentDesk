//! Module 6 — cost & usage tracker.
//!
//! Parses Claude Code JSONL session files and aggregates per-session
//! and per-project token usage into USD. Pricing is a static table
//! keyed by model-name prefix — when a model is not recognized, we
//! still surface the raw token counts but report `cost_usd = 0.0`.
//!
//! This is intentionally *derived on demand*: no persistent cost
//! database. The JSONL files are the source of truth, and because
//! they are append-only we can re-scan them cheaply.

use crate::models::{ModelBreakdown, ModelPricing, ProjectCost, SessionCost, UsageTokens};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

// ──────────────────────── pricing ────────────────────────

/// Returns the price (USD per 1M tokens) for a given model. Supports
/// Claude families (Opus / Sonnet / Haiku, Anthropic API prices) and
/// OpenAI GPT families (GPT-5.x, sourced from OpenRouter). Prefix
/// matching — "claude-opus-4-6" routes to Opus, "gpt-5.4" to GPT-5.4.
///
/// For GPT models we set `cache_write = 0` because OpenAI's cache
/// mechanism is an implicit discount on the read side, not a separate
/// write fee.
pub fn price_for_model(model: &str) -> Option<ModelPricing> {
    let m = model.to_lowercase();
    if m.contains("opus") {
        Some(ModelPricing {
            input: 15.00,
            output: 75.00,
            cache_write: 18.75,
            cache_read: 1.50,
        })
    } else if m.contains("sonnet") {
        Some(ModelPricing {
            input: 3.00,
            output: 15.00,
            cache_write: 3.75,
            cache_read: 0.30,
        })
    } else if m.contains("haiku") {
        Some(ModelPricing {
            input: 0.80,
            output: 4.00,
            cache_write: 1.00,
            cache_read: 0.08,
        })
    } else if m.starts_with("gpt-5.4-pro") {
        Some(ModelPricing { input: 30.00, output: 180.00, cache_write: 0.0, cache_read: 0.0 })
    } else if m.starts_with("gpt-5.4-nano") {
        Some(ModelPricing { input: 0.20, output: 1.25, cache_write: 0.0, cache_read: 0.02 })
    } else if m.starts_with("gpt-5.4-mini") {
        Some(ModelPricing { input: 0.75, output: 4.50, cache_write: 0.0, cache_read: 0.075 })
    } else if m.starts_with("gpt-5.4") {
        // Matches "gpt-5.4" and any suffixed variants (chat, codex, …).
        Some(ModelPricing { input: 2.50, output: 15.00, cache_write: 0.0, cache_read: 0.25 })
    } else if m.starts_with("gpt-5.3-codex") || m.starts_with("gpt-5.3-chat") {
        Some(ModelPricing { input: 1.75, output: 14.00, cache_write: 0.0, cache_read: 0.175 })
    } else if m.starts_with("gpt-5.2-pro") {
        Some(ModelPricing { input: 21.00, output: 168.00, cache_write: 0.0, cache_read: 0.0 })
    } else if m.starts_with("gpt-5.2") || m.starts_with("gpt-5.3") {
        Some(ModelPricing { input: 1.75, output: 14.00, cache_write: 0.0, cache_read: 0.175 })
    } else if m.starts_with("gpt-5") {
        // Fallback for bare "gpt-5" — same rate as gpt-5.4 flagship.
        Some(ModelPricing { input: 2.50, output: 15.00, cache_write: 0.0, cache_read: 0.25 })
    } else {
        None
    }
}

// ──────────────────────── scanning ────────────────────────

/// Parse a single JSONL session file and return its cost breakdown.
pub fn cost_for_session_file(path: &Path) -> Option<SessionCost> {
    let content = fs::read_to_string(path).ok()?;
    let file_stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    let mut session_id: Option<String> = None;
    let mut project_root: Option<String> = None;
    let mut started_at: Option<String> = None;
    let mut last_activity: Option<String> = None;
    let mut message_count: u64 = 0;
    // model name → (tokens, cost, count)
    let mut per_model: HashMap<String, (UsageTokens, f64, u64)> = HashMap::new();
    let mut total = UsageTokens::default();
    let mut total_cost = 0.0_f64;

    for line in content.lines() {
        let record: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if session_id.is_none() {
            if let Some(sid) = record.get("sessionId").and_then(|s| s.as_str()) {
                session_id = Some(sid.to_string());
            }
        }
        if project_root.is_none() {
            if let Some(cwd) = record.get("cwd").and_then(|s| s.as_str()) {
                project_root = Some(cwd.to_string());
            }
        }
        if let Some(ts) = record.get("timestamp").and_then(|t| t.as_str()) {
            if started_at.is_none() {
                started_at = Some(ts.to_string());
            }
            last_activity = Some(ts.to_string());
        }

        if record.get("type").and_then(|t| t.as_str()) != Some("assistant") {
            continue;
        }
        let message = match record.get("message") {
            Some(m) => m,
            None => continue,
        };
        let usage = match message.get("usage") {
            Some(u) => u,
            None => continue,
        };
        let model = message
            .get("model")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown")
            .to_string();

        let u = UsageTokens {
            input: usage
                .get("input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            output: usage
                .get("output_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            cache_write: usage
                .get("cache_creation_input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            cache_read: usage
                .get("cache_read_input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
        };

        // Skip empty-usage sync messages (tool results etc. that have
        // no model call attached).
        if u.total() == 0 {
            continue;
        }

        let cost = price_for_model(&model)
            .map(|p| p.cost_usd(&u))
            .unwrap_or(0.0);

        total.add(&u);
        total_cost += cost;
        message_count += 1;

        let entry = per_model
            .entry(model)
            .or_insert((UsageTokens::default(), 0.0, 0));
        entry.0.add(&u);
        entry.1 += cost;
        entry.2 += 1;
    }

    if message_count == 0 {
        return None;
    }

    let mut models: Vec<ModelBreakdown> = per_model
        .into_iter()
        .map(|(model, (tokens, cost_usd, message_count))| ModelBreakdown {
            model,
            tokens,
            cost_usd,
            message_count,
        })
        .collect();
    models.sort_by(|a, b| b.cost_usd.partial_cmp(&a.cost_usd).unwrap_or(std::cmp::Ordering::Equal));

    Some(SessionCost {
        session_id: session_id.unwrap_or(file_stem),
        project_root,
        started_at,
        last_activity,
        models,
        total_tokens: total,
        total_cost_usd: total_cost,
        message_count,
    })
}

/// Parse a Codex rollout JSONL and return its cost breakdown.
///
/// Codex files are structurally different from Claude Code sessions:
/// * `session_meta` holds `cwd` (used by `codex_scanner`, not us).
/// * `turn_context.payload.model` carries the model name (e.g. "gpt-5.4").
/// * `event_msg` with `payload.type == "token_count"` carries a
///   *cumulative* `total_token_usage` object. We must take the LAST
///   such record, not sum them, or we would double-count.
///
/// Token field mapping, Codex → our `UsageTokens`:
/// * `cached_input_tokens` → `cache_read`
/// * `input_tokens - cached_input_tokens` → `input` (uncached prompt)
/// * `output_tokens + reasoning_output_tokens` → `output`
///   (OpenAI bills reasoning tokens at the completion rate)
/// * `cache_write` stays 0 — OpenAI has no separate cache-write fee.
pub fn cost_for_codex_session_file(path: &Path) -> Option<SessionCost> {
    let content = fs::read_to_string(path).ok()?;
    let file_stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    let mut session_id: Option<String> = None;
    let mut project_root: Option<String> = None;
    let mut started_at: Option<String> = None;
    let mut last_activity: Option<String> = None;
    let mut model: Option<String> = None;
    // Cumulative totals — overwritten on every token_count record so
    // the last one wins.
    let mut last_totals: Option<(u64, u64, u64, u64)> = None;

    for line in content.lines() {
        let record: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(ts) = record.get("timestamp").and_then(|t| t.as_str()) {
            if started_at.is_none() {
                started_at = Some(ts.to_string());
            }
            last_activity = Some(ts.to_string());
        }

        let record_type = record.get("type").and_then(|t| t.as_str()).unwrap_or("");
        let payload = record.get("payload");

        match record_type {
            "session_meta" => {
                if let Some(p) = payload {
                    if session_id.is_none() {
                        if let Some(id) = p.get("id").and_then(|v| v.as_str()) {
                            session_id = Some(id.to_string());
                        }
                    }
                    if project_root.is_none() {
                        if let Some(cwd) = p.get("cwd").and_then(|v| v.as_str()) {
                            project_root = Some(cwd.to_string());
                        }
                    }
                }
            }
            "turn_context" => {
                // turn_context carries the active model for this turn.
                // Projects can switch models mid-session; we keep the
                // most recent one as the "primary" label.
                if let Some(p) = payload {
                    if let Some(m) = p.get("model").and_then(|v| v.as_str()) {
                        model = Some(m.to_string());
                    }
                }
            }
            "event_msg" => {
                let Some(p) = payload else { continue };
                if p.get("type").and_then(|v| v.as_str()) != Some("token_count") {
                    continue;
                }
                let totals = p
                    .get("info")
                    .and_then(|i| i.get("total_token_usage"))
                    .cloned();
                let Some(totals) = totals else { continue };

                let input = totals.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                let cached = totals
                    .get("cached_input_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let output = totals.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                let reasoning = totals
                    .get("reasoning_output_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                last_totals = Some((input, cached, output, reasoning));
            }
            _ => {}
        }
    }

    let (input, cached, output, reasoning) = last_totals?;
    let model = model.unwrap_or_else(|| "gpt-unknown".to_string());

    let usage = UsageTokens {
        // "input" in our schema means *uncached* prompt tokens, so we
        // subtract the cached portion.
        input: input.saturating_sub(cached),
        output: output + reasoning,
        cache_write: 0,
        cache_read: cached,
    };

    if usage.total() == 0 {
        return None;
    }

    let cost = price_for_model(&model)
        .map(|p| p.cost_usd(&usage))
        .unwrap_or(0.0);

    let breakdown = ModelBreakdown {
        model: model.clone(),
        tokens: usage.clone(),
        cost_usd: cost,
        message_count: 1,
    };

    Some(SessionCost {
        session_id: session_id.unwrap_or(file_stem),
        project_root,
        started_at,
        last_activity,
        models: vec![breakdown],
        total_tokens: usage,
        total_cost_usd: cost,
        message_count: 1,
    })
}

/// Aggregate costs for every JSONL file beneath a set of
/// `~/.claude/projects/<dir>` directories (one or more claude_dir_names
/// bind to one AgentDesk project), plus any Codex rollout files bound
/// to the same project via cwd.
pub fn project_cost(
    project_root: &Path,
    claude_dir_names: &[String],
    codex_session_files: &[std::path::PathBuf],
) -> ProjectCost {
    let home = dirs::home_dir().unwrap_or_default();
    let mut session_count = 0u64;
    let mut message_count = 0u64;
    let mut tokens = UsageTokens::default();
    let mut cost_usd = 0.0_f64;
    let mut per_model: HashMap<String, (UsageTokens, f64, u64)> = HashMap::new();

    for name in claude_dir_names {
        let dir = home.join(".claude").join("projects").join(name);
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            if let Some(s) = cost_for_session_file(&path) {
                session_count += 1;
                message_count += s.message_count;
                tokens.add(&s.total_tokens);
                cost_usd += s.total_cost_usd;
                for mb in s.models {
                    let e = per_model
                        .entry(mb.model)
                        .or_insert((UsageTokens::default(), 0.0, 0));
                    e.0.add(&mb.tokens);
                    e.1 += mb.cost_usd;
                    e.2 += mb.message_count;
                }
            }
        }
    }

    // Codex rollouts — each file contributes a single cumulative
    // token_count record, so we count one "session" per file.
    for path in codex_session_files {
        if let Some(s) = cost_for_codex_session_file(path) {
            session_count += 1;
            message_count += s.message_count;
            tokens.add(&s.total_tokens);
            cost_usd += s.total_cost_usd;
            for mb in s.models {
                let e = per_model
                    .entry(mb.model)
                    .or_insert((UsageTokens::default(), 0.0, 0));
                e.0.add(&mb.tokens);
                e.1 += mb.cost_usd;
                e.2 += mb.message_count;
            }
        }
    }

    let mut models: Vec<ModelBreakdown> = per_model
        .into_iter()
        .map(|(model, (tokens, cost_usd, message_count))| ModelBreakdown {
            model,
            tokens,
            cost_usd,
            message_count,
        })
        .collect();
    models.sort_by(|a, b| b.cost_usd.partial_cmp(&a.cost_usd).unwrap_or(std::cmp::Ordering::Equal));

    ProjectCost {
        project_root: project_root.to_string_lossy().to_string(),
        session_count,
        message_count,
        tokens,
        cost_usd,
        models,
    }
}

/// Format a USD amount in a way that reads well for small values
/// (`$0.023`) and large values (`$12.45`).
pub fn format_usd(amount: f64) -> String {
    if amount < 0.01 {
        format!("${:.4}", amount)
    } else if amount < 10.0 {
        format!("${:.3}", amount)
    } else {
        format!("${:.2}", amount)
    }
}

/// Abbreviate a token count — "1.2M", "845K", "532".
pub fn format_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}
