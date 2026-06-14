//! Session domain model: status enum, the draft a session is created from,
//! the metadata persisted to `metadata.json`, and the dashboard list shape.
//!
//! Schemas follow `docs/build/technical-design.md` §9 and the lifecycle in
//! `docs/build/flows.md` §2.

use serde::{Deserialize, Serialize};

/// The session's own lifecycle status, persisted in `metadata.json`.
/// See flows.md §2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Draft,
    Recording,
    Paused,
    Ending,
    Analyzing,
    Reviewing,
    Completed,
    Failed,
    Recovering,
}

impl Default for SessionStatus {
    fn default() -> Self {
        SessionStatus::Draft
    }
}

/// A label/tag reference attached to a session (`labels.json` defines the set).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelRef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
}

/// The input to `create_session` — the cheap "New Session" form (flows.md §4).
///
/// Everything is optional-friendly so the frontend can send a partially-filled
/// draft; storage fills in `id`, `status`, `created_at`, and cost fields.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionDraft {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub labels: Vec<LabelRef>,
    #[serde(default)]
    pub participants: Vec<String>,
    #[serde(default)]
    pub context_notes: Option<String>,
    #[serde(default)]
    pub budget_cap: Option<f64>,
}

/// Full persisted metadata (`sessions/{uuid}/metadata.json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    pub id: String,
    pub status: SessionStatus,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub labels: Vec<LabelRef>,
    /// ISO-8601 creation timestamp.
    pub date: String,
    #[serde(default)]
    pub duration_ms: u64,
    #[serde(default)]
    pub participants: Vec<String>,
    #[serde(default)]
    pub context_notes: Option<String>,
    #[serde(default)]
    pub budget_cap: Option<f64>,
    #[serde(default)]
    pub total_api_cost: f64,
}

impl SessionMeta {
    /// Build fresh metadata for a brand-new `draft` session from a draft +
    /// generated id + creation timestamp.
    pub fn from_draft(id: String, date: String, draft: SessionDraft) -> Self {
        SessionMeta {
            id,
            status: SessionStatus::Draft,
            name: draft.name,
            labels: draft.labels,
            date,
            duration_ms: 0,
            participants: draft.participants,
            context_notes: draft.context_notes,
            budget_cap: draft.budget_cap,
            total_api_cost: 0.0,
        }
    }
}

/// Returned by `create_session`: `{ session_id }`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatedSession {
    pub session_id: String,
}

/// Returned by `get_session`: meta + (later) transcript + analysis.
///
/// In M1 only `meta` is populated; the rest are placeholders so the shape is
/// stable for the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionFull {
    pub meta: SessionMeta,
    #[serde(default)]
    pub transcript: Vec<serde_json::Value>,
    #[serde(default)]
    pub analysis: Option<serde_json::Value>,
}
