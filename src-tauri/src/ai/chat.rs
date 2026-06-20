//! Ask-AI chat over Sonnet, streamed (D13/D15; flows §5 C3).
//!
//! Runs on a blocking thread (spawned by the `ask_ai` command): reads the
//! session's transcript + prep notes from disk, streams a Sonnet answer over the
//! question, emits `ai-chat-token` per delta and `ai-chat-done` at the end, and
//! logs the turn to `chat.json`.

use serde_json::json;
use tauri::AppHandle;

use crate::ai::{ClaudeClient, MODEL_SONNET};
use crate::audio::StreamTag;
use crate::error::{AppError, AppResult};
use crate::events;
use crate::session::model::TranscriptEntry;
use crate::storage;

/// Output budget for a chat answer.
const MAX_TOKENS: u32 = 4096;

/// Run one Ask-AI turn. Returns the full answer + this turn's cost.
pub fn ask(app: &AppHandle, session_id: &str, question: &str) -> AppResult<(String, f64)> {
    let client = ClaudeClient::from_stored()?;
    let transcript = storage::read_transcript_at(&storage::transcript_path(session_id)?)?;
    let meta = storage::get_session_meta(session_id)?;

    let system = chat_system(meta.context_notes.as_deref());
    let user = format!(
        "{}\n\nQUESTION: {}",
        transcript_block(&transcript),
        question.trim()
    );
    let body = json!({
        "model": MODEL_SONNET,
        "max_tokens": MAX_TOKENS,
        "stream": true,
        "system": system,
        "messages": [{ "role": "user", "content": user }],
    });

    let app_tok = app.clone();
    let outcome = client.stream_text(&body, |tok| {
        events::emit(&app_tok, events::AI_CHAT_TOKEN, json!({ "token": tok }));
    })?;

    // Surface refusals / truncation rather than presenting a partial or empty
    // answer as if it were complete (a stopped stream is still an HTTP 200).
    let answer = finalize_answer(outcome.text, outcome.stop_reason.as_deref())?;
    let cost = outcome.usage.cost(MODEL_SONNET);
    events::emit(
        app,
        events::AI_CHAT_DONE,
        json!({ "answer": answer, "cost": cost }),
    );

    let _ = storage::append_json_line(
        &storage::chat_path(session_id)?,
        &json!({
            "question": question,
            "answer": answer,
            "tokens_in": outcome.usage.input_tokens,
            "tokens_out": outcome.usage.output_tokens,
            "cost": cost,
        }),
    );
    Ok((answer, cost))
}

/// Map a streamed answer + its `stop_reason` to the text shown to the user.
/// Refusals and length cuts are flagged in-band; a stream that ended with no
/// usable text and no clean terminal reason is a hard error.
fn finalize_answer(text: String, stop_reason: Option<&str>) -> AppResult<String> {
    match stop_reason {
        Some("end_turn") | Some("stop_sequence") => Ok(text),
        // A refusal carries no usable answer — discard any partial output.
        Some("refusal") => Ok("(The assistant declined to answer this question.)".to_string()),
        Some("max_tokens") => {
            Ok(format!("{text}\n\n(Answer truncated — it reached the length limit.)"))
        }
        // No terminal frame (dropped/incomplete stream) or an unexpected reason.
        other => {
            if text.trim().is_empty() {
                Err(AppError::Api(format!(
                    "the answer stream ended before any response{}",
                    other
                        .map(|r| format!(" (stop_reason: {r})"))
                        .unwrap_or_default()
                )))
            } else {
                Ok(format!(
                    "{text}\n\n(The answer may be incomplete — the response ended early.)"
                ))
            }
        }
    }
}

fn chat_system(notes: Option<&str>) -> String {
    let base = "You are the user's meeting copilot. Answer the user's question using only the \
call transcript and prep notes below. Be concise and direct, and quote the transcript when it \
helps. If the transcript doesn't contain the answer, say so plainly rather than guessing.";
    match notes.map(str::trim).filter(|s| !s.is_empty()) {
        Some(n) => format!("{base}\n\nPREP NOTES:\n{n}"),
        None => base.to_string(),
    }
}

fn transcript_block(entries: &[TranscriptEntry]) -> String {
    if entries.is_empty() {
        return "TRANSCRIPT: (nothing has been transcribed yet)".to_string();
    }
    let mut s = String::from("TRANSCRIPT SO FAR:\n");
    for e in entries {
        let who = match e.stream {
            StreamTag::You => "You",
            StreamTag::Remote => "Remote",
        };
        s.push_str(&format!("[{who}] {}\n", e.text.trim()));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_includes_notes_when_present() {
        assert!(chat_system(Some("budget is $5M")).contains("budget is $5M"));
        assert!(!chat_system(None).contains("PREP NOTES"));
        // Blank notes are treated as none.
        assert!(!chat_system(Some("   ")).contains("PREP NOTES"));
    }

    #[test]
    fn transcript_block_labels_both_sides() {
        let entries = vec![
            TranscriptEntry {
                id: "1".into(),
                t_ms: 0,
                stream: StreamTag::You,
                text: "hi".into(),
                confidence: 1.0,
            },
            TranscriptEntry {
                id: "2".into(),
                t_ms: 1,
                stream: StreamTag::Remote,
                text: "hello".into(),
                confidence: 1.0,
            },
        ];
        let b = transcript_block(&entries);
        assert!(b.contains("[You] hi"));
        assert!(b.contains("[Remote] hello"));
    }

    #[test]
    fn empty_transcript_is_noted() {
        assert!(transcript_block(&[]).contains("nothing has been transcribed"));
    }

    #[test]
    fn finalize_answer_passes_through_normal_completion() {
        assert_eq!(finalize_answer("hi".into(), Some("end_turn")).unwrap(), "hi");
        assert_eq!(finalize_answer("hi".into(), Some("stop_sequence")).unwrap(), "hi");
    }

    #[test]
    fn finalize_answer_flags_refusal_and_discards_partial() {
        let a = finalize_answer("leaked partial".into(), Some("refusal")).unwrap();
        assert!(a.contains("declined"));
        assert!(!a.contains("leaked partial"));
    }

    #[test]
    fn finalize_answer_notes_truncation_but_keeps_text() {
        let a = finalize_answer("a long answer".into(), Some("max_tokens")).unwrap();
        assert!(a.starts_with("a long answer"));
        assert!(a.contains("truncated"));
    }

    #[test]
    fn finalize_answer_errors_on_empty_incomplete_stream() {
        assert!(finalize_answer(String::new(), None).is_err());
    }

    #[test]
    fn finalize_answer_keeps_partial_on_incomplete_stream() {
        let a = finalize_answer("got this far".into(), None).unwrap();
        assert!(a.starts_with("got this far"));
        assert!(a.contains("incomplete"));
    }
}
