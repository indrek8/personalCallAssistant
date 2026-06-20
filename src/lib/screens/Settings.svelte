<script lang="ts">
  import { navigate, devices, settings, refreshDevices, banner, modelDownload } from "$lib/stores";
  import { saveSettings, isTauri, listModels, downloadModel } from "$lib/ipc";
  import type { Settings as SettingsT, Toggles, ModelStatus } from "$lib/types";

  type NavKey = "api" | "audio" | "transcription" | "storage";
  let section = $state<NavKey>("api");
  let showKey = $state(false);
  let apiKey = $state("sk-ant-api03-xxxxxxxxxxxxxxxxxxxx");

  // Working copy of settings (falls back to sensible defaults outside Tauri).
  const base: SettingsT = $settings ?? {
    capture_device_id: null,
    whisper_model: "medium",
    default_toggles: { f: true, c: true, s: false, q: true },
    budget_default: 5,
    storage_path: null,
    first_run: false,
  };
  let model = $state<string>(base.whisper_model);
  let captureDevice = $state<string>(base.capture_device_id ?? "");
  let toggles = $state<Toggles>({ ...base.default_toggles });

  const FEATURES: { k: keyof Toggles; n: string }[] = [
    { k: "f", n: "Fact-check" },
    { k: "c", n: "Commitments" },
    { k: "s", n: "Suggestions" },
    { k: "q", n: "Questions" },
  ];

  let models = $state<ModelStatus[]>([]);
  let dlActive = false;

  async function loadModels() {
    try {
      models = (await listModels()).filter((m) => m.offered);
    } catch (e) {
      banner.set(`Could not list models: ${String(e)}`);
    }
  }

  $effect(() => {
    if (section === "transcription" && models.length === 0) void loadModels();
  });
  // Refresh statuses when a download finishes (store returns to null).
  $effect(() => {
    const dl = $modelDownload;
    if (dl) dlActive = true;
    else if (dlActive) {
      dlActive = false;
      void loadModels();
    }
  });

  async function persist() {
    if (!isTauri()) return;
    try {
      const next: SettingsT = {
        ...base,
        whisper_model: model,
        capture_device_id: captureDevice || null,
        default_toggles: { ...toggles },
      };
      await saveSettings(next);
      settings.set(next);
    } catch (e) {
      banner.set(`Could not save settings: ${String(e)}`);
    }
  }
</script>

