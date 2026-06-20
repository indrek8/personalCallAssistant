//! Audio subsystem.
//!
//! - **Enumeration** (`list_input_devices`) — the real Core Audio path proven in
//!   M1; the New Session / Settings device dropdowns are populated from it.
//! - **Capture** (`capture`) — M2: two passive `cpal` input streams (mic = "you",
//!   BlackHole/Call Assistant = "remote") resampled to 16 kHz mono and written to
//!   an incremental stereo ground-truth WAV (technical-design.md §4).
//! - **WAV** (`wav`) — the incremental stereo writer + crash-recovery header repair.
//! - **VAD** (`vad`) — energy segmentation of each 16 kHz stream into utterances
//!   for the Whisper feed (M2/PR2).

pub mod capture;
pub mod vad;
pub mod wav;

use serde::{Deserialize, Serialize};

use cpal::traits::{DeviceTrait, HostTrait};
use cpal::Device;

use crate::error::{AppError, AppResult};

/// One audio input device, as returned by `list_audio_input_devices` (§7).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevice {
    /// Identifier used for selection/persistence. cpal exposes no opaque id on
    /// macOS, so this is the device name, disambiguated with an occurrence suffix
    /// when names collide. `find_input_device_by_id` resolves it back to a device.
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

/// Which side of the call a stream/utterance belongs to. The two physical capture
/// streams are known, so this is "free" speaker attribution — no diarization
/// (technical-design.md §4). Whisper transcript entries carry the same tag (PR2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StreamTag {
    /// The local user — captured from the real microphone. Written to WAV channel L.
    You,
    /// The far side — captured from the BlackHole/Call Assistant loopback. WAV channel R.
    Remote,
}

/// A 16 kHz mono chunk of resampled capture audio, tagged by side. This is the
/// hand-off from the capture worker to the STT feed (technical-design.md §2's
/// `SampleChunk`): capture tees these to VAD → Whisper (PR2).
#[derive(Debug, Clone)]
pub struct SampleChunk {
    pub tag: StreamTag,
    pub samples: Vec<f32>,
}

/// Disambiguate a repeated device name with an occurrence suffix (`Name #2`,
/// `Name #3`, …) so the persisted `id` stays unique. The first occurrence keeps
/// the bare name; `seen` carries the running counts across the enumeration.
fn disambiguated_id(name: &str, seen: &mut std::collections::HashMap<String, u32>) -> String {
    let count = seen.entry(name.to_string()).or_insert(0);
    *count += 1;
    if *count == 1 {
        name.to_string()
    } else {
        format!("{name} #{count}")
    }
}

/// Recover the display name from an `id` produced by [`disambiguated_id`] by
/// stripping a trailing ` #<digits>` occurrence suffix (a non-numeric `#` suffix
/// is left intact — it's part of the real device name).
fn display_name_from_id(id: &str) -> String {
    id.rsplit_once(" #")
        .filter(|(_, n)| n.chars().all(|c| c.is_ascii_digit()))
        .map(|(base, _)| base.to_string())
        .unwrap_or_else(|| id.to_string())
}

/// Enumerate input devices, pairing each with the stable `id` the frontend uses
/// for selection and the `is_default` flag. The single source of truth for the
/// id scheme so enumeration (`list_input_devices`) and resolution
/// (`find_input_device_by_id`) never disagree.
fn input_devices_with_ids() -> AppResult<Vec<(String, Device, bool)>> {
    let host = cpal::default_host();

    let default_name = host.default_input_device().and_then(|d| d.name().ok());

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
        let is_default = default_name.as_deref() == Some(name.as_str());
        // Disambiguate repeated names (e.g. two identical USB mics) with an
        // occurrence suffix so the id stays unique for selection/persistence.
        let id = disambiguated_id(&name, &mut seen);
        out.push((id, device, is_default));
    }

    Ok(out)
}

