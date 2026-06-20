<script lang="ts">
  import { onMount } from "svelte";
  import {
    navigate,
    transcript,
    live,
    liveSessionId,
    sessions,
    settings,
    devices,
    banner,
    refreshSessions,
  } from "$lib/stores";
  import { isTauri, pauseCapture, resumeCapture, endSession } from "$lib/ipc";
  import type { StreamTag } from "$lib/types";

  // M2: the transcript + timer are real (fed by `transcript-entry` /
  // `capture-state` events). The AI panel is intentionally inert — live analysis
  // lands in M3.

  let ending = $state(false);
  let trArea: HTMLDivElement;

  // Local 1 Hz timer, corrected by `capture-state` events (which only fire on
  // start/pause/resume/end, so we only re-sync when elapsed actually changes).
  let displayMs = $state(0);
  let prevElapsed = -1;
  $effect(() => {
    if ($live.elapsedMs !== prevElapsed) {
      prevElapsed = $live.elapsedMs;
      displayMs = $live.elapsedMs;
    }
  });
  onMount(() => {
    const id = setInterval(() => {
      if ($live.status === "recording") displayMs += 1000;
    }, 1000);
    return () => clearInterval(id);
  });

  // Auto-scroll the transcript to the newest line.
  $effect(() => {
    void $transcript.length;
    if (trArea) trArea.scrollTop = trArea.scrollHeight;
  });

  const timer = $derived.by(() => {
    const t = Math.floor(displayMs / 1000);
    const h = String(Math.floor(t / 3600)).padStart(2, "0");
    const m = String(Math.floor((t % 3600) / 60)).padStart(2, "0");
    const s = String(t % 60).padStart(2, "0");
    return `${h}:${m}:${s}`;
  });

  const liveName = $derived.by(
    () => $sessions.find((s) => s.id === $liveSessionId)?.name ?? "Live Session",
  );
  const micName = $derived.by(() => {
    const id = $settings?.capture_device_id;
    const d = $devices.find((x) => x.id === id) ?? $devices.find((x) => x.is_default);
    return d?.name ?? "Default mic";
  });

  const paused = $derived($live.status === "paused");

  function fmtTs(ms: number): string {
    const t = Math.floor(ms / 1000);
    return `${String(Math.floor(t / 60)).padStart(2, "0")}:${String(t % 60).padStart(2, "0")}`;
  }
  const whoLabel = (s: StreamTag) => (s === "you" ? "You" : "Remote");
  const whoClass = (s: StreamTag) => (s === "you" ? "who-you" : "who-remote");

  async function togglePause() {
    try {
      if (!isTauri()) return;
      if (paused) await resumeCapture();
      else await pauseCapture();
    } catch (e) {
      banner.set(`Could not ${paused ? "resume" : "pause"}: ${String(e)}`);
    }
  }

  async function end() {
    if (typeof window !== "undefined" && !window.confirm("End this session?")) return;
    ending = true;
    try {
      if (isTauri()) {
        await endSession();
        await refreshSessions();
      }
      liveSessionId.set(null);
      navigate("dashboard");
    } catch (e) {
      banner.set(`Could not end session: ${String(e)}`);
      ending = false;
    }
  }
</script>

