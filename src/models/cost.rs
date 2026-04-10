//! Cost & usage data structures for Module 6.
//!
//! Costs are derived from Claude Code JSONL session files. Each
//! `assistant` message carries a `usage` object with input / output /
//! cache token counts. We multiply those by a per-model price table
//! and aggregate into summaries.

use serde::{Deserialize, Serialize};

/// Raw usage numbers pulled out of a single assistant message.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UsageTokens {
    pub input: u64,
    pub output: u64,
    pub cache_write: u64,
    pub cache_read: u64,
}

impl UsageTokens {
    pub fn total(&self) -> u64 {
        self.input + self.output + self.cache_write + self.cache_read
    }

    pub fn add(&mut self, other: &UsageTokens) {
        self.input += other.input;
        self.output += other.output;
        self.cache_write += other.cache_write;
        self.cache_read += other.cache_read;
    }
}

/// Price per 1M tokens, in USD.
#[derive(Clone, Debug)]
pub struct ModelPricing {
    pub input: f64,
    pub output: f64,
    pub cache_write: f64,
    pub cache_read: f64,
}

impl ModelPricing {
    pub fn cost_usd(&self, u: &UsageTokens) -> f64 {
        let per = 1_000_000.0_f64;
        (u.input as f64) * self.input / per
            + (u.output as f64) * self.output / per
            + (u.cache_write as f64) * self.cache_write / per
            + (u.cache_read as f64) * self.cache_read / per
    }
}

/// Summary of tokens + USD for a single model within a scope
/// (session, project, or global).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ModelBreakdown {
    pub model: String,
    pub tokens: UsageTokens,
    pub cost_usd: f64,
    pub message_count: u64,
}

/// A session-scoped cost summary. Summed across all assistant
/// messages in one JSONL file.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SessionCost {
    pub session_id: String,
    pub project_root: Option<String>,
    pub started_at: Option<String>,
    pub last_activity: Option<String>,
    pub models: Vec<ModelBreakdown>,
    pub total_tokens: UsageTokens,
    pub total_cost_usd: f64,
    pub message_count: u64,
}

/// Project-level aggregation across all sessions bound to this project.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProjectCost {
    pub project_root: String,
    pub session_count: u64,
    pub message_count: u64,
    pub tokens: UsageTokens,
    pub cost_usd: f64,
    pub models: Vec<ModelBreakdown>,
}
