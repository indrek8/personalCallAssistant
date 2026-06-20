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

// ----------------------------------------------------------------------------
// Post-analysis (Sonnet) — full-transcript extraction (M4; technical-design §6)
// ----------------------------------------------------------------------------

/// Frozen post-analysis instructions. Unlike the live prompt this runs once over
/// the whole transcript, so it asks for a summary + actions + decisions + topics.
const ANALYSIS_SYSTEM: &str = "\
You are an expert meeting analyst. You are given the full transcript of a finished call, \
split into \"You\" (the user) and \"Remote\" (the other side), the user's prep notes, and \
any commitments already detected live. Produce a faithful post-meeting analysis as a \
single structured object:
- summary: a tight, factual recap (3-6 sentences) of what was discussed and decided — no \
fluff, no invented detail.
- actions: concrete next steps. For each give a short title; the owner (\"Me\" for the user, \
otherwise the person's name); a deadline if one was stated (else an empty string); the exact \
transcript_quote it came from; and a type — \"commitment\" (a clear promise someone made), \
\"follow_up\" (a task implied but not explicitly promised), or \"suggestion\" (a softer \
recommended next step). Fold in the live-detected commitments; do not duplicate them.
- decisions: concrete decisions reached, each a short sentence. Empty if none.
- key_topics: a few short topic tags.
Ground every item in the transcript. Prefer empty arrays over speculation. Return only the \
structured object.";

/// Build the post-analysis system prompt: frozen instructions + the prep notes.
pub fn analysis_system_prompt(context_notes: Option<&str>) -> String {
    match context_notes.map(str::trim).filter(|s| !s.is_empty()) {
        Some(notes) => format!("{ANALYSIS_SYSTEM}\n\nPREP NOTES:\n{notes}"),
        None => format!("{ANALYSIS_SYSTEM}\n\nPREP NOTES:\n(none provided)"),
    }
}

/// The post-analysis user turn: live-detected commitments to reconcile against,
/// then the full transcript (You/Remote labelled).
pub fn analysis_user_message(transcript: &[TranscriptEntry], live_annotations: &[String]) -> String {
    let mut s = String::new();
    if live_annotations.is_empty() {
        s.push_str("LIVE-DETECTED COMMITMENTS: (none)\n\n");
    } else {
        s.push_str("LIVE-DETECTED COMMITMENTS (reconcile, do not duplicate):\n");
        for a in live_annotations {
            s.push_str(&format!("- {a}\n"));
        }
        s.push('\n');
    }
    s.push_str("FULL TRANSCRIPT:\n");
    if transcript.is_empty() {
        s.push_str("(empty)\n");
    } else {
        for e in transcript {
            let who = match e.stream {
                StreamTag::You => "You",
                StreamTag::Remote => "Remote",
            };
            s.push_str(&format!("[{who}] {}\n", e.text.trim()));
        }
    }
    s
}

/// JSON Schema for the post-analysis `output_config.format` (D17). Same
/// structured-output rules as `findings_schema` (every object: `required` +
/// `additionalProperties:false`); `deadline` is a possibly-empty string.
pub fn analysis_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["summary", "actions", "decisions", "key_topics"],
        "properties": {
            "summary": { "type": "string" },
            "actions": { "type": "array", "items": {
                "type": "object", "additionalProperties": false,
                "required": ["title", "owner", "deadline", "transcript_quote", "type"],
                "properties": {
                    "title": { "type": "string" },
                    "owner": { "type": "string" },
                    "deadline": { "type": "string" },
                    "transcript_quote": { "type": "string" },
                    "type": { "type": "string", "enum": ["commitment", "follow_up", "suggestion"] }
                } } },
            "decisions": { "type": "array", "items": { "type": "string" } },
            "key_topics": { "type": "array", "items": { "type": "string" } }
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

    #[test]
    fn analysis_schema_is_strict_structured_output() {
        let s = analysis_schema();
        assert_eq!(s["type"], "object");
        assert_eq!(s["additionalProperties"], false);
        let action = &s["properties"]["actions"]["items"];
        assert_eq!(action["additionalProperties"], false);
        assert!(action["required"].as_array().unwrap().iter().any(|v| v == "type"));
        assert_eq!(
            action["properties"]["type"]["enum"],
            json!(["commitment", "follow_up", "suggestion"])
        );
    }

    #[test]
    fn analysis_system_prompt_embeds_notes_or_none() {
        assert!(analysis_system_prompt(Some("budget $5M")).contains("budget $5M"));
        assert!(analysis_system_prompt(None).contains("(none provided)"));
        assert!(analysis_system_prompt(Some("   ")).contains("(none provided)"));
    }

    #[test]
    fn analysis_user_message_includes_annotations_and_transcript() {
        let msg = analysis_user_message(&[e("we shipped it")], &["Sarah: send report".to_string()]);
        assert!(msg.contains("LIVE-DETECTED COMMITMENTS"));
        assert!(msg.contains("Sarah: send report"));
        assert!(msg.contains("[You] we shipped it"));
        assert!(analysis_user_message(&[e("hi")], &[]).contains("LIVE-DETECTED COMMITMENTS: (none)"));
    }
}
