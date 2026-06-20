//! Post-session analysis over Sonnet, structured-output (M4; D17; flows §6).
//!
//! Runs on a blocking thread (spawned by the `run_post_analysis` command): reads
//! the whole transcript + prep notes + the live/saved commitments from disk, asks
//! Sonnet for a structured `{summary, actions, decisions, key_topics}`, merges the
//! actions with the user's saved + live commitments (D19), and returns the draft
//! for review. Cost is billed the moment the call returns — *before* the parse — so
//! a refusal / malformed body still records what it cost (D-cost, mirrors `live.rs`).

use std::collections::HashSet;

use chrono::Utc;
use serde::Deserialize;
use serde_json::json;
use tauri::AppHandle;
use uuid::Uuid;

use crate::ai::{prompts, ClaudeClient, MODEL_SONNET};
use crate::error::{AppError, AppResult};
use crate::events;
use crate::session::model::{
    Action, ActionStatus, ActionType, Analysis, CreatedBy, OwnerType, TranscriptEntry,
};
use crate::storage;

/// Output budget — generous so a long call's JSON is never truncated mid-object.
const MAX_TOKENS: u32 = 8192;

/// Below this many transcribed words, skip the model entirely (EXC-EMPTY).
const MIN_WORDS: usize = 25;

/// Run post-analysis for `session_id` and return the review draft. Bills its own
/// Sonnet cost (D-cost) and persists nothing — the command writes the draft.
pub fn run(app: &AppHandle, session_id: &str) -> AppResult<Analysis> {
    let transcript = storage::read_transcript_at(&storage::transcript_path(session_id)?)?;
    let meta = storage::get_session_meta(session_id)?;

    // EXC-EMPTY: too little speech to analyze — minimal review, no API call.
    if count_words(&transcript) < MIN_WORDS {
        return Ok(minimal_empty_analysis());
    }

    let saved = collect_saved_actions(session_id)?;
    let live = collect_live_commitments(session_id)?;
    let annotations: Vec<String> = live.iter().map(annotation_line).collect();

    let client = ClaudeClient::from_stored()?;
    let body = json!({
        "model": MODEL_SONNET,
        "max_tokens": MAX_TOKENS,
        "system": prompts::analysis_system_prompt(meta.context_notes.as_deref()),
        "messages": [{
            "role": "user",
            "content": prompts::analysis_user_message(&transcript, &annotations),
        }],
        "output_config": { "format": { "type": "json_schema", "schema": prompts::analysis_schema() } },
    });
    let resp = client.messages(&body)?;

    // Bill before the parse (D-cost): the call is billed even on refusal / bad body.
    let cost = resp.usage.cost(MODEL_SONNET);
    if cost > 0.0 {
        if let Ok(total) = storage::add_api_cost(session_id, cost) {
            events::emit(
                app,
                events::COST_UPDATE,
                json!({ "session_id": session_id, "total": total, "last": cost }),
            );
        }
    }

    // A refusal carries no usable analysis — surface it (EXC-API-POST).
    if resp.stop_reason.as_deref() == Some("refusal") {
        return Err(AppError::Api("post-analysis was declined by the model".into()));
    }
    let truncated = resp.stop_reason.as_deref() == Some("max_tokens");
    let raw: RawAnalysis = serde_json::from_str(&resp.text())
        .map_err(|e| AppError::Api(format!("post-analysis returned unparseable JSON: {e}")))?;

    let sonnet_actions: Vec<Action> = raw.actions.into_iter().map(raw_to_action).collect();
    let actions = merge_actions(sonnet_actions, saved, live);

    let mut summary = raw.summary.trim().to_string();
    if truncated {
        summary.push_str("\n\n(Note: analysis was truncated — it reached the length limit.)");
    }
    Ok(Analysis {
        summary,
        actions,
        decisions: raw.decisions,
        key_topics: raw.key_topics,
        generated_at: Utc::now().to_rfc3339(),
    })
}

/// Sonnet's structured response (deserialize-only; enriched into `Analysis`).
#[derive(Deserialize, Default)]
struct RawAnalysis {
    #[serde(default)]
    summary: String,
    #[serde(default)]
    actions: Vec<RawAction>,
    #[serde(default)]
    decisions: Vec<String>,
    #[serde(default)]
    key_topics: Vec<String>,
}

