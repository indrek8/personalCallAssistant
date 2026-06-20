//! Live-analysis batcher (D12/D15; technical-design §6; flows §5 C2).
//!
//! A dedicated std thread is fed finalized [`TranscriptEntry`]s (teed off the STT
//! forwarder by the SessionManager). It keeps a rolling history, fires a Haiku
//! batch when **≥5 new entries OR ≥30 s** have passed with ≥1 toggle on, parses
//! the structured-JSON findings, appends a crash-safe record to `ai_live.json`,
//! and emits `ai-finding` + `cost-update`. The whole thing is best-effort: a
//! missing key or an HTTP error never touches the capture/transcript path.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossbeam_channel::{unbounded, Receiver, RecvTimeoutError, Sender};
use serde::Deserialize;
use serde_json::{json, Value};
use tauri::AppHandle;
use uuid::Uuid;

use crate::ai::{prompts, ClaudeClient, MODEL_HAIKU};
use crate::events;
use crate::session::model::TranscriptEntry;
use crate::storage;
use crate::storage::schema::Toggles;

/// Fire when this many new entries have accumulated since the last batch...
const FIRE_ON_ENTRIES: usize = 5;
/// ...or this long has elapsed with ≥1 new entry.
const FIRE_AFTER: Duration = Duration::from_secs(30);
/// Consecutive HTTP failures after which live AI auto-disables (EXC-API-LIVE).
const MAX_LIVE_FAILURES: u32 = 3;
/// Cap on retained history (the 3-min window is the prompt; this bounds memory).
const HISTORY_CAP: usize = 400;
/// Findings response budget — large enough that the JSON is never truncated.
const MAX_TOKENS: u32 = 2048;

/// Everything the batcher thread needs for one session.
pub struct AiConfig {
    pub session_id: String,
    pub app: AppHandle,
    pub context_notes: Option<String>,
    pub budget_cap: Option<f64>,
    /// Shared with the SessionManager so `set_toggles` lands on the next batch.
    pub toggles: Arc<Mutex<Toggles>>,
    /// Shared running cost total (persisted to metadata on End).
    pub cost: Arc<Mutex<f64>>,
    pub ai_live_path: PathBuf,
}

/// Handle to the running batcher. Feed entries via [`AiBatcher::sender`].
pub struct AiBatcher {
    entry_tx: Sender<TranscriptEntry>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl AiBatcher {
    /// Spawn the batcher. **Never fails** — a missing key or HTTP error must not
    /// break capture (acceptance: live-AI failures don't interrupt the transcript).
    pub fn start(cfg: AiConfig) -> Self {
        let (entry_tx, entry_rx) = unbounded::<TranscriptEntry>();
        let stop = Arc::new(AtomicBool::new(false));
        let worker = {
            let stop = stop.clone();
            thread::Builder::new()
                .name("ai-batcher".into())
                .spawn(move || run_batcher(cfg, entry_rx, stop))
                .ok()
        };
        Self {
            entry_tx,
            stop,
            worker,
        }
    }

    /// A sender the SessionManager tees finalized transcript entries into.
    pub fn sender(&self) -> Sender<TranscriptEntry> {
        self.entry_tx.clone()
    }

    /// Stop the batcher and join its thread.
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.worker.take() {
            let _ = h.join();
        }
    }
}

impl Drop for AiBatcher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.worker.take() {
            let _ = h.join();
        }
    }
}

