<script lang="ts">
  import { navigate, devices, settings, refreshDevices, banner } from "$lib/stores";
  import { saveSettings, isTauri } from "$lib/ipc";
  import Mark from "$lib/components/Mark.svelte";
  import type { Settings as SettingsT } from "$lib/types";

  // 4-step wizard (flows.md §3). Step 1 welcome lives in the brand rail copy;
  // the form steps are 1..4 here (Claude, Audio, Model, Done).
  let step = $state(1);
  let showKey = $state(false);
  let apiKey = $state("sk-ant-api03-xxxxxxxxxxxxxxxxxxxx");
  let captureDevice = $state("");
  let model = $state("medium");
  let saving = $state(false);

  const STEPS = ["Connect Claude", "Audio device", "Transcription model", "All set"];

  const WAVE = [20, 38, 52, 30, 46, 58, 34, 44, 24, 40, 54, 28, 48, 36, 22];
  const MODELS: { id: string; name: string; desc: string }[] = [
    { id: "base", name: "Base", desc: "Fastest · lower accuracy" },
    { id: "small", name: "Small", desc: "Balanced · fast" },
    { id: "medium", name: "Medium", desc: "Best accuracy · recommended" },
  ];

  $effect(() => {
    if (step === 2 && $devices.length === 0) refreshDevices();
  });
  $effect(() => {
    if (!captureDevice && $devices.length) {
      captureDevice = ($devices.find((d) => d.is_default) ?? $devices[0]).id;
    }
  });

  function next() {
    if (step < 4) step += 1;
  }
  function back() {
    if (step > 1) step -= 1;
  }

  async function finish() {
    saving = true;
    try {
      if (isTauri()) {
        const base: SettingsT = $settings ?? {
          capture_device_id: null,
          whisper_model: "medium",
          default_toggles: { f: true, c: true, s: false, q: true },
          budget_default: 5,
          storage_path: null,
          first_run: true,
        };
        const next: SettingsT = {
          ...base,
          capture_device_id: captureDevice || null,
          whisper_model: model,
          first_run: false,
        };
        await saveSettings(next);
        settings.set(next);
      }
      navigate("dashboard");
    } catch (e) {
      banner.set(`Could not finish setup: ${String(e)}`);
      saving = false;
    }
  }
</script>