#[derive(Deserialize, Default)]
struct RawAction {
    #[serde(default)]
    title: String,
    #[serde(default)]
    owner: String,
    #[serde(default)]
    deadline: String,
    #[serde(default)]
    transcript_quote: String,
    #[serde(rename = "type", default)]
    kind: String,
}

fn count_words(transcript: &[TranscriptEntry]) -> usize {
    transcript.iter().map(|e| e.text.split_whitespace().count()).sum()
}

fn minimal_empty_analysis() -> Analysis {
    Analysis {
        summary: "Nothing substantial was captured in this session.".to_string(),
        actions: Vec::new(),
        decisions: Vec::new(),
        key_topics: Vec::new(),
        generated_at: Utc::now().to_rfc3339(),
    }
}

/// "Me"/"You"/"I" (the user) → mine; anyone else → theirs.
fn owner_type_of(owner: &str) -> OwnerType {
    match owner.trim().to_lowercase().as_str() {
        "me" | "you" | "i" | "myself" | "self" => OwnerType::Mine,
        _ => OwnerType::Theirs,
    }
}

/// Map a free-form / enum `type` string to `ActionType` (unknown → follow_up).
fn action_type_of(kind: &str) -> ActionType {
    match kind.trim().to_lowercase().replace('-', "_").as_str() {
        "commitment" => ActionType::Commitment,
        "suggestion" => ActionType::Suggestion,
        "follow_up" | "followup" => ActionType::FollowUp,
        _ => ActionType::FollowUp,
    }
}

fn non_empty(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_string())
}

fn raw_to_action(r: RawAction) -> Action {
    Action {
        id: Uuid::new_v4().to_string(),
        title: r.title.trim().to_string(),
        owner: r.owner.trim().to_string(),
        owner_type: owner_type_of(&r.owner),
        kind: action_type_of(&r.kind),
        status: ActionStatus::Pending,
        deadline: non_empty(&r.deadline),
        transcript_quote: r.transcript_quote.trim().to_string(),
        transcript_t_ms: 0,
        notes: None,
        created_by: CreatedBy::AiExtracted,
        completed_at: None,
    }
}

