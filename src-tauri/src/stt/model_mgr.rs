//! Whisper model management (technical-design.md §10).
//!
//! ggml model blobs live under `…/CallAssistant/models/ggml-{name}.bin` and are
//! **not** bundled (142 MB – 1.4 GB). They are downloaded on demand from the
//! official whisper.cpp Hugging Face repo, **resumably** — the 1.4 GB `medium`
//! pull commonly hits `connection reset`, so we resume in place and retry,
//! mirroring the resilient pattern documented in `spikes/models/models.md`.

use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::storage;

/// Models the app offers (smallest → most accurate). `medium` is the default
/// (validated real-time in M0/S1); `base`/`small` are faster fallbacks.
pub const MODELS: [&str; 3] = ["base", "small", "medium"];

const HF_BASE: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";
/// ggml file magic: the u32 `0x67676d6c` ("ggml"), little-endian on disk.
const GGML_MAGIC_LE: [u8; 4] = [0x6c, 0x6d, 0x67, 0x67];
const MAX_DOWNLOAD_ATTEMPTS: u32 = 50;

/// `…/CallAssistant/models/`.
pub fn models_dir() -> AppResult<PathBuf> {
    let dir = storage::base_dir()?.join("models");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// ggml filename for a model name, e.g. `small` → `ggml-small.bin`.
pub fn model_filename(name: &str) -> String {
    format!("ggml-{name}.bin")
}

/// Absolute path to a model's blob (whether or not it exists yet).
pub fn model_path(name: &str) -> AppResult<PathBuf> {
    Ok(models_dir()?.join(model_filename(name)))
}

/// Download URL for a model on the whisper.cpp HF repo.
pub fn model_url(name: &str) -> String {
    format!("{HF_BASE}/{}", model_filename(name))
}

/// Whether a file looks like a valid (complete-enough) ggml model: present and
/// starting with the ggml magic. Guards against using a half-finished download.
pub fn is_valid_ggml(path: &std::path::Path) -> bool {
    let mut head = [0u8; 4];
    matches!(
        fs::File::open(path).and_then(|mut f| f.read_exact(&mut head)),
        Ok(()) if head == GGML_MAGIC_LE
    )
}

/// Download/validity status of a model, surfaced to Settings/onboarding (PR3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStatus {
    pub name: String,
    /// Display name, e.g. "Small".
    pub label: String,
    /// Approximate download size in MB (for setup/Settings estimates).
    pub approx_mb: u32,
    /// Short speed / quality note shown next to the option.
    pub speed_note: String,
    /// Whether this model is shown in the onboarding/Settings picker. `base` is
    /// hidden — `small` is the floor, `medium` the recommended default.
    pub offered: bool,
    pub downloaded: bool,
    pub size_bytes: u64,
    pub path: String,
}

/// Static catalog metadata: `(label, approx_mb, speed_note, offered)`. Speeds
/// are the M0/S1 measurements on Apple Silicon.
fn model_info(name: &str) -> (&'static str, u32, &'static str, bool) {
    match name {
        "base" => ("Base", 142, "fastest · lower accuracy", false),
        "small" => ("Small", 466, "~25× real-time · balanced", true),
        "medium" => ("Medium", 1500, "~18× real-time · best accuracy", true),
        _ => ("Custom", 0, "", false),
    }
}

/// Status of a single model (catalog metadata + on-disk presence).
pub fn model_status(name: &str) -> AppResult<ModelStatus> {
    let path = model_path(name)?;
    let size_bytes = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let (label, approx_mb, speed_note, offered) = model_info(name);
    Ok(ModelStatus {
        name: name.to_string(),
        label: label.to_string(),
        approx_mb,
        speed_note: speed_note.to_string(),
        offered,
        downloaded: is_valid_ggml(&path),
        size_bytes,
        path: path.to_string_lossy().to_string(),
    })
}

/// Status of every offered model.
pub fn list_models() -> AppResult<Vec<ModelStatus>> {
    MODELS.iter().map(|n| model_status(n)).collect()
}

/// Download `name`'s model if not already present, resuming + retrying on
/// network errors. `progress(downloaded, total)` is called as bytes arrive
/// (`total` is `None` until the server reports a size). Downloads to a `.part`
/// file and renames into place only after the ggml magic validates.
pub fn download_model(
    name: &str,
    progress: impl FnMut(u64, Option<u64>),
) -> AppResult<()> {
    if !MODELS.contains(&name) {
        return Err(AppError::Model(format!("unknown model: {name}")));
    }
    let final_path = model_path(name)?;
    if is_valid_ggml(&final_path) {
        return Ok(()); // already have it
    }
    let part = final_path.with_extension("bin.part");
    download_to_part(&model_url(name), &part, progress)?;

    if !is_valid_ggml(&part) {
        let _ = fs::remove_file(&part);
        return Err(AppError::Model(format!(
            "downloaded {name} model failed ggml validation"
        )));
    }
    fs::rename(&part, &final_path)?;
    Ok(())
}

