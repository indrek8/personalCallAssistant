//! Audio subsystem.
//!
//! M1 implements **only** real input-device enumeration via `cpal` (the M1
//! acceptance check: the device dropdown is populated by Rust, not hardcoded —
//! milestones.md §M1). Capture / resample / WAV / VAD are stubbed for M2.

use serde::{Deserialize, Serialize};

use cpal::traits::{DeviceTrait, HostTrait};

use crate::error::{AppError, AppResult};

/// One audio input device, as returned by `list_audio_input_devices` (§7).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevice {
    /// Stable-ish identifier. cpal exposes no opaque id on macOS, so we use the
    /// device name (unique enough for the device dropdown in M1).
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

/// Enumerate input (capture) devices on the default host.
///
/// The default input device is flagged with `is_default = true`. This is the
/// real Core Audio path that proves Svelte ↔ Rust ↔ Core Audio.
pub fn list_input_devices() -> AppResult<Vec<AudioDevice>> {
    let host = cpal::default_host();

    let default_name = host
        .default_input_device()
        .and_then(|d| d.name().ok());

    let devices = host
        .input_devices()
        .map_err(|e| AppError::Audio(format!("could not enumerate input devices: {e}")))?;

    let mut out = Vec::new();
    for device in devices {
        let name = match device.name() {
            Ok(n) => n,
            Err(_) => continue, // skip devices we can't name
        };
        let is_default = default_name
            .as_deref()
            .map(|d| d == name)
            .unwrap_or(false);
        out.push(AudioDevice {
            id: name.clone(),
            name,
            is_default,
        });
    }

    Ok(out)
}
