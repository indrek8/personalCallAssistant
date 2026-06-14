//! S4 · Claude API spike.
//!
//! Loads `ANTHROPIC_API_KEY` from the gitignored root `.env` (via `dotenvy`),
//! then makes one minimal Claude Messages API call to a Haiku model and one to
//! a Sonnet model (raw HTTP via `reqwest` + `serde_json`). Prints the parsed
//! text plus input/output tokens and the computed cost.
//!
//! There is no official Anthropic Rust SDK, so we hit the REST endpoint
//! directly — this is exactly the shape `ai/mod.rs` will use in M3 (§6).
//!
//! Usage:
//!   cargo run --bin s4_claude
//!
//! Setup (see RUN.md): put your key in the repo-root `.env`:
//!   cp .env.example .env   # then edit ANTHROPIC_API_KEY
//!
//! Model IDs and pricing (USD per million tokens) are current as of the spike
//! (verified against the claude-api reference):
//!   - Haiku  4.5  `claude-haiku-4-5`   $1.00 in / $5.00 out
//!   - Sonnet 4.6  `claude-sonnet-4-6`  $3.00 in / $15.00 out

use serde::Deserialize;

/// Absolute path to the repo-root .env (the spikes dir is one level down).
const ROOT_ENV: &str = "/Users/indrek/Development/personalCallAssistant/.env";
const API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// (model_id, input $/Mtok, output $/Mtok)
struct ModelSpec {
    id: &'static str,
    label: &'static str,
    in_per_mtok: f64,
    out_per_mtok: f64,
}

const HAIKU: ModelSpec = ModelSpec {
    id: "claude-haiku-4-5",
    label: "Haiku 4.5",
    in_per_mtok: 1.00,
    out_per_mtok: 5.00,
};
const SONNET: ModelSpec = ModelSpec {
    id: "claude-sonnet-4-6",
    label: "Sonnet 4.6",
    in_per_mtok: 3.00,
    out_per_mtok: 15.00,
};

#[derive(Deserialize)]
struct MessagesResponse {
    content: Vec<ContentBlock>,
    usage: Usage,
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    #[serde(default)]
    text: String,
}

#[derive(Deserialize)]
struct Usage {
    input_tokens: u64,
    output_tokens: u64,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("s4_claude error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    // Load the gitignored root .env. `dotenvy::from_path` reads the file even
    // when cwd is the spikes/ subdir.
    if let Err(e) = dotenvy::from_path(ROOT_ENV) {
        eprintln!(
            "warning: could not load {ROOT_ENV}: {e}\n\
             (falling back to any ANTHROPIC_API_KEY already in the environment)"
        );
    }

    let api_key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| {
        format!(
            "ANTHROPIC_API_KEY not set. Put it in {ROOT_ENV}:\n  \
             cp .env.example .env   # then edit the key"
        )
    })?;
    if api_key.contains("REPLACE_ME") || api_key.trim().is_empty() {
        return Err(format!(
            "ANTHROPIC_API_KEY still looks like the placeholder. Edit {ROOT_ENV}."
        ));
    }

    let client = reqwest::blocking::Client::new();
    let prompt = "Reply with exactly: spike ok";

    call_model(&client, &api_key, &HAIKU, prompt)?;
    println!();
    call_model(&client, &api_key, &SONNET, prompt)?;

    Ok(())
}

fn call_model(
    client: &reqwest::blocking::Client,
    api_key: &str,
    model: &ModelSpec,
    prompt: &str,
) -> Result<(), String> {
    let body = serde_json::json!({
        "model": model.id,
        "max_tokens": 64,
        "messages": [{ "role": "user", "content": prompt }],
    });

    let resp = client
        .post(API_URL)
        .header("x-api-key", api_key)
        .header("anthropic-version", ANTHROPIC_VERSION)
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .map_err(|e| format!("[{}] request failed: {e}", model.label))?;

    let status = resp.status();
    let text = resp
        .text()
        .map_err(|e| format!("[{}] reading body failed: {e}", model.label))?;

    if !status.is_success() {
        return Err(format!(
            "[{}] HTTP {status}:\n{text}",
            model.label
        ));
    }

    let parsed: MessagesResponse = serde_json::from_str(&text)
        .map_err(|e| format!("[{}] parse failed: {e}\nbody: {text}", model.label))?;

    let reply: String = parsed
        .content
        .iter()
        .filter(|b| b.block_type == "text")
        .map(|b| b.text.as_str())
        .collect::<Vec<_>>()
        .join("");

    let cost = parsed.usage.input_tokens as f64 / 1_000_000.0 * model.in_per_mtok
        + parsed.usage.output_tokens as f64 / 1_000_000.0 * model.out_per_mtok;

    println!("--- {} ({}) ---", model.label, model.id);
    println!("reply:  {}", reply.trim());
    println!(
        "tokens: in={}, out={}",
        parsed.usage.input_tokens, parsed.usage.output_tokens
    );
    println!("cost:   ${cost:.6}");

    Ok(())
}

