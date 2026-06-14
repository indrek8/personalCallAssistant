//! Serde structs for the on-disk JSON files (technical-design.md §9, §10).
//!
//! M1 covers `settings.json` and session `metadata.json` (the latter lives in
//! `session::model`). Transcript / ai_live / analysis schemas arrive with their
//! milestones.

use serde::{Deserialize, Serialize};

/// `settings.json` — app configuration (technical-design.md §10).
///
/// The API key is **never** stored here (Keychain in the shipped app); this
/// struct deliberately omits it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub capture_device_id: Option<String>,
    #[serde(default = "default_whisper_model")]
    pub whisper_model: String,
    #[serde(default)]
    pub default_toggles: Toggles,
    #[serde(default = "default_budget")]
    pub budget_default: f64,
    #[serde(default)]
    pub storage_path: Option<String>,
    #[serde(default = "default_first_run")]
    pub first_run: bool,
}

/// The live-AI feature toggles (Fact-check / Commitments / Suggestions /
/// unanswered-Questions).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Toggles {
    #[serde(default)]
    pub f: bool,
    #[serde(default)]
    pub c: bool,
    #[serde(default)]
    pub s: bool,
    #[serde(default)]
    pub q: bool,
}

impl Default for Toggles {
    fn default() -> Self {
        // Conservative default: nothing on → zero API calls until the user opts in.
        Toggles {
            f: false,
            c: false,
            s: false,
            q: false,
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            capture_device_id: None,
            whisper_model: default_whisper_model(),
            default_toggles: Toggles::default(),
            budget_default: default_budget(),
            storage_path: None,
            first_run: default_first_run(),
        }
    }
}

fn default_whisper_model() -> String {
    "small".to_string()
}

fn default_budget() -> f64 {
    5.0
}

fn default_first_run() -> bool {
    true
}
