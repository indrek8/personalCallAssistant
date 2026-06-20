//! Live-analysis prompt + structured-output schema (D12; technical-design §6).
//!
//! The system prompt is **frozen per session** (instructions + the user's prep
//! notes) so it can sit behind a `cache_control` breakpoint and be reused across
//! the ~30 s batches. The active F/C/S/Q toggles ride in the *user* turn, so
//! flipping a toggle never invalidates the cached prefix.

use serde_json::{json, Value};

use crate::audio::StreamTag;
use crate::session::model::TranscriptEntry;
use crate::storage::schema::Toggles;

/// Rolling transcript window sent to Haiku (latest ~3 minutes).
pub const WINDOW_MS: u64 = 180_000;

/// Frozen system instructions. All four features are *always* described; the
/// per-batch user message names which are ACTIVE.
const LIVE_SYSTEM: &str = "\
You are a silent, real-time meeting copilot. You receive a rolling transcript of a \
live call, split into \"You\" (the user) and \"Remote\" (the other side), plus the \
user's prep notes. Surface only high-signal, *new* observations from the most recent \
exchange — never restate the whole conversation.

Emit findings in these categories, but only for the ones listed as ACTIVE in the user turn:
- fact_checks: a claim in the call that conflicts with or is unsupported by the prep \
notes. Use severity \"warning\" for a real contradiction, \"info\" for a softer mismatch.
- commitments: a concrete promise / action / deadline someone took on (who, what, \
by_when; by_when may be empty if unstated).
- suggestions: a brief, useful follow-up question or a point the user is missing.
- unanswered_questions: a question that was asked but not yet answered.

Be conservative — empty arrays are the correct answer when nothing new qualifies. Do \
not invent anything not grounded in the transcript. Return only the structured object.";

/// Build the system prompt: frozen instructions + the session's prep notes. The
/// whole string is the cached prefix (stable for the session).
pub fn system_prompt(context_notes: Option<&str>) -> String {
    match context_notes.map(str::trim).filter(|s| !s.is_empty()) {
        Some(notes) => format!("{LIVE_SYSTEM}\n\nPREP NOTES:\n{notes}"),
        None => format!("{LIVE_SYSTEM}\n\nPREP NOTES:\n(none provided)"),
    }
}

/// The active-feature line + rolling transcript window — the volatile user turn.
pub fn user_message(toggles: &Toggles, window: &[TranscriptEntry]) -> String {
    let mut active = Vec::new();
    if toggles.f {
        active.push("fact_checks");
    }
    if toggles.c {
        active.push("commitments");
    }
    if toggles.s {
        active.push("suggestions");
    }
    if toggles.q {
        active.push("unanswered_questions");
    }

    let mut s = format!("ACTIVE: {}\n\nTRANSCRIPT:\n", active.join(", "));
    for e in window {
        let who = match e.stream {
            StreamTag::You => "You",
            StreamTag::Remote => "Remote",
        };
        s.push_str(&format!("[{who}] {}\n", e.text.trim()));
    }
    s
}

/// JSON Schema for `output_config.format` — the API guarantees schema-valid
/// findings (D12). Structured outputs require `additionalProperties:false` +
/// `required` on every object; no numeric/length constraints are used.
pub fn findings_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["fact_checks", "commitments", "suggestions", "unanswered_questions"],
        "properties": {
            "fact_checks": { "type": "array", "items": {
                "type": "object", "additionalProperties": false,
                "required": ["claim", "assessment", "severity"],
                "properties": {
                    "claim": { "type": "string" },
                    "assessment": { "type": "string" },
                    "severity": { "type": "string", "enum": ["warning", "info"] }
                } } },
            "commitments": { "type": "array", "items": {
                "type": "object", "additionalProperties": false,
                "required": ["who", "what", "by_when"],
                "properties": {
                    "who": { "type": "string" },
                    "what": { "type": "string" },
                    "by_when": { "type": "string" }
                } } },
            "suggestions": { "type": "array", "items": { "type": "string" } },
            "unanswered_questions": { "type": "array", "items": { "type": "string" } }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(t: &str) -> TranscriptEntry {
        TranscriptEntry {
            id: "x".into(),
            t_ms: 0,
            stream: StreamTag::You,
            text: t.into(),
            confidence: 1.0,
        }
    }

    #[test]
    fn user_message_lists_only_active_features() {
        let toggles = Toggles {
            f: true,
            c: false,
            s: false,
            q: true,
        };
        let msg = user_message(&toggles, &[e("hello")]);
        assert!(msg.contains("ACTIVE: fact_checks, unanswered_questions"));
        assert!(msg.contains("[You] hello"));
    }

    #[test]
    fn system_prompt_embeds_notes() {
        assert!(system_prompt(Some("CB-2025-041")).contains("CB-2025-041"));
        assert!(system_prompt(None).contains("(none provided)"));
        // Blank notes are treated as none.
        assert!(system_prompt(Some("   ")).contains("(none provided)"));
    }

    #[test]
    fn schema_is_object_with_required_arrays() {
        let s = findings_schema();
        assert_eq!(s["type"], "object");
        assert_eq!(s["additionalProperties"], false);
        assert!(s["properties"]["commitments"].is_object());
    }
}
