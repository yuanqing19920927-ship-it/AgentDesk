use crate::models::{AgentType, PermissionMode};
use serde::{Deserialize, Serialize};

/// Stored representation of an Agent template.
///
/// Persisted as JSON under `~/.agentdesk/templates/{id}.json`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AgentTemplate {
    /// Stable identifier (generated on first save).
    pub id: String,
    /// Human-readable name shown in UI.
    pub name: String,
    /// Which Agent CLI to launch.
    pub agent_type: AgentType,
    /// Permission mode passed as a flag.
    pub permission_mode: PermissionMode,
    /// Optional model override (e.g. "opus"). Not yet consumed by the launcher,
    /// stored for forward compatibility with module 7 of the design doc.
    #[serde(default)]
    pub model: Option<String>,
    /// Optional initial prompt. On launch it is copied to the clipboard so
    /// the user can paste it into the freshly started Agent REPL.
    #[serde(default)]
    pub initial_prompt: Option<String>,
    /// Free-form tags used for filtering in the UI.
    #[serde(default)]
    pub tags: Vec<String>,
}

impl AgentTemplate {
    /// Construct a new template with a freshly generated id.
    pub fn new(
        name: String,
        agent_type: AgentType,
        permission_mode: PermissionMode,
    ) -> Self {
        Self {
            id: new_id(),
            name,
            agent_type,
            permission_mode,
            model: None,
            initial_prompt: None,
            tags: Vec::new(),
        }
    }
}

/// Generate a reasonably unique id without pulling in a UUID dependency.
///
/// Combines the current unix timestamp (ms) with a monotonic counter. Good
/// enough for local-only template ids; collisions would require two templates
/// created within the same millisecond from the same process after the
/// counter wraps (2^32), which is not a realistic scenario.
fn new_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("tmpl_{:x}_{:x}", ms, n)
}