/// A user-saved or live commitment finding (the normalized M3 feed-item shape) →
/// an `Action`. `None` for a finding with no title. Saved/live findings keep their
/// original `id` so dedupe-by-id works.
fn finding_to_action(v: &serde_json::Value, created_by: CreatedBy) -> Option<Action> {
    let title = v.get("title").and_then(|t| t.as_str()).unwrap_or("").trim().to_string();
    if title.is_empty() {
        return None;
    }
    let kind = match v.get("kind").and_then(|k| k.as_str()).unwrap_or("commitment") {
        "suggestion" => ActionType::Suggestion,
        "fact" | "question" => ActionType::FollowUp,
        _ => ActionType::Commitment,
    };
    let who = v.get("who").and_then(|t| t.as_str()).unwrap_or("").trim().to_string();
    let by_when = v.get("by_when").and_then(|t| t.as_str()).unwrap_or("").trim().to_string();
    let t_ms = v.get("t_ms").and_then(|t| t.as_u64()).unwrap_or(0);
    let id = v
        .get("id")
        .and_then(|t| t.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    Some(Action {
        id,
        owner_type: owner_type_of(&who),
        title,
        owner: who,
        kind,
        status: ActionStatus::Pending,
        deadline: non_empty(&by_when),
        transcript_quote: String::new(),
        transcript_t_ms: t_ms,
        notes: None,
        created_by,
        completed_at: None,
    })
}

fn collect_saved_actions(session_id: &str) -> AppResult<Vec<Action>> {
    let raw = storage::read_saved_actions(session_id)?;
    Ok(raw.iter().filter_map(|v| finding_to_action(v, CreatedBy::Manual)).collect())
}

fn collect_live_commitments(session_id: &str) -> AppResult<Vec<Action>> {
    let records = storage::read_ai_live(session_id)?;
    let mut out = Vec::new();
    for rec in &records {
        let Some(findings) = rec.get("findings").and_then(|f| f.as_array()) else {
            continue;
        };
        for f in findings {
            if f.get("kind").and_then(|k| k.as_str()) == Some("commitment") {
                if let Some(a) = finding_to_action(f, CreatedBy::AiExtracted) {
                    out.push(a);
                }
            }
        }
    }
    Ok(out)
}

fn annotation_line(a: &Action) -> String {
    let who = if a.owner.is_empty() { "someone" } else { a.owner.as_str() };
    match &a.deadline {
        Some(d) if !d.is_empty() => format!("{who}: {} (by {d})", a.title),
        _ => format!("{who}: {}", a.title),
    }
}

/// Normalize a title for duplicate detection: lowercase, punctuation → spaces,
/// whitespace collapsed.
fn normalize_key(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Whether `title` near-duplicates any existing action's title — exact normalized
/// match, or substantial containment either way. Conservative on purpose: it only
/// suppresses the softer *live* commitments, never a Sonnet or user-saved row.
fn is_duplicate(title: &str, existing: &[Action]) -> bool {
    let key = normalize_key(title);
    if key.is_empty() {
        return false;
    }
    existing.iter().any(|a| {
        let other = normalize_key(&a.title);
        other == key
            || (other.len() >= 8 && key.len() >= 8 && (other.contains(&key) || key.contains(&other)))
    })
}

/// Merge per D19: Sonnet actions first; **every** saved action kept (deduped only
/// against itself by id — a user's `[+ Save action]` is never silently dropped);
/// each live commitment appended only when not a near-duplicate of what's there.
fn merge_actions(sonnet: Vec<Action>, saved: Vec<Action>, live: Vec<Action>) -> Vec<Action> {
    let mut out = sonnet;
    let mut seen_saved = HashSet::new();
    for a in saved {
        if seen_saved.insert(a.id.clone()) {
            out.push(a);
        }
    }
    for a in live {
        if !is_duplicate(&a.title, &out) {
            out.push(a);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::StreamTag;

    fn tentry(text: &str) -> TranscriptEntry {
        TranscriptEntry {
            id: "x".into(),
            t_ms: 0,
            stream: StreamTag::You,
            text: text.into(),
            confidence: 1.0,
        }
    }

    fn act(id: &str, title: &str, cb: CreatedBy) -> Action {
        Action {
            id: id.into(),
            title: title.into(),
            owner: String::new(),
            owner_type: OwnerType::Theirs,
            kind: ActionType::Commitment,
            status: ActionStatus::Pending,
            deadline: None,
            transcript_quote: String::new(),
            transcript_t_ms: 0,
            notes: None,
            created_by: cb,
            completed_at: None,
        }
    }

    #[test]
    fn owner_type_maps_first_person_to_mine() {
        for me in ["Me", "you", "I", "  myself "] {
            assert_eq!(owner_type_of(me), OwnerType::Mine, "{me}");
        }
        for them in ["Sarah", "Ahmed", ""] {
            assert_eq!(owner_type_of(them), OwnerType::Theirs, "{them}");
        }
    }

    #[test]
    fn action_type_parses_and_falls_back() {
        assert_eq!(action_type_of("commitment"), ActionType::Commitment);
        assert_eq!(action_type_of("follow_up"), ActionType::FollowUp);
        assert_eq!(action_type_of("follow-up"), ActionType::FollowUp);
        assert_eq!(action_type_of("suggestion"), ActionType::Suggestion);
        assert_eq!(action_type_of("garbage"), ActionType::FollowUp);
    }

    #[test]
    fn count_words_sums_across_entries() {
        assert_eq!(count_words(&[tentry("one two"), tentry("three")]), 3);
        assert!(count_words(&[tentry("a b c")]) < MIN_WORDS);
    }

    #[test]
    fn minimal_empty_analysis_is_safe() {
        let a = minimal_empty_analysis();
        assert!(!a.summary.is_empty());
        assert!(a.actions.is_empty() && a.decisions.is_empty());
    }

    #[test]
    fn normalize_and_duplicate_detection() {
        assert_eq!(normalize_key("Send the Q2 Report!!"), "send the q2 report");
        let existing = vec![act("s", "Send the Q2 report", CreatedBy::AiExtracted)];
        assert!(is_duplicate("send the q2 report", &existing)); // exact (normalized)
        assert!(is_duplicate("Send the Q2 report to the board", &existing)); // containment
        assert!(!is_duplicate("Book the vendor review", &existing));
        assert!(!is_duplicate("", &existing));
    }

    #[test]
    fn raw_to_action_enriches_defaults() {
        let a = raw_to_action(RawAction {
            title: " Ship it ".into(),
            owner: "Me".into(),
            deadline: "".into(),
            transcript_quote: "we ship".into(),
            kind: "commitment".into(),
        });
        assert_eq!(a.title, "Ship it");
        assert_eq!(a.owner_type, OwnerType::Mine);
        assert_eq!(a.status, ActionStatus::Pending);
        assert_eq!(a.created_by, CreatedBy::AiExtracted);
        assert_eq!(a.deadline, None);
        assert!(!a.id.is_empty());
    }

    #[test]
    fn finding_to_action_maps_commitment_value() {
        let v = json!({ "id": "c1", "kind": "commitment", "t_ms": 4200, "title": "Send report", "who": "Sarah", "by_when": "Friday" });
        let a = finding_to_action(&v, CreatedBy::Manual).unwrap();
        assert_eq!(a.id, "c1");
        assert_eq!(a.title, "Send report");
        assert_eq!(a.owner, "Sarah");
        assert_eq!(a.owner_type, OwnerType::Theirs);
        assert_eq!(a.deadline.as_deref(), Some("Friday"));
        assert_eq!(a.transcript_t_ms, 4200);
        assert_eq!(a.created_by, CreatedBy::Manual);
        // A titleless finding is dropped.
        assert!(finding_to_action(&json!({ "kind": "commitment" }), CreatedBy::Manual).is_none());
    }

    #[test]
    fn merge_keeps_all_saved_and_dedups_live() {
        let sonnet = vec![act("s1", "Send the board the Q2 timeline", CreatedBy::AiExtracted)];
        let saved = vec![
            act("v1", "Send the board the Q2 timeline", CreatedBy::Manual), // dup of sonnet → STILL kept
            act("v2", "Book the vendor review", CreatedBy::Manual),
            act("v2", "Book the vendor review", CreatedBy::Manual), // same id → deduped
        ];
        let live = vec![
            act("l1", "send the board the q2 timeline", CreatedBy::AiExtracted), // dup → dropped
            act("l2", "Chase the legal sign-off", CreatedBy::AiExtracted),       // unique → kept
        ];
        let merged = merge_actions(sonnet, saved, live);
        assert_eq!(merged.len(), 4);
        assert_eq!(merged.iter().filter(|a| a.created_by == CreatedBy::Manual).count(), 2);
        assert!(merged.iter().any(|a| a.title == "Chase the legal sign-off"));
        // Sonnet row + saved dup both survive; the live dup was dropped.
        assert_eq!(
            merged.iter().filter(|a| normalize_key(&a.title) == "send the board the q2 timeline").count(),
            2
        );
    }

    #[test]
    fn parses_a_sonnet_fixture() {
        let raw: RawAnalysis = serde_json::from_str(
            r#"{
                "summary": "Discussed the timeline.",
                "actions": [{"title":"Send timeline","owner":"Me","deadline":"Apr 5","transcript_quote":"I'll send it","type":"commitment"}],
                "decisions": ["Deadline moved to Aug"],
                "key_topics": ["timeline"]
            }"#,
        )
        .unwrap();
        assert_eq!(raw.actions.len(), 1);
        assert_eq!(raw.decisions, vec!["Deadline moved to Aug".to_string()]);
        let a = raw_to_action(raw.actions.into_iter().next().unwrap());
        assert_eq!(a.kind, ActionType::Commitment);
        assert_eq!(a.deadline.as_deref(), Some("Apr 5"));
        // Malformed → Err at the call site.
        assert!(serde_json::from_str::<RawAnalysis>("{ not json").is_err());
    }
}
