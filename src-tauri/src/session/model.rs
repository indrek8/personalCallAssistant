//! Session domain model: status enum, the draft a session is created from,
//! the metadata persisted to `metadata.json`, and the dashboard list shape.
//!
//! Schemas follow `docs/build/technical-design.md` §9 and the lifecycle in
//! `docs/build/flows.md` §2.

use serde::{Deserialize, Serialize};

use crate::audio::StreamTag;

/// One finalized transcript line (technical-design.md §9). Persisted as a line
/// in `transcript.jsonl` and streamed to the UI via the `transcript-entry` event
/// (PR3). `stream` serializes to `"you"` / `"remote"`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptEntry {
    pub id: String,
    /// Start time from capture start, in milliseconds (sample-derived).
    pub t_ms: u64,
    pub stream: StreamTag,
    pub text: String,
    /// Mean token probability from Whisper, 0.0–1.0.
    pub confidence: f32,
}

/// The session's own lifecycle status, persisted in `metadata.json`.
/// See flows.md §2.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    #[default]
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

/// Returned by `get_session`: meta + transcript + post-analysis.
///
/// `analysis` is `None` until M4's post-analysis runs and writes `analysis.json`
/// (read back by `storage::read_analysis`); the frontend renders it read-only.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionFull {
    pub meta: SessionMeta,
    #[serde(default)]
    pub transcript: Vec<TranscriptEntry>,
    #[serde(default)]
    pub analysis: Option<Analysis>,
}

/// Post-session analysis (`analysis.json`) — Sonnet extraction reviewed/edited
/// before save (M4; technical-design.md §6, §9). Also the `SessionFull.analysis`
/// payload returned to the dashboard.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Analysis {
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub actions: Vec<Action>,
    #[serde(default)]
    pub decisions: Vec<String>,
    #[serde(default)]
    pub key_topics: Vec<String>,
    /// ISO-8601 timestamp the analysis was generated.
    #[serde(default)]
    pub generated_at: String,
}

/// One extracted action item (technical-design.md §9). The minimal Sonnet output
/// (title/owner/deadline/quote/type) is enriched here with a stable `id`, review
/// `status`, `owner_type`, and provenance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub owner: String,
    #[serde(default)]
    pub owner_type: OwnerType,
    /// Serializes as `"type"` (`commitment` / `follow_up` / `suggestion`).
    #[serde(rename = "type", default)]
    pub kind: ActionType,
    #[serde(default)]
    pub status: ActionStatus,
    #[serde(default)]
    pub deadline: Option<String>,
    #[serde(default)]
    pub transcript_quote: String,
    /// Best-effort link back to the transcript moment (0 when Sonnet-extracted —
    /// the quote is the anchor; precise linking is a v1 concern).
    #[serde(default)]
    pub transcript_t_ms: u64,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub created_by: CreatedBy,
    #[serde(default)]
    pub completed_at: Option<String>,
}

/// What kind of action this is. Wire: `commitment` / `follow_up` / `suggestion`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    #[default]
    Commitment,
    FollowUp,
    Suggestion,
}

/// Review status of an action. Wire: `pending` / `in_progress` / `done` /
/// `wont_do` / `postponed`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionStatus {
    #[default]
    Pending,
    InProgress,
    Done,
    WontDo,
    Postponed,
}

/// Whose action it is. Wire: `mine` / `theirs`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerType {
    #[default]
    Mine,
    Theirs,
}

/// How the action entered the set. Wire: `ai_extracted` / `manual`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreatedBy {
    #[default]
    AiExtracted,
    Manual,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_status_wire_format_is_lowercase() {
        // The on-disk + IPC contract: status strings are lowercase and round-trip.
        assert_eq!(serde_json::to_string(&SessionStatus::Recording).unwrap(), "\"recording\"");
        assert_eq!(serde_json::to_string(&SessionStatus::Completed).unwrap(), "\"completed\"");
        assert_eq!(serde_json::to_string(&SessionStatus::Recovering).unwrap(), "\"recovering\"");
        assert_eq!(SessionStatus::default(), SessionStatus::Draft);
        let s: SessionStatus = serde_json::from_str("\"paused\"").unwrap();
        assert_eq!(s, SessionStatus::Paused);
    }

    #[test]
    fn stream_tag_wire_format_is_lowercase() {
        // Load-bearing for transcript.jsonl + the transcript-entry event.
        assert_eq!(serde_json::to_string(&StreamTag::You).unwrap(), "\"you\"");
        assert_eq!(serde_json::to_string(&StreamTag::Remote).unwrap(), "\"remote\"");
        let t: StreamTag = serde_json::from_str("\"remote\"").unwrap();
        assert_eq!(t, StreamTag::Remote);
    }

    #[test]
    fn from_draft_maps_fields_and_zeroes_runtime_state() {
        let draft = SessionDraft {
            name: Some("Board call".into()),
            labels: vec![LabelRef { id: "b".into(), name: "Board".into(), color: None }],
            participants: vec!["Ahmed".into(), "Sarah".into()],
            context_notes: Some("CB-2025-041".into()),
            budget_cap: Some(5.0),
        };
        let m = SessionMeta::from_draft("id-1".into(), "2026-06-20T00:00:00Z".into(), draft);
        assert_eq!(m.id, "id-1");
        assert_eq!(m.status, SessionStatus::Draft);
        assert_eq!(m.name.as_deref(), Some("Board call"));
        assert_eq!(m.participants, vec!["Ahmed".to_string(), "Sarah".to_string()]);
        assert_eq!(m.budget_cap, Some(5.0));
        // A brand-new session starts with zero duration + cost.
        assert_eq!(m.duration_ms, 0);
        assert_eq!(m.total_api_cost, 0.0);
    }

    #[test]
    fn analysis_action_wire_format_round_trips() {
        // The on-disk (analysis.json) + IPC contract for actions — enum + renamed
        // field wire values are load-bearing for the frontend mirror.
        let a = Action {
            id: "a1".into(),
            title: "Send the board the timeline".into(),
            owner: "Me".into(),
            owner_type: OwnerType::Mine,
            kind: ActionType::FollowUp,
            status: ActionStatus::Pending,
            deadline: Some("2026-04-05".into()),
            transcript_quote: "I'll circulate it by Friday".into(),
            transcript_t_ms: 1234,
            notes: None,
            created_by: CreatedBy::AiExtracted,
            completed_at: None,
        };
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains("\"type\":\"follow_up\""), "{json}");
        assert!(json.contains("\"status\":\"pending\""));
        assert!(json.contains("\"owner_type\":\"mine\""));
        assert!(json.contains("\"created_by\":\"ai_extracted\""));
        let back: Action = serde_json::from_str(&json).unwrap();
        assert_eq!(back.kind, ActionType::FollowUp);
        assert_eq!(back.status, ActionStatus::Pending);

        // Remaining enum wire values + partial-object defaults.
        assert_eq!(serde_json::to_string(&ActionStatus::InProgress).unwrap(), "\"in_progress\"");
        assert_eq!(serde_json::to_string(&ActionStatus::WontDo).unwrap(), "\"wont_do\"");
        assert_eq!(serde_json::to_string(&ActionType::Commitment).unwrap(), "\"commitment\"");
        assert_eq!(serde_json::to_string(&OwnerType::Theirs).unwrap(), "\"theirs\"");
        assert_eq!(serde_json::to_string(&CreatedBy::Manual).unwrap(), "\"manual\"");
        let an: Analysis = serde_json::from_str("{}").unwrap();
        assert!(an.summary.is_empty() && an.actions.is_empty() && an.decisions.is_empty());
    }
}
