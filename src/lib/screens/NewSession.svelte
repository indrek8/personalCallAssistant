<script lang="ts">
  import { navigate, devices, settings, refreshSessions, refreshDevices, banner, startLive, modelDownload } from "$lib/stores";
  import { createSession, isTauri, runPreflight, startCapture, saveSettings, downloadModel, setToggles } from "$lib/ipc";
  import type { LabelRef, SessionDraft, Toggles, PreflightResult } from "$lib/types";

  let name = $state("Board Call Q2");
  let participants = $state("");
  let context = $state(
    `Follow-up to the Q1 board meeting.\n\nDiscussing CBUAE Phase 2 delays and the KYC module certification timeline. Central bank circular CB-2025-041 sets the Phase 2 deadline at Aug 2026.\n\nAhmed owes cost estimates from the last call. Sarah is tracking the KYC vendor relationship.`,
  );
  let labels = $state<LabelRef[]>([
    { id: "acme", name: "Acme" },
    { id: "board", name: "Board" },
  ]);
  let deviceId = $state<string>($settings?.capture_device_id ?? "");
  let starting = $state(false);
  let preflight = $state<PreflightResult | null>(null);
  let pendingSessionId = $state<string | null>(null);

  const initialToggles = $settings?.default_toggles ?? { f: true, c: true, s: false, q: true };
  let toggles = $state<Toggles>({ ...initialToggles });

  const FEATURES: { k: keyof Toggles; n: string }[] = [
    { k: "f", n: "Fact-check" },
    { k: "c", n: "Commitments" },
    { k: "s", n: "Suggestions" },
    { k: "q", n: "Questions" },
  ];

  // Default the device selection to the system default once devices load.
  $effect(() => {
    if (!deviceId && $devices.length) {
      deviceId = ($devices.find((d) => d.is_default) ?? $devices[0]).id;
    }
  });

  async function start() {
    starting = true;
    preflight = null;
    try {
      const draft: SessionDraft = {
        name: name.trim() || null,
        labels,
        participants: participants
          .split(",")
          .map((p) => p.trim())
          .filter(Boolean),
        context_notes: context.trim() || null,
        budget_cap: $settings?.budget_default ?? null,
      };
      if (!isTauri()) {
        navigate("live");
        return;
      }

      // Persist the chosen capture device so start_capture uses it.
      if ($settings && deviceId && deviceId !== $settings.capture_device_id) {
        const next = { ...$settings, capture_device_id: deviceId };
        await saveSettings(next);
        settings.set(next);
      }

      // Reuse the draft session across retries so a failed pre-flight doesn't
      // leave orphan sessions behind.
      let sid = pendingSessionId;
      if (!sid) {
        sid = (await createSession(draft)).session_id;
        pendingSessionId = sid;
        await refreshSessions();
      }

      const pf = await runPreflight(sid);
      if (!pf.ok) {
        preflight = pf;
        starting = false;
        return;
      }

      startLive(sid, toggles);
      await startCapture(sid);
      // Sync the form's toggle choice to the live-AI batcher (it starts from the
      // saved defaults; this applies the per-session selection).
      await setToggles(toggles);
      pendingSessionId = null;
      navigate("live");
    } catch (e) {
      banner.set(`Could not start session: ${String(e)}`);
      starting = false;
    }
  }

  function fixModel() {
    downloadModel($settings?.whisper_model ?? "medium").catch((e) =>
      banner.set(`Download failed: ${String(e)}`),
    );
  }

  function labelClass(n: string) {
    const x = n.toLowerCase();
    if (x.includes("acme")) return "lbl-acme";
    if (x.includes("globex")) return "lbl-globex";
    if (x.includes("board")) return "lbl-int";
    return "lbl-int";
  }
</script>

