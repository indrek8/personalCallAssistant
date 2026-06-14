# Next run — M1 Walking Skeleton + M0 spikes (ultramode brief)

> Working file: the kickoff brief for the first build run. Paste the prompt below into your ultramode run. Delete this file once M1 is merged.

## Prereqs

- Rust + Node + Xcode CLT — ✅ already installed
- **BlackHole 2ch** — installed; **requires a reboot** before the audio spike (s3) / M2
- `ANTHROPIC_API_KEY` — keep it in a **gitignored `.env`** at the repo root (never committed): `cp .env.example .env`, then add your key. The spikes load it automatically via `dotenvy` — no manual export, and it survives reboots. Only `s4` needs it.

## Run order (after reboot)

1. `git checkout -b m1-walking-skeleton`
2. Run the prompt below in ultramode → scaffolds M1 + writes the spikes + verifies it compiles
3. `cp .env.example .env` and add your key (gitignored; only `s4` needs it)
4. Validate the unknowns: `cargo run --bin s1_whisper` · `cargo run --bin s3_dual_audio` · `cargo run --bin s4_claude`
5. M1 acceptance: `npm run tauri dev` opens the dashboard shell; creating a session writes a folder under `~/Library/Application Support/CallAssistant/` that survives a restart

---

## Ultramode prompt

```
Build the M1 walking skeleton + M0 de-risking spikes for this repo (Personal Call
Assistant), strictly following the specs already in docs/build/.

Read first: docs/build/milestones.md (M0, M1), docs/build/technical-design.md
(§3 modules, §7 IPC contract, §9 storage), docs/build/flows.md (§1 state machine,
§4 pre-flight), docs/mvp.md, and design/prototype.html + design/ui-spec.md for the
exact visual language.

Deliver on branch m1-walking-skeleton:

APP (M1):
1. Tauri v2 + Svelte + TypeScript scaffold at repo root (package.json, src/, src-tauri/).
2. Rust module skeleton per technical-design §3: audio/ stt/ ai/ storage/ session/,
   plus commands.rs, events.rs, error.rs.
3. All six screens as Svelte components in the "Warm Ink Studio" language from
   design/prototype.html (reuse its tokens/fonts/colors), driven by a single `mode`
   store matching flows.md §1. Mock data where M1 says so.
4. Rust command list_audio_input_devices() (real cpal) wired to a populated dropdown.
5. storage module: create_session writes sessions/{uuid}/metadata.json; list_sessions
   reads them; dashboard left pane renders from disk.
6. Boot routing: load settings.json, route to onboarding vs dashboard.

SPIKES (M0) in spikes/ (separate throwaway cargo project), each with a RUN.md:
7. s1_whisper: load a `small` ggml model via whisper-rs, transcribe a 10s 16kHz WAV,
   print text + wall-time.
8. s3_dual_audio: open mic + a BlackHole input via cpal at once, write a 10s stereo
   WAV (L=mic, R=blackhole).
9. s4_claude: minimal Haiku + Sonnet call (load ANTHROPIC_API_KEY from a gitignored root
   .env via dotenvy), print the parsed JSON + token/cost fields.

CONSTRAINTS: Svelte + TS (not React). Match the IPC command/event names in
technical-design §7. Flat-file storage under ~/Library/Application Support/CallAssistant/.
Secrets load from a gitignored root .env via dotenvy (add dotenvy to the spike crate).
Skeleton only — NO Whisper/Claude wiring inside the app yet (that's M2/M3).

VERIFY before finishing: `cargo check` (app + spikes) and `npm run build` pass;
`npm run tauri dev` launches to the dashboard shell; creating a session writes a folder
that survives a restart. Acceptance = the M1 criteria in docs/build/milestones.md.
```