<section class="screen live">
  <div class="live-bar rise r1">
    <div class="rec" class:paused><span class="d"></span>{paused ? "PAUSED" : "REC"}</div>
    <div class="live-title">{liveName}</div>
    <div class="timer">{timer}</div>
    <div class="spacer"></div>
    <div class="dev">🎙 Mic: <b>{micName}</b></div>
    <div class="dev">🔈 Remote: <b>BlackHole</b></div>
    {#if $live.lagging}
      <div class="lag">transcribing…</div>
    {/if}
    <div class="ctrl">
      <button class="btn btn-ghost" aria-label={paused ? "Resume" : "Pause"} onclick={togglePause}>
        {#if paused}
          <svg class="icon" viewBox="0 0 24 24"><path d="M8 5v14l11-7z" /></svg>
        {:else}
          <svg class="icon" viewBox="0 0 24 24"><path d="M6 4h4v16H6zM14 4h4v16h-4z" /></svg>
        {/if}
      </button>
      <button class="btn btn-rec" disabled={ending} onclick={end}>
        <svg class="icon" viewBox="0 0 24 24"><rect x="5" y="5" width="14" height="14" rx="2" /></svg>{ending ? "Ending…" : "End"}
      </button>
    </div>
  </div>

  <div class="live-body">
    <div class="tr-area scroll" bind:this={trArea}>
      {#each $transcript as line (line.id)}
        <div class="tr">
          <div class="ts">{fmtTs(line.t_ms)}</div>
          <div>
            <div class="who {whoClass(line.stream)}">{whoLabel(line.stream)}</div>
            <div class="said">{line.text}</div>
          </div>
        </div>
      {/each}
      {#if $transcript.length === 0}
        <div class="empty">Waiting for speech… talk, or play audio through the Multi-Output device.</div>
      {/if}
      {#if !paused}
        <div class="listening">
          <div class="eq"><i></i><i></i><i></i><i></i><i></i><i></i></div>
          <span>Listening…</span>
        </div>
      {/if}
    </div>

    <div class="ai">
      <div class="ai-head">
        <span class="lab">Live Intelligence</span>
        <span class="soon">arrives in M3</span>
      </div>
      <div class="findings scroll">
        <div class="placeholder">
          Real-time fact-checks, commitments, suggestions, and Q&amp;A will appear here once the
          live AI pipeline (M3) is wired in. For now the transcript above is fully live.
        </div>
      </div>
      <div class="ask-bar">
        <div class="ic"><svg class="icon" viewBox="0 0 24 24"><path d="M12 2a4 4 0 0 1 4 4v6a4 4 0 0 1-8 0V6a4 4 0 0 1 4-4z" /><path d="M5 11a7 7 0 0 0 14 0M12 18v3" /></svg></div>
        <input placeholder="Ask AI — available in M3" disabled />
      </div>
    </div>
  </div>
</section>

<style>
  .live{background:radial-gradient(900px 380px at 50% -120px,var(--rec-soft),transparent 70%),var(--bg-1)}
  .live-bar{flex:none;height:58px;display:flex;align-items:center;gap:14px;padding:0 18px;border-bottom:1px solid var(--line-soft);position:relative}
  .live-bar::after{content:"";position:absolute;left:0;right:0;bottom:-1px;height:1px;background:linear-gradient(90deg,transparent,var(--rec-soft),transparent)}
  .rec{display:flex;align-items:center;gap:9px;font-family:var(--f-mono);font-size:10px;letter-spacing:.18em;color:var(--rec);font-weight:600}
  .rec .d{width:9px;height:9px;border-radius:50%;background:var(--rec);animation:pulse 1.7s infinite}
  .rec.paused{color:var(--ink-3)}
  .rec.paused .d{background:var(--ink-4);animation:none}
  .live-title{font-weight:600;font-size:14px}
  .timer{font-family:var(--f-mono);font-size:15px;color:var(--ink);letter-spacing:.03em;font-weight:500}
  .spacer{flex:1}
  .dev{display:flex;align-items:center;gap:7px;font-family:var(--f-mono);font-size:11px;color:var(--ink-3);background:var(--bg-2);border:1px solid var(--line);border-radius:7px;padding:6px 10px}
  .dev b{color:var(--ink-2);font-weight:500}
  .lag{font-family:var(--f-mono);font-size:10px;letter-spacing:.08em;text-transform:uppercase;color:var(--fact)}
  .ctrl{display:flex;gap:8px}

  .live-body{flex:1;display:flex;flex-direction:column;overflow:hidden}
  .tr-area{flex:1;overflow-y:auto;padding:24px 30px}
  .tr{display:flex;gap:18px;margin-bottom:21px;max-width:880px}
  .tr .ts{font-family:var(--f-mono);font-size:10.5px;color:var(--ink-4);width:62px;flex:none;padding-top:3px}
  .tr .who{font-family:var(--f-mono);font-size:10.5px;letter-spacing:.12em;text-transform:uppercase;font-weight:600;margin-bottom:5px}
  .tr .said{font-size:15px;line-height:1.62;color:var(--ink)}
  :global(.who-you){color:var(--gold)} :global(.who-remote){color:var(--commit)}
  .empty{font-size:14px;color:var(--ink-4);max-width:560px;line-height:1.6;padding:8px 0}
  .listening{display:flex;align-items:center;gap:13px;padding:4px 0 8px 80px;opacity:.85}
  .eq{display:flex;align-items:flex-end;gap:3px;height:18px}
  .eq i{width:3px;background:var(--rec);border-radius:2px;animation:eq 1s infinite ease-in-out}
  .eq i:nth-child(1){animation-delay:0s;height:6px} .eq i:nth-child(2){animation-delay:.15s;height:14px}
  .eq i:nth-child(3){animation-delay:.3s;height:9px} .eq i:nth-child(4){animation-delay:.45s;height:17px}
  .eq i:nth-child(5){animation-delay:.6s;height:7px} .eq i:nth-child(6){animation-delay:.75s;height:12px}
  .listening span{font-family:var(--f-mono);font-size:10.5px;letter-spacing:.1em;text-transform:uppercase;color:var(--ink-3)}

  .ai{flex:none;height:200px;border-top:1px solid var(--line);background:linear-gradient(180deg,var(--bg-2),var(--bg-1));display:flex;flex-direction:column}
  .ai-head{flex:none;display:flex;align-items:center;gap:12px;padding:12px 18px;border-bottom:1px solid var(--line-soft)}
  .ai-head .lab{font-family:var(--f-mono);font-size:10px;letter-spacing:.2em;text-transform:uppercase;color:var(--ink-3)}
  .ai-head .soon{margin-left:auto;font-family:var(--f-mono);font-size:9.5px;letter-spacing:.1em;text-transform:uppercase;color:var(--ink-4);border:1px solid var(--line);border-radius:6px;padding:3px 8px}
  .findings{flex:1;overflow-y:auto;padding:14px 18px}
  .placeholder{font-size:13px;line-height:1.6;color:var(--ink-4);max-width:620px}
  .ask-bar{flex:none;padding:12px 16px;border-top:1px solid var(--line-soft);display:flex;gap:10px;align-items:center;background:var(--bg-1)}
  .ask-bar .ic{width:30px;height:30px;border-radius:8px;background:var(--gold-soft);border:1px solid var(--gold-line);display:flex;align-items:center;justify-content:center;color:var(--gold);flex:none;opacity:.5}
  .ask-bar input{flex:1;background:var(--bg-2);border:1px solid var(--line);border-radius:9px;padding:10px 14px;color:var(--ink);font-family:var(--f-ui);font-size:13px;outline:none;opacity:.5}
  .ask-bar input::placeholder{color:var(--ink-4)}
</style>