<section class="screen">
  <div class="duo">
    <div class="rail rise r1">
      <div class="rail-top">
        <button class="backlink" onclick={() => navigate("dashboard")}>
          <svg class="icon" viewBox="0 0 24 24"><path d="m15 18-6-6 6-6" /></svg>Dashboard
        </button>
        <div class="eyebrow">Prepare</div>
        <h2>New Session</h2>
      </div>
      <div class="rail-body scroll">
        <div class="field">
          <label for="ns-name">Session name</label>
          <input id="ns-name" class="inp" bind:value={name} />
        </div>

        <div class="field">
          <span class="field-label">Labels</span>
          <div class="chips-in">
            {#each labels as l}
              <span class="lbl {labelClass(l.name)}">{l.name}</span>
            {/each}
            <button class="add" type="button">+ add</button>
          </div>
        </div>

        <div class="field">
          <label for="ns-part">Participants <span class="opt">· optional</span></label>
          <input id="ns-part" class="inp" placeholder="Sarah, Ahmed" bind:value={participants} />
        </div>

        <div class="field">
          <label for="ns-dev">Capture device</label>
          <div class="select-wrap">
            <select id="ns-dev" class="inp" bind:value={deviceId} onfocus={() => refreshDevices()}>
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

        <div class="field">
          <span class="field-label">Live analysis</span>
          <div class="feat-grid">
            {#each FEATURES as f}
              <button
                type="button"
                class="feat"
                class:on={toggles[f.k]}
                data-k={f.k.toUpperCase()}
                onclick={() => (toggles[f.k] = !toggles[f.k])}
              >
                <div class="k">{f.k.toUpperCase()}</div>
                <div class="n">{f.n}</div>
              </button>
            {/each}
          </div>
        </div>
      </div>
      <div class="rail-foot">
        {#if preflight && !preflight.ok}
          <div class="preflight">
            {#each preflight.checks.filter((c) => c.status !== "ok") as c}
              <div class="pf-row">
                <div class="pf-info">
                  <span class="pf-label">{c.label}</span>
                  <span class="pf-msg">{c.message}</span>
                </div>
                {#if c.fixable === "download_model"}
                  {#if $modelDownload}
                    <span class="pf-prog">{$modelDownload.pct}%</span>
                  {:else}
                    <button class="pf-btn" type="button" onclick={fixModel}>Download</button>
                  {/if}
                {/if}
              </div>
            {/each}
          </div>
        {/if}
        <button class="btn btn-gold start" disabled={starting} onclick={start}>
          <span class="d"></span>{starting ? "Starting…" : "Start Session"}
        </button>
      </div>
    </div>

    <div class="ctx rise r2">
      <div class="ctx-h">
        <span class="ci"><svg class="icon" viewBox="0 0 24 24"><path d="M4 6h16M4 12h16M4 18h10" /></svg></span>
        <span class="ct">Context for AI</span>
      </div>
      <div class="sub">
        This is what the AI fact-checks the live conversation against. Paste the agenda, key numbers,
        prior commitments, anything decided last time — the richer this is, the sharper the live
        analysis during your call.
      </div>
      <textarea bind:value={context}></textarea>
    </div>
  </div>
</section>

<style>
  .duo{flex:1;display:flex;overflow:hidden}
  .rail{width:432px;flex:none;border-right:1px solid var(--line-soft);display:flex;flex-direction:column}
  .rail-top{padding:30px 32px 14px}
  .rail-top .backlink{margin-bottom:16px}
  .rail-top h2{font-family:var(--f-disp);font-weight:600;font-size:30px;letter-spacing:-.02em;line-height:1;margin-top:2px}
  .rail-body{flex:1;overflow-y:auto;padding:8px 32px 20px}
  .rail-body .field{margin-bottom:20px}
  .rail .feat-grid{grid-template-columns:repeat(2,1fr)}
  .rail-foot{padding:16px 32px 24px;border-top:1px solid var(--line-soft)}
  .preflight{display:flex;flex-direction:column;gap:8px;margin-bottom:12px}
  .pf-row{display:flex;align-items:center;gap:10px;padding:10px 12px;border:1px solid rgba(255,107,92,.28);background:rgba(255,107,92,.08);border-radius:9px}
  .pf-info{display:flex;flex-direction:column;gap:2px;flex:1;min-width:0}
  .pf-label{font-family:var(--f-mono);font-size:10px;letter-spacing:.1em;text-transform:uppercase;color:var(--late)}
  .pf-msg{font-size:12.5px;color:var(--ink-2);line-height:1.4}
  .pf-btn{font-family:var(--f-mono);font-size:11px;color:var(--gold);background:var(--gold-soft);border:1px solid var(--gold-line);border-radius:7px;padding:6px 12px;cursor:pointer;flex:none}
  .pf-prog{font-family:var(--f-mono);font-size:11px;color:var(--gold);flex:none}

  .field-label{display:block;font-family:var(--f-mono);font-size:10px;letter-spacing:.14em;text-transform:uppercase;color:var(--ink-3);margin-bottom:9px}
  .opt{color:var(--ink-4);text-transform:none;letter-spacing:0}
  .chips-in{display:flex;flex-wrap:wrap;gap:8px;align-items:center;background:var(--bg-2);border:1px solid var(--line);border-radius:10px;padding:9px 11px}
  .chips-in .lbl{padding:4px 10px;font-size:12px;display:flex;align-items:center;gap:6px}
  .chips-in .add{font-family:var(--f-mono);font-size:11px;color:var(--ink-4);background:none;border:1px dashed var(--line);border-radius:6px;padding:4px 9px;cursor:pointer}

  .select-wrap{position:relative}
  .select-wrap select{appearance:none;-webkit-appearance:none;cursor:pointer;padding-right:36px}
  .select-wrap .caret{position:absolute;right:12px;top:50%;transform:translateY(-50%);color:var(--ink-4);pointer-events:none}

  .start{width:100%;justify-content:center;padding:15px;font-size:15px;margin-top:8px}
  .start .d{width:9px;height:9px;border-radius:50%;background:var(--rec);animation:pulse 1.7s infinite}

  .ctx{flex:1;display:flex;flex-direction:column;padding:34px 42px;background:radial-gradient(700px 340px at 85% -60px,rgba(231,178,76,.05),transparent 70%)}
  .ctx-h{display:flex;align-items:center;gap:11px;margin-bottom:12px}
  .ctx-h .ci{width:32px;height:32px;border-radius:9px;background:var(--gold-soft);border:1px solid var(--gold-line);display:flex;align-items:center;justify-content:center;color:var(--gold)}
  .ctx-h .ct{font-family:var(--f-disp);font-weight:600;font-size:21px;letter-spacing:-.01em}
  .ctx .sub{font-size:13px;color:var(--ink-3);line-height:1.6;margin-bottom:18px;max-width:580px}
  .ctx textarea{flex:1;resize:none;width:100%;background:var(--bg-2);border:1px solid var(--line);border-radius:14px;padding:22px 24px;color:var(--ink);font-family:var(--f-ui);font-size:15px;line-height:1.78;outline:none;transition:.18s}
  .ctx textarea:focus{border-color:var(--gold-line);box-shadow:0 0 0 3px var(--gold-soft)}
</style>