/// Enumerate input (capture) devices on the default host.
///
/// The default input device is flagged with `is_default = true`. This is the
/// real Core Audio path that proves Svelte ↔ Rust ↔ Core Audio.
pub fn list_input_devices() -> AppResult<Vec<AudioDevice>> {
    Ok(input_devices_with_ids()?
        .into_iter()
        .map(|(id, _device, is_default)| {
            let name = display_name_from_id(&id);
            AudioDevice {
                id,
                name,
                is_default,
            }
        })
        .collect())
}

/// Resolve a device `id` (as produced by `list_input_devices`) back to a cpal
/// `Device`. Used by capture to open the selected mic + remote streams.
pub fn find_input_device_by_id(id: &str) -> AppResult<Device> {
    input_devices_with_ids()?
        .into_iter()
        .find(|(dev_id, _, _)| dev_id == id)
        .map(|(_, device, _)| device)
        .ok_or_else(|| AppError::Audio(format!("input device not found: {id}")))
}

/// The current default input device, or an error if there is none.
pub fn default_input_device() -> AppResult<Device> {
    cpal::default_host()
        .default_input_device()
        .ok_or_else(|| AppError::Audio("no default input device".into()))
}

/// Find the BlackHole / "Call Assistant" loopback input that carries the remote
/// side of the call (name contains "blackhole" or "call assistant",
/// case-insensitive). Returns its `id`, or `None` if not installed — the caller
/// surfaces EXC-NODEV (flows.md §9). Mirrors the M0/S3 spike's detection.
pub fn find_remote_loopback_id() -> AppResult<Option<String>> {
    Ok(input_devices_with_ids()?
        .into_iter()
        .find(|(_, device, _)| {
            device
                .name()
                .map(|n| {
                    let n = n.to_lowercase();
                    n.contains("blackhole") || n.contains("call assistant")
                })
                .unwrap_or(false)
        })
        .map(|(id, _, _)| id))
}

/// The `id` of the current default input device (used for capture when the user
/// hasn't selected one). Falls back to the first input if there is no default.
pub fn default_input_id() -> AppResult<String> {
    let devices = input_devices_with_ids()?;
    devices
        .iter()
        .find(|(_, _, is_default)| *is_default)
        .or_else(|| devices.first())
        .map(|(id, _, _)| id.clone())
        .ok_or_else(|| AppError::Audio("no input devices available".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn disambiguates_repeated_names() {
        let mut seen = HashMap::new();
        assert_eq!(disambiguated_id("USB Mic", &mut seen), "USB Mic");
        assert_eq!(disambiguated_id("USB Mic", &mut seen), "USB Mic #2");
        assert_eq!(disambiguated_id("USB Mic", &mut seen), "USB Mic #3");
        // A different name starts its own count.
        assert_eq!(disambiguated_id("Other", &mut seen), "Other");
    }

    #[test]
    fn id_to_display_name_round_trip() {
        assert_eq!(display_name_from_id("USB Mic"), "USB Mic");
        assert_eq!(display_name_from_id("USB Mic #2"), "USB Mic");
        assert_eq!(display_name_from_id("USB Mic #10"), "USB Mic");
        // A non-numeric '#' suffix is part of the real name, not an occurrence tag.
        assert_eq!(display_name_from_id("Track #A"), "Track #A");
    }

    #[test]
    fn enumeration_ids_stay_unique() {
        // The id scheme must stay self-consistent so find_input_device_by_id can
        // resolve every id list_input_devices produced.
        let mut seen = HashMap::new();
        let ids: Vec<String> = ["Mic", "Mic", "BlackHole", "Mic"]
            .iter()
            .map(|n| disambiguated_id(n, &mut seen))
            .collect();
        assert_eq!(ids, vec!["Mic", "Mic #2", "BlackHole", "Mic #3"]);
        let mut uniq = ids.clone();
        uniq.sort();
        uniq.dedup();
        assert_eq!(uniq.len(), ids.len(), "device ids must be unique");
    }
}
