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
use crate::error::AppResult;
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
    let (answer, usage) = client.stream_text(&body, |tok| {
        events::emit(&app_tok, events::AI_CHAT_TOKEN, json!({ "token": tok }));
    })?;

    let cost = usage.cost(MODEL_SONNET);
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
            "tokens_in": usage.input_tokens,
            "tokens_out": usage.output_tokens,
            "cost": cost,
        }),
    );
    Ok((answer, cost))
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
}
