//! Dev helper: fetch a Whisper model into the app's model dir via the real
//! `model_mgr` downloader (resumable). Parallels `spikes/fetch-model.sh`, but
//! exercises the production download path used by Settings/onboarding in PR3.
//!
//!   cargo run --example fetch_model -- base     # or: small | medium

use std::io::Write;

use call_assistant_lib::stt::model_mgr;

fn main() {
    let name = std::env::args().nth(1).unwrap_or_else(|| "base".to_string());
    let path = match model_mgr::model_path(&name) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    if model_mgr::is_valid_ggml(&path) {
        println!("{name} already present → {}", path.display());
        return;
    }

    println!("downloading {name} → {} …", path.display());
    let mut last = 0u64;
    let result = model_mgr::download_model(&name, |done, total| {
        if done.saturating_sub(last) > 4_000_000 || Some(done) == total {
            last = done;
            match total {
                Some(t) => print!(
                    "\r  {:.1} / {:.1} MB ({:.0}%)    ",
                    done as f64 / 1e6,
                    t as f64 / 1e6,
                    100.0 * done as f64 / t as f64
                ),
                None => print!("\r  {:.1} MB    ", done as f64 / 1e6),
            }
            let _ = std::io::stdout().flush();
        }
    });

    match result {
        Ok(()) => {
            let st = model_mgr::model_status(&name).unwrap();
            println!(
                "\ndone → {} (downloaded={}, {} bytes)",
                st.path, st.downloaded, st.size_bytes
            );
        }
        Err(e) => {
            eprintln!("\ndownload failed: {e}");
            std::process::exit(1);
        }
    }
}