fn download_to_part(
    url: &str,
    part: &std::path::Path,
    mut progress: impl FnMut(u64, Option<u64>),
) -> AppResult<()> {
    let client = reqwest::blocking::Client::builder()
        .build()
        .map_err(|e| AppError::Model(format!("http client: {e}")))?;

    let mut attempts = 0;
    loop {
        attempts += 1;
        match try_download_once(&client, url, part, &mut progress) {
            Ok(true) => return Ok(()),
            Ok(false) | Err(_) if attempts < MAX_DOWNLOAD_ATTEMPTS => {
                // Resume from the partial file on the next attempt.
                thread::sleep(Duration::from_millis(500));
            }
            Ok(false) => {
                return Err(AppError::Model(
                    "download incomplete after maximum retries".into(),
                ))
            }
            Err(e) => return Err(e),
        }
    }
}

/// One download attempt. Resumes from the current `.part` length via a `Range`
/// request; returns `Ok(true)` when the file is complete.
fn try_download_once(
    client: &reqwest::blocking::Client,
    url: &str,
    part: &std::path::Path,
    progress: &mut impl FnMut(u64, Option<u64>),
) -> AppResult<bool> {
    let mut have = fs::metadata(part).map(|m| m.len()).unwrap_or(0);

    let mut req = client.get(url);
    if have > 0 {
        req = req.header(reqwest::header::RANGE, format!("bytes={have}-"));
    }
    let mut resp = req
        .send()
        .map_err(|e| AppError::Model(format!("request failed: {e}")))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(AppError::Model(format!("server returned HTTP {status}")));
    }

    let resumed = status.as_u16() == 206;
    let total = total_size(&resp, have, resumed);

    // If the server ignored our Range (200 on a resume), start the file over.
    let mut file = if resumed && have > 0 {
        OpenOptions::new().append(true).open(part)?
    } else {
        have = 0;
        OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(part)?
    };

    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = resp
            .read(&mut buf)
            .map_err(|e| AppError::Model(format!("read failed: {e}")))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        have += n as u64;
        progress(have, total);
    }
    file.flush()?;

    Ok(total.map(|t| have >= t).unwrap_or(true))
}

/// Total expected size: from `Content-Range` on a 206 resume, else `Content-Length`.
fn total_size(resp: &reqwest::blocking::Response, _have: u64, resumed: bool) -> Option<u64> {
    if resumed {
        let cr = resp.headers().get(reqwest::header::CONTENT_RANGE)?;
        // "bytes start-end/total"
        cr.to_str().ok()?.rsplit('/').next()?.trim().parse().ok()
    } else {
        resp.headers()
            .get(reqwest::header::CONTENT_LENGTH)?
            .to_str()
            .ok()?
            .parse()
            .ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn filename_and_url() {
        assert_eq!(model_filename("small"), "ggml-small.bin");
        let url = model_url("medium");
        assert!(url.starts_with("https://huggingface.co/"));
        assert!(url.ends_with("/ggml-medium.bin"));
    }

    #[test]
    fn catalog_offers_small_and_medium_not_base() {
        // Catalog metadata is static (independent of what's on disk).
        assert!(!model_status("base").unwrap().offered, "base should be hidden");
        assert!(model_status("small").unwrap().offered);
        let medium = model_status("medium").unwrap();
        assert!(medium.offered);
        assert_eq!(medium.label, "Medium");
        assert!(medium.approx_mb > 1000);
    }

    #[test]
    fn validates_ggml_magic() {
        let dir = std::env::temp_dir();
        let good = dir.join(format!("ca_modeltest_good_{}.bin", std::process::id()));
        let bad = dir.join(format!("ca_modeltest_bad_{}.bin", std::process::id()));

        let mut f = fs::File::create(&good).unwrap();
        f.write_all(&GGML_MAGIC_LE).unwrap();
        f.write_all(&[0u8; 16]).unwrap();
        assert!(is_valid_ggml(&good));

        fs::File::create(&bad).unwrap().write_all(b"not a model").unwrap();
        assert!(!is_valid_ggml(&bad));

        assert!(!is_valid_ggml(&dir.join("ca_modeltest_missing.bin")));

        let _ = fs::remove_file(&good);
        let _ = fs::remove_file(&bad);
    }
}
