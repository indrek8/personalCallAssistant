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
/// unanswered-Questions). Default is all-off (→ zero API calls until opt-in).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
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
    "medium".to_string()
}

fn default_budget() -> f64 {
    5.0
}

fn default_first_run() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_default_from_empty_object() {
        // A fresh/partial settings.json must fill the documented defaults
        // (technical-design.md §10).
        let s: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(s.whisper_model, "medium");
        assert_eq!(s.budget_default, 5.0);
        assert!(s.first_run);
        assert_eq!(s.capture_device_id, None);
        assert!(!s.default_toggles.f && !s.default_toggles.c);
    }

    #[test]
    fn toggles_default_all_off() {
        // All-off is load-bearing: it's the "zero API calls until opt-in" guarantee.
        let t: Toggles = serde_json::from_str("{}").unwrap();
        assert!(!t.f && !t.c && !t.s && !t.q);
        let d = Toggles::default();
        assert!(!d.f && !d.c && !d.s && !d.q);
    }

    #[test]
    fn settings_round_trip_preserves_fields() {
        let s = Settings {
            capture_device_id: Some("USB Mic".into()),
            whisper_model: "small".into(),
            default_toggles: Toggles { f: true, c: false, s: true, q: false },
            budget_default: 12.5,
            storage_path: None,
            first_run: false,
        };
        let back: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back.whisper_model, "small");
        assert_eq!(back.budget_default, 12.5);
        assert!(!back.first_run);
        assert!(back.default_toggles.f && back.default_toggles.s);
        assert_eq!(back.capture_device_id.as_deref(), Some("USB Mic"));
    }
}