<section class="screen">
  <div class="ob">
    <div class="ob-brand">
      <div>
        <Mark size={34} />
        <div class="big">Call<br />Assistant</div>
        <div class="tag">
          Your invisible meeting partner. It listens, transcribes locally, and turns every
          conversation into tracked commitments.
        </div>
        <div class="ob-wave">
          {#each WAVE as h, i}
            <i style={`height:${h}px;animation-delay:${i * 0.09}s;animation-duration:${1.1 + (i % 4) * 0.18}s`}></i>
          {/each}
        </div>
      </div>
      <div class="ob-steps-mini">
        {#each STEPS as label, i}
          <div class="osm" class:done={step > i + 1} class:cur={step === i + 1}>
            <span class="n">{step > i + 1 ? "✓" : i + 1}</span>{label}
          </div>
        {/each}
      </div>
    </div>

    <div class="ob-form">
      {#if step === 1}
        <div class="step-no rise r1">Step 1 of 4</div>
        <h2 class="rise r1">Connect to Claude</h2>
        <p class="rise r2">
          Paste your Claude API key. It powers live fact-checking, commitment detection, and
          post-call analysis. Stored only in your macOS Keychain — never leaves your machine.
        </p>
        <div class="rise r3" style="max-width:440px">
          <div class="field">
            <label for="ob-key">Claude API key</label>
            <div class="key-in">
              <input id="ob-key" class="inp" type={showKey ? "text" : "password"} bind:value={apiKey} />
              <button class="btn" onclick={() => (showKey = !showKey)}>{showKey ? "Hide" : "Show"}</button>
            </div>
          </div>
          <div style="display:flex;gap:12px;margin-top:8px">
            <button class="btn btn-gold" style="padding:12px 22px" onclick={next}>Test &amp; continue</button>
            <button class="btn btn-ghost" onclick={next}>Skip for now</button>
          </div>
        </div>
      {:else if step === 2}
        <div class="step-no rise r1">Step 2 of 4</div>
        <h2 class="rise r1">Audio device</h2>
        <p class="rise r2">
          Set up a Multi-Output Device so the remote side of the call reaches both your ears and us.
          Then pick the capture (input) device we should listen to.
        </p>
        <div class="rise r3" style="max-width:440px">
          <div class="field">
            <label for="ob-dev">Capture device</label>
            <div class="select-wrap">
              <select id="ob-dev" class="inp" bind:value={captureDevice} onfocus={() => refreshDevices()}>
                {#if $devices.length === 0}
                  <option value="">No input devices found</option>
                {/if}
                {#each $devices as d}
                  <option value={d.id}>{d.name}{d.is_default ? " (default)" : ""}</option>
                {/each}
              </select>
              <svg class="caret icon" viewBox="0 0 24 24"><path d="m6 9 6 6 6-6" /></svg>
            </div>
          </div>
          <div style="display:flex;gap:12px;margin-top:8px">
            <button class="btn btn-ghost" onclick={back}>Back</button>
            <button class="btn btn-gold" style="padding:12px 22px" onclick={next}>Continue</button>
          </div>
        </div>
      {:else if step === 3}
        <div class="step-no rise r1">Step 3 of 4</div>
        <h2 class="rise r1">Transcription model</h2>
        <p class="rise r2">
          Whisper runs locally on your machine. Pick a model — you can change this any time in
          Settings.
        </p>
        <div class="rise r3" style="max-width:480px">
          <div class="field">
            <div class="radio">
              {#each MODELS as m}
                <button class="rad" class:on={model === m.id} onclick={() => (model = m.id)}>
                  <div class="rn"><span class="ring"></span>{m.name}</div>
                  <div class="rd">{m.desc}</div>
                </button>
              {/each}
            </div>
          </div>
          <div style="display:flex;gap:12px;margin-top:8px">
            <button class="btn btn-ghost" onclick={back}>Back</button>
            <button class="btn btn-gold" style="padding:12px 22px" onclick={next}>Continue</button>
          </div>
        </div>
      {:else}
        <div class="step-no rise r1">Step 4 of 4</div>
        <h2 class="rise r1">You're all set</h2>
        <p class="rise r2">
          Setup is complete. Create your first session to start capturing calls — the AI will fact-check,
          track commitments, and summarize when you're done.
        </p>
        <div class="rise r3" style="display:flex;gap:12px;margin-top:8px">
          <button class="btn btn-ghost" onclick={back}>Back</button>
          <button class="btn btn-gold" style="padding:12px 22px" disabled={saving} onclick={finish}>
            {saving ? "Saving…" : "Go to dashboard"}
          </button>
        </div>
      {/if}
    </div>
  </div>
</section>

<style>
  .ob{flex:1;display:flex;overflow:hidden}
  .ob-brand{width:420px;flex:none;background:radial-gradient(600px 400px at 30% 20%,rgba(231,178,76,.1),transparent 60%),linear-gradient(160deg,#1B1610,#100E0B);border-right:1px solid var(--line-soft);padding:54px 44px;display:flex;flex-direction:column;justify-content:space-between;position:relative;overflow:hidden}
  .ob-brand .big{font-family:var(--f-disp);font-weight:600;font-size:46px;line-height:1.02;letter-spacing:-.03em;margin-top:26px}
  .ob-brand .tag{font-size:15px;color:var(--ink-2);line-height:1.6;margin-top:18px;max-width:300px}
  .ob-wave{display:flex;align-items:flex-end;gap:4px;height:60px;margin-top:30px}
  .ob-wave i{width:4px;background:linear-gradient(180deg,var(--gold),rgba(231,178,76,.25));border-radius:3px;animation:eq 1.4s infinite ease-in-out}
  .ob-steps-mini{display:flex;flex-direction:column;gap:14px}
  .osm{display:flex;align-items:center;gap:12px;font-size:13px;color:var(--ink-3)}
  .osm .n{width:24px;height:24px;border-radius:50%;border:1px solid var(--line);display:flex;align-items:center;justify-content:center;font-family:var(--f-mono);font-size:11px;flex:none}
  .osm.done{color:var(--ink-2)} .osm.done .n{background:var(--gold-soft);border-color:var(--gold-line);color:var(--gold)}
  .osm.cur{color:var(--ink)} .osm.cur .n{background:var(--gold);border-color:var(--gold);color:#27200C;font-weight:600}

  .ob-form{flex:1;display:flex;flex-direction:column;justify-content:center;padding:0 56px}
  .ob-form .step-no{font-family:var(--f-mono);font-size:11px;letter-spacing:.16em;text-transform:uppercase;color:var(--gold);margin-bottom:12px}
  .ob-form h2{font-family:var(--f-disp);font-weight:600;font-size:32px;letter-spacing:-.02em;margin-bottom:10px}
  .ob-form p{font-size:14px;color:var(--ink-2);line-height:1.6;margin-bottom:26px;max-width:420px}

  .key-in{display:flex;gap:8px;align-items:center}
  .key-in .inp{font-family:var(--f-mono);font-size:13px;letter-spacing:.04em}

  .select-wrap{position:relative}
  .select-wrap select{appearance:none;-webkit-appearance:none;cursor:pointer;padding-right:36px}
  .select-wrap .caret{position:absolute;right:12px;top:50%;transform:translateY(-50%);color:var(--ink-4);pointer-events:none}

  .radio{display:flex;gap:10px}
  .rad{flex:1;border:1px solid var(--line);background:var(--bg-2);border-radius:10px;padding:13px 14px;cursor:pointer;transition:.16s;text-align:left;font-family:var(--f-ui);color:var(--ink)}
  .rad.on{border-color:var(--gold-line);background:var(--gold-soft)}
  .rad .rn{font-weight:600;font-size:13px;display:flex;align-items:center;gap:8px}
  .rad .rd{font-size:11px;color:var(--ink-3);margin-top:5px}
  .ring{width:14px;height:14px;border-radius:50%;border:1.5px solid var(--line);flex:none}
  .rad.on .ring{border-color:var(--gold);background:radial-gradient(circle,var(--gold) 42%,transparent 46%)}
</style>
