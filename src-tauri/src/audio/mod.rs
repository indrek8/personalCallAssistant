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
    /// Identifier used for selection/persistence. cpal exposes no opaque id on
    /// macOS, so this is the device name, disambiguated with an occurrence suffix
    /// when names collide. Stable Core Audio device IDs come in M2.
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
    let mut seen: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    for device in devices {
        let name = match device.name() {
            Ok(n) => n,
            Err(_) => continue, // skip devices we can't name
        };
        let is_default = default_name
            .as_deref()
            .map(|d| d == name)
            .unwrap_or(false);
        // cpal exposes no opaque id on macOS, so the id is the device name —
        // disambiguated with an occurrence suffix when names repeat (e.g. two
        // identical USB mics) so the id stays unique for selection/persistence.
        // Stable Core Audio device IDs arrive in M2 when we open the device.
        let count = seen.entry(name.clone()).or_insert(0);
        *count += 1;
        let id = if *count == 1 {
            name.clone()
        } else {
            format!("{name} #{count}")
        };
        out.push(AudioDevice { id, name, is_default });
    }

    Ok(out)
}