fn run_batcher(cfg: AiConfig, entry_rx: Receiver<TranscriptEntry>, stop: Arc<AtomicBool>) {
    // Resolve the key once. No key → idle (capture is unaffected); notify once.
    let client = match ClaudeClient::from_stored() {
        Ok(c) => c,
        Err(_) => {
            events::emit(
                &cfg.app,
                events::APP_ERROR,
                json!({
                    "code": "EXC-KEY",
                    "message": "No Claude API key — live AI is off for this session.",
                    "recoverable": true,
                }),
            );
            drain_until_stop(&entry_rx, &stop);
            return;
        }
    };

    let system = prompts::system_prompt(cfg.context_notes.as_deref());
    let schema = prompts::findings_schema();

    let mut history: VecDeque<TranscriptEntry> = VecDeque::new();
    let mut pending: usize = 0;
    let mut last_fire = Instant::now();
    let mut failures: u32 = 0;
    let mut disabled = false; // budget hit or too many consecutive failures

    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        match entry_rx.recv_timeout(Duration::from_millis(400)) {
            Ok(entry) => {
                history.push_back(entry);
                while history.len() > HISTORY_CAP {
                    history.pop_front();
                }
                pending += 1;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        if disabled {
            pending = 0;
            continue;
        }

        let toggles = *cfg.toggles.lock().unwrap();
        if !any_on(&toggles) {
            // Toggles off → zero API calls. Keep history for context, reset the
            // timers so turning a toggle on starts a fresh batch (no retroactive
            // analysis — flows §5 C5).
            pending = 0;
            last_fire = Instant::now();
            continue;
        }

        let should_fire =
            pending >= FIRE_ON_ENTRIES || (pending >= 1 && last_fire.elapsed() >= FIRE_AFTER);
        if !should_fire {
            continue;
        }

        let window = recent_window(&history);
        match run_batch(&client, &cfg, &system, &schema, &toggles, &window) {
            Ok(()) => failures = 0,
            Err(BatchError::Discard(msg)) => {
                // The HTTP call succeeded but the body didn't parse — drop this
                // batch and keep going (not an EXC-API-LIVE failure).
                eprintln!("[ai] discarded batch: {msg}");
            }
            Err(BatchError::Api(msg)) => {
                failures += 1;
                eprintln!("[ai] live call failed ({failures}/{MAX_LIVE_FAILURES}): {msg}");
                if failures >= MAX_LIVE_FAILURES {
                    disabled = true;
                    emit_error(
                        &cfg,
                        "EXC-API-LIVE",
                        "Live AI paused after repeated errors. The transcript keeps recording.",
                    );
                }
            }
        }
        pending = 0;
        last_fire = Instant::now();

        // Budget guard (EXC-BUDGET): pause live AI; transcript continues.
        if let Some(cap) = cfg.budget_cap {
            if cap > 0.0 && *cfg.cost.lock().unwrap() >= cap {
                disabled = true;
                emit_error(
                    &cfg,
                    "EXC-BUDGET",
                    &format!("Budget of ${cap:.2} reached — live AI paused. Transcript continues."),
                );
            }
        }
    }
}

fn any_on(t: &Toggles) -> bool {
    t.f || t.c || t.s || t.q
}

fn emit_error(cfg: &AiConfig, code: &str, message: &str) {
    events::emit(
        &cfg.app,
        events::APP_ERROR,
        json!({ "code": code, "message": message, "recoverable": true }),
    );
}

/// Keep draining (so the channel never backs up) until told to stop.
fn drain_until_stop(entry_rx: &Receiver<TranscriptEntry>, stop: &Arc<AtomicBool>) {
    while !stop.load(Ordering::Relaxed) {
        if let Err(RecvTimeoutError::Disconnected) =
            entry_rx.recv_timeout(Duration::from_millis(250))
        {
            break;
        }
    }
}

/// One batch failure category.
enum BatchError {
    /// HTTP / transport error — counts toward auto-disable.
    Api(String),
    /// Response couldn't be parsed into findings — drop and continue.
    Discard(String),
}

#[allow(clippy::too_many_arguments)]
fn run_batch(
    client: &ClaudeClient,
    cfg: &AiConfig,
    system: &str,
    schema: &Value,
    toggles: &Toggles,
    window: &[TranscriptEntry],
) -> Result<(), BatchError> {
    if window.is_empty() {
        return Ok(());
    }
    let user = prompts::user_message(toggles, window);
    let body = json!({
        "model": MODEL_HAIKU,
        "max_tokens": MAX_TOKENS,
        "system": [{ "type": "text", "text": system, "cache_control": { "type": "ephemeral" } }],
        "messages": [{ "role": "user", "content": user }],
        "output_config": { "format": { "type": "json_schema", "schema": schema } },
    });

    let started = Instant::now();
    let resp = client
        .messages(&body)
        .map_err(|e| BatchError::Api(e.to_string()))?;
    let latency_ms = started.elapsed().as_millis() as u64;

    let findings: Findings = serde_json::from_str(&resp.text())
        .map_err(|e| BatchError::Discard(format!("schema parse: {e}")))?;

    let t_ms = window.last().map(|e| e.t_ms).unwrap_or(0);
    let normalized = findings.normalize(t_ms);

    // Cost: update the running total, emit cost-update for the live meter.
    let last = resp.usage.cost(MODEL_HAIKU);
    let total = {
        let mut c = cfg.cost.lock().unwrap();
        *c += last;
        *c
    };
    events::emit(
        &cfg.app,
        events::COST_UPDATE,
        json!({ "session_id": cfg.session_id, "total": total, "last": last }),
    );

    // Crash-safe call record (cost + findings) appended to ai_live.json.
    let record = json!({
        "t_ms": t_ms,
        "model": MODEL_HAIKU,
        "tokens_in": resp.usage.input_tokens,
        "tokens_out": resp.usage.output_tokens,
        "cache_read": resp.usage.cache_read_input_tokens,
        "cost": last,
        "latency_ms": latency_ms,
        "findings": normalized,
    });
    let _ = storage::append_json_line(&cfg.ai_live_path, &record);

    for finding in normalized {
        events::emit(
            &cfg.app,
            events::AI_FINDING,
            json!({ "session_id": cfg.session_id, "finding": finding }),
        );
    }
    Ok(())
}

/// The last ~3 minutes of history, by entry timestamp.
fn recent_window(history: &VecDeque<TranscriptEntry>) -> Vec<TranscriptEntry> {
    let Some(latest) = history.back() else {
        return Vec::new();
    };
    let cutoff = latest.t_ms.saturating_sub(prompts::WINDOW_MS);
    history
        .iter()
        .filter(|e| e.t_ms >= cutoff)
        .cloned()
        .collect()
}

// --- Parsed findings (D12 schema) → normalized feed items -------------------

#[derive(Deserialize, Default)]
struct Findings {
    #[serde(default)]
    fact_checks: Vec<FactCheck>,
    #[serde(default)]
    commitments: Vec<Commitment>,
    #[serde(default)]
    suggestions: Vec<String>,
    #[serde(default)]
    unanswered_questions: Vec<String>,
}

#[derive(Deserialize)]
struct FactCheck {
    #[serde(default)]
    claim: String,
    #[serde(default)]
    assessment: String,
    #[serde(default)]
    severity: String,
}

#[derive(Deserialize)]
struct Commitment {
    #[serde(default)]
    who: String,
    #[serde(default)]
    what: String,
    #[serde(default)]
    by_when: String,
}

impl Findings {
    /// Flatten into uniform feed items with ids + a shared timestamp, dropping
    /// any blank entries the model emitted.
    fn normalize(self, t_ms: u64) -> Vec<Value> {
        let mut out = Vec::new();
        for f in self.fact_checks {
            if f.claim.trim().is_empty() && f.assessment.trim().is_empty() {
                continue;
            }
            let severity = if f.severity.is_empty() {
                "info".to_string()
            } else {
                f.severity
            };
            out.push(json!({
                "id": Uuid::new_v4().to_string(), "kind": "fact", "t_ms": t_ms,
                "title": f.claim, "detail": f.assessment, "severity": severity,
            }));
        }
        for c in self.commitments {
            if c.what.trim().is_empty() {
                continue;
            }
            out.push(json!({
                "id": Uuid::new_v4().to_string(), "kind": "commitment", "t_ms": t_ms,
                "title": c.what, "who": c.who, "by_when": c.by_when,
            }));
        }
        for s in self.suggestions {
            if s.trim().is_empty() {
                continue;
            }
            out.push(json!({
                "id": Uuid::new_v4().to_string(), "kind": "suggestion", "t_ms": t_ms, "title": s,
            }));
        }
        for q in self.unanswered_questions {
            if q.trim().is_empty() {
                continue;
            }
            out.push(json!({
                "id": Uuid::new_v4().to_string(), "kind": "question", "t_ms": t_ms, "title": q,
            }));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::StreamTag;

    fn entry(t_ms: u64, text: &str) -> TranscriptEntry {
        TranscriptEntry {
            id: "x".into(),
            t_ms,
            stream: StreamTag::You,
            text: text.into(),
            confidence: 1.0,
        }
    }

    #[test]
    fn window_keeps_last_three_minutes() {
        let mut h = VecDeque::new();
        h.push_back(entry(0, "old"));
        h.push_back(entry(100_000, "mid"));
        h.push_back(entry(250_000, "new")); // latest → cutoff = 250k-180k = 70k
        let w = recent_window(&h);
        assert_eq!(w.len(), 2, "old (0 ms) should be outside the window");
        assert_eq!(w[0].text, "mid");
    }

    #[test]
    fn findings_parse_and_normalize() {
        let f: Findings = serde_json::from_str(
            r#"{
            "fact_checks":[{"claim":"Q2","assessment":"actually Q3","severity":"warning"}],
            "commitments":[{"who":"Ahmed","what":"send report","by_when":"Fri"}],
            "suggestions":["ask about budget"],
            "unanswered_questions":[]
        }"#,
        )
        .unwrap();
        let items = f.normalize(1000);
        assert_eq!(items.len(), 3);
        assert_eq!(items[0]["kind"], "fact");
        assert_eq!(items[0]["severity"], "warning");
        assert_eq!(items[1]["kind"], "commitment");
        assert_eq!(items[1]["who"], "Ahmed");
        assert_eq!(items[2]["kind"], "suggestion");
    }

    #[test]
    fn empty_findings_normalize_to_nothing() {
        assert!(Findings::default().normalize(0).is_empty());
    }

    #[test]
    fn blank_items_are_dropped() {
        let f: Findings = serde_json::from_str(
            r#"{
            "fact_checks":[], "commitments":[{"who":"","what":"  ","by_when":""}],
            "suggestions":["", "real"], "unanswered_questions":[]
        }"#,
        )
        .unwrap();
        let items = f.normalize(0);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["title"], "real");
    }

    #[test]
    fn missing_severity_defaults_to_info() {
        let f: Findings = serde_json::from_str(
            r#"{"fact_checks":[{"claim":"c","assessment":"a","severity":""}],
                "commitments":[],"suggestions":[],"unanswered_questions":[]}"#,
        )
        .unwrap();
        assert_eq!(f.normalize(0)[0]["severity"], "info");
    }
}