<section class="screen">
  <div class="duo">
    <div class="set-nav rise r1">
      <button class="backlink" onclick={() => navigate("dashboard")}>
        <svg class="icon" viewBox="0 0 24 24"><path d="m15 18-6-6 6-6" /></svg>Dashboard
      </button>
      <div class="stitle">Settings</div>
      <button class="snav" class:on={section === "api"} onclick={() => (section = "api")}>
        <svg viewBox="0 0 24 24"><path d="M12 2a4 4 0 0 1 4 4v6a4 4 0 0 1-8 0V6a4 4 0 0 1 4-4z" /><path d="M5 11a7 7 0 0 0 14 0M12 18v3" /></svg>API &amp; AI
      </button>
      <button class="snav" class:on={section === "audio"} onclick={() => { section = "audio"; refreshDevices(); }}>
        <svg viewBox="0 0 24 24"><path d="M3 10v4h4l5 4V6L7 10H3zM16 8a5 5 0 0 1 0 8" /></svg>Audio
      </button>
      <button class="snav" class:on={section === "transcription"} onclick={() => (section = "transcription")}>
        <svg viewBox="0 0 24 24"><path d="M4 6h16M4 12h10M4 18h7" /></svg>Transcription
      </button>
      <button class="snav" class:on={section === "storage"} onclick={() => (section = "storage")}>
        <svg viewBox="0 0 24 24"><path d="M3 7l9-4 9 4-9 4-9-4zM3 7v10l9 4 9-4V7" /></svg>Storage
      </button>
    </div>

    <div class="set-content scroll rise r2">
      {#if section === "api"}
        <div class="set-block">
          <div class="eyebrow">API &amp; intelligence</div>
          <div class="field">
            <label for="set-key">Claude API key</label>
            <div class="key-in">
              <input id="set-key" class="inp" type={showKey ? "text" : "password"} bind:value={apiKey} />
              <button class="btn" onclick={() => (showKey = !showKey)}>{showKey ? "Hide" : "Show"}</button>
              <button class="btn btn-gold">Test</button>
            </div>
            <div style="margin-top:10px">
              <span class="statusok"><svg class="icon" width="13" height="13" viewBox="0 0 24 24"><path d="M20 6 9 17l-5-5" /></svg>Connected · keys stored in macOS Keychain</span>
            </div>
          </div>
          <div class="field">
            <span class="field-label">Default live analysis for new sessions</span>
            <div class="feat-grid">
              {#each FEATURES as f}
                <button type="button" class="feat" class:on={toggles[f.k]} data-k={f.k.toUpperCase()} onclick={() => { toggles[f.k] = !toggles[f.k]; persist(); }}>
                  <div class="k">{f.k.toUpperCase()}</div>
                  <div class="n">{f.n}</div>
                </button>
              {/each}
            </div>
          </div>
        </div>
      {:else if section === "audio"}
        <div class="set-block">
          <div class="eyebrow">Audio</div>
          <div class="field">
            <label for="set-dev">Capture device</label>
            <div class="select-wrap">
              <select id="set-dev" class="inp" bind:value={captureDevice} onchange={persist} onfocus={() => refreshDevices()}>
                {#if $devices.length === 0}
                  <option value="">No input devices found</option>
                {/if}
                {#each $devices as d}
                  <option value={d.id}>{d.name}{d.is_default ? " (default)" : ""}</option>
                {/each}
              </select>
              <svg class="caret icon" viewBox="0 0 24 24"><path d="m6 9 6 6 6-6" /></svg>
            </div>
            <div class="hint">Select the virtual device (e.g. “Call Assistant” / BlackHole) you set as your meeting app's speaker, so we can capture the remote side.</div>
          </div>
        </div>
      {:else if section === "transcription"}
        <div class="set-block">
          <div class="eyebrow">Transcription</div>
          <div class="field">
            <span class="field-label">Whisper model</span>
            <div class="model-list">
              {#each models as m}
                <div class="model-row" class:on={model === m.name}>
                  <button class="model-pick" onclick={() => { model = m.name; persist(); }}>
                    <span class="ring"></span>
                    <span class="ml-name">{m.label}</span>
                    <span class="ml-desc">{m.approx_mb} MB · {m.speed_note}</span>
                  </button>
                  <div class="ml-action">
                    {#if m.downloaded}
                      <span class="dl-ok">✓ downloaded</span>
                    {:else if $modelDownload && $modelDownload.name === m.name}
                      <span class="dl-prog">{$modelDownload.pct}%</span>
                    {:else}
                      <button class="btn" onclick={() => downloadModel(m.name)}>Download</button>
                    {/if}
                  </div>
                </div>
              {/each}
              {#if models.length === 0}
                <div class="ml-desc" style="padding:8px">Loading models…</div>
              {/if}
            </div>
            <div class="hint">The active model is used for new sessions. “Medium” is the most accurate; “Small” downloads and runs faster.</div>
          </div>
        </div>
      {:else}
        <div class="set-block">
          <div class="eyebrow">Storage</div>
          <div class="set-row">
            <div class="si">
              <div class="sn">Data location</div>
              <div class="sd mono">{base.storage_path ?? "~/Library/Application Support/CallAssistant"}</div>
            </div>
            <button class="btn">Reveal in Finder</button>
          </div>
        </div>
      {/if}
    </div>
  </div>
</section>

<style>
  .duo{flex:1;display:flex;overflow:hidden}
  .set-nav{width:230px;flex:none;border-right:1px solid var(--line-soft);padding:28px 14px;display:flex;flex-direction:column;gap:2px}
  .set-nav .backlink{padding:0 12px;margin-bottom:4px}
  .set-nav .stitle{font-family:var(--f-disp);font-weight:600;font-size:22px;margin:6px 0 16px;padding:0 12px}
  .snav{display:flex;align-items:center;gap:11px;padding:10px 12px;border-radius:9px;font-size:13px;font-weight:500;color:var(--ink-3);cursor:pointer;transition:.15s;background:transparent;border:0;font-family:var(--f-ui);text-align:left}
  .snav:hover{background:var(--bg-2);color:var(--ink-2)}
  .snav.on{background:var(--gold-soft);color:var(--gold);box-shadow:inset 0 0 0 1px var(--gold-line)}
  .snav svg{width:16px;height:16px;stroke:currentColor;fill:none;stroke-width:1.7;stroke-linecap:round;stroke-linejoin:round}

  .set-content{flex:1;overflow-y:auto;padding:38px 46px 50px;max-width:880px}
  .set-block{margin-bottom:38px}
  .set-block>.eyebrow{margin-bottom:16px;display:block}
  .field-label{display:block;font-family:var(--f-mono);font-size:10px;letter-spacing:.14em;text-transform:uppercase;color:var(--ink-3);margin-bottom:9px}

  .key-in{display:flex;gap:8px;align-items:center}
  .key-in .inp{font-family:var(--f-mono);font-size:13px;letter-spacing:.04em}
  .statusok{display:inline-flex;align-items:center;gap:6px;font-family:var(--f-mono);font-size:11px;color:var(--suggest)}

  .select-wrap{position:relative}
  .select-wrap select{appearance:none;-webkit-appearance:none;cursor:pointer;padding-right:36px}
  .select-wrap .caret{position:absolute;right:12px;top:50%;transform:translateY(-50%);color:var(--ink-4);pointer-events:none}

  .ring{width:14px;height:14px;border-radius:50%;border:1.5px solid var(--line);flex:none}
  .model-list{display:flex;flex-direction:column;gap:10px;max-width:560px}
  .model-row{display:flex;align-items:center;gap:12px;border:1px solid var(--line);background:var(--bg-2);border-radius:10px;padding:4px 12px 4px 4px;transition:.16s}
  .model-row.on{border-color:var(--gold-line);background:var(--gold-soft)}
  .model-row.on .ring{border-color:var(--gold);background:radial-gradient(circle,var(--gold) 42%,transparent 46%)}
  .model-pick{flex:1;display:flex;align-items:center;gap:10px;background:none;border:0;cursor:pointer;text-align:left;padding:9px;font-family:var(--f-ui);color:var(--ink)}
  .ml-name{font-weight:600;font-size:13px}
  .ml-desc{font-size:11px;color:var(--ink-3)}
  .ml-action{flex:none}
  .dl-ok{font-family:var(--f-mono);font-size:10px;color:var(--suggest)}
  .dl-prog{font-family:var(--f-mono);font-size:11px;color:var(--gold)}
  .hint{font-size:12px;color:var(--ink-4);line-height:1.5;margin-top:12px;max-width:520px}

  .set-row{display:flex;align-items:center;justify-content:space-between;gap:14px;border:1px solid var(--line-soft);border-radius:10px;padding:13px 15px}
  .set-row .si{flex:1}
  .set-row .sn{font-weight:600;font-size:13.5px}
  .set-row .sd{font-size:12px;color:var(--ink-3);margin-top:3px}
  .set-row .sd.mono{font-family:var(--f-mono);font-size:11px}
</style>
