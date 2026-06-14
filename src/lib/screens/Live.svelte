<script lang="ts">
  import { onMount } from "svelte";
  import { navigate } from "$lib/stores";
  import { LIVE_TRANSCRIPT, LIVE_FINDINGS } from "$lib/mock";
  import type { Toggles } from "$lib/types";

  // M1: the live pipeline is mocked. The timer ticks; transcript + findings are
  // static illustrative content from the prototype.
  let toggles = $state<Toggles>({ f: true, c: true, s: false, q: true });
  let secs = $state(23 * 60 + 15);

  const TOGGLE_KEYS: (keyof Toggles)[] = ["f", "c", "s", "q"];

  const timer = $derived.by(() => {
    const h = String(Math.floor(secs / 3600)).padStart(2, "0");
    const m = String(Math.floor((secs % 3600) / 60)).padStart(2, "0");
    const s = String(secs % 60).padStart(2, "0");
    return `${h}:${m}:${s}`;
  });

  function findClass(kind: string) {
    if (kind === "fact") return "f-fact";
    if (kind === "commit") return "f-commit";
    return "f-ask";
  }

  onMount(() => {
    const id = setInterval(() => (secs += 1), 1000);
    return () => clearInterval(id);
  });
</script>

<section class="screen live">
  <div class="live-bar rise r1">
    <div class="rec"><span class="d"></span>REC</div>
    <div class="live-title">Board Call Q2</div>
    <div class="timer">{timer}</div>
    <div class="spacer"></div>
    <div class="dev">🎙 Mic: <b>AirPods Pro</b> <svg class="icon" width="12" height="12" viewBox="0 0 24 24"><path d="m6 9 6 6 6-6" /></svg></div>
    <div class="dev">🔈 <b>MacBook</b> <svg class="icon" width="12" height="12" viewBox="0 0 24 24"><path d="m6 9 6 6 6-6" /></svg></div>
    <div class="cost">cost <b>$0.12</b> / $5</div>
    <div class="ctrl">
      <button class="btn btn-ghost" aria-label="Pause"><svg class="icon" viewBox="0 0 24 24"><path d="M6 4h4v16H6zM14 4h4v16h-4z" /></svg></button>
      <button class="btn btn-rec" onclick={() => navigate("post")}><svg class="icon" viewBox="0 0 24 24"><rect x="5" y="5" width="14" height="14" rx="2" /></svg>End</button>
    </div>
  </div>

  <div class="live-body">
    <div class="tr-area scroll">
      {#each LIVE_TRANSCRIPT as line}
        <div class="tr" class:muted={line.muted}>
          <div class="ts">{line.ts}</div>
          <div>
            <div class="who {line.cls}">{line.who}</div>
            <div class="said">{line.said}</div>
          </div>
        </div>
      {/each}
      <div class="listening">
        <div class="eq"><i></i><i></i><i></i><i></i><i></i><i></i></div>
        <span>Listening…</span>
      </div>
    </div>

    <div class="ai">
      <div class="ai-head">
        <span class="lab">Live Intelligence</span>
        <div class="toggles">
          {#each TOGGLE_KEYS as k}
            <button class="tg" class:on={toggles[k]} data-k={k.toUpperCase()} onclick={() => (toggles[k] = !toggles[k])}>
              {k.toUpperCase()}
            </button>
          {/each}
        </div>
      </div>
      <div class="findings scroll">
        {#each LIVE_FINDINGS as f}
          <div class="find {findClass(f.kind)}">
            <div class="fh"><span class="ft">{f.label}</span><span class="fts">{f.ts}</span></div>
            <!-- eslint-disable-next-line svelte/no-at-html-tags -->
            <div class="fc">{@html f.html}</div>
            {#if f.save}
              <button class="save">+ Save action</button>
            {/if}
          </div>
        {/each}
      </div>
      <div class="ask-bar">
        <div class="ic"><svg class="icon" viewBox="0 0 24 24"><path d="M12 2a4 4 0 0 1 4 4v6a4 4 0 0 1-8 0V6a4 4 0 0 1 4-4z" /><path d="M5 11a7 7 0 0 0 14 0M12 18v3" /></svg></div>
        <input placeholder="Ask AI about the call…  “summarize what we've agreed so far”" />
        <button class="btn btn-gold" aria-label="Send"><svg class="icon" viewBox="0 0 24 24"><path d="m5 12 14 0M13 6l6 6-6 6" /></svg></button>
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
  .live-title{font-weight:600;font-size:14px}
  .timer{font-family:var(--f-mono);font-size:15px;color:var(--ink);letter-spacing:.03em;font-weight:500}
  .dev{display:flex;align-items:center;gap:7px;font-family:var(--f-mono);font-size:11px;color:var(--ink-3);background:var(--bg-2);border:1px solid var(--line);border-radius:7px;padding:6px 10px;cursor:pointer;transition:.15s}
  .dev:hover{border-color:#3C362C;color:var(--ink-2)}
  .dev b{color:var(--ink-2);font-weight:500}
  .dev svg{color:var(--ink-4)}
  .cost{font-family:var(--f-mono);font-size:11px;color:var(--ink-3)}
  .cost b{color:var(--gold);font-weight:600}
  .ctrl{display:flex;gap:8px}

  .live-body{flex:1;display:flex;flex-direction:column;overflow:hidden}
  .tr-area{flex:1;overflow-y:auto;padding:24px 30px}
  .tr{display:flex;gap:18px;margin-bottom:21px;max-width:880px}
  .tr .ts{font-family:var(--f-mono);font-size:10.5px;color:var(--ink-4);width:62px;flex:none;padding-top:3px}
  .tr .who{font-family:var(--f-mono);font-size:10.5px;letter-spacing:.12em;text-transform:uppercase;font-weight:600;margin-bottom:5px}
  .tr .said{font-size:15px;line-height:1.62;color:var(--ink)}
  :global(.who-you){color:var(--gold)} :global(.who-sar){color:var(--commit)} :global(.who-ahm){color:var(--ask)}
  .tr.muted .said{color:var(--ink-2)}
  .listening{display:flex;align-items:center;gap:13px;padding:4px 0 8px 80px;opacity:.85}
  .eq{display:flex;align-items:flex-end;gap:3px;height:18px}
  .eq i{width:3px;background:var(--rec);border-radius:2px;animation:eq 1s infinite ease-in-out}
  .eq i:nth-child(1){animation-delay:0s;height:6px} .eq i:nth-child(2){animation-delay:.15s;height:14px}
  .eq i:nth-child(3){animation-delay:.3s;height:9px} .eq i:nth-child(4){animation-delay:.45s;height:17px}
  .eq i:nth-child(5){animation-delay:.6s;height:7px} .eq i:nth-child(6){animation-delay:.75s;height:12px}
  .listening span{font-family:var(--f-mono);font-size:10.5px;letter-spacing:.1em;text-transform:uppercase;color:var(--ink-3)}

  .ai{flex:none;height:316px;border-top:1px solid var(--line);background:linear-gradient(180deg,var(--bg-2),var(--bg-1));display:flex;flex-direction:column}
  .ai-head{flex:none;display:flex;align-items:center;gap:12px;padding:12px 18px;border-bottom:1px solid var(--line-soft)}
  .ai-head .lab{font-family:var(--f-mono);font-size:10px;letter-spacing:.2em;text-transform:uppercase;color:var(--ink-3)}
  .toggles{display:flex;gap:6px;margin-left:auto}
  .tg{width:30px;height:30px;border-radius:8px;border:1px solid var(--line);background:var(--bg-2);color:var(--ink-4);font-family:var(--f-mono);font-weight:600;font-size:12px;cursor:pointer;transition:.18s var(--ease);display:flex;align-items:center;justify-content:center}
  .tg:hover{color:var(--ink-2);border-color:#3C362C}
  .tg.on[data-k="F"]{background:rgba(240,166,64,.16);color:var(--fact);border-color:rgba(240,166,64,.4);box-shadow:0 0 14px -3px rgba(240,166,64,.5)}
  .tg.on[data-k="C"]{background:rgba(84,197,222,.16);color:var(--commit);border-color:rgba(84,197,222,.4);box-shadow:0 0 14px -3px rgba(84,197,222,.5)}
  .tg.on[data-k="S"]{background:rgba(138,196,121,.16);color:var(--suggest);border-color:rgba(138,196,121,.4);box-shadow:0 0 14px -3px rgba(138,196,121,.5)}
  .tg.on[data-k="Q"]{background:rgba(178,149,232,.16);color:var(--ask);border-color:rgba(178,149,232,.4);box-shadow:0 0 14px -3px rgba(178,149,232,.5)}
  .findings{flex:1;overflow-y:auto;padding:14px 18px;display:flex;flex-direction:column;gap:11px}
  .find{border:1px solid var(--line-soft);border-left-width:3px;border-radius:10px;background:var(--bg-2);padding:12px 14px;position:relative}
  .find.f-fact{border-left-color:var(--fact)} .find.f-commit{border-left-color:var(--commit)} .find.f-ask{border-left-color:var(--ask)}
  .find .fh{display:flex;align-items:center;gap:9px;margin-bottom:6px}
  .find .ft{font-family:var(--f-mono);font-size:9.5px;letter-spacing:.14em;text-transform:uppercase;font-weight:600}
  .f-fact .ft{color:var(--fact)} .f-commit .ft{color:var(--commit)} .f-ask .ft{color:var(--ask)}
  .find .fts{font-family:var(--f-mono);font-size:10px;color:var(--ink-4);margin-left:auto}
  .find .fc{font-size:13px;line-height:1.55;color:var(--ink-2)}
  .find .fc :global(b){color:var(--ink);font-weight:600}
  .find .save{position:absolute;right:12px;bottom:12px;font-family:var(--f-mono);font-size:10px;letter-spacing:.06em;text-transform:uppercase;color:var(--commit);background:rgba(84,197,222,.1);border:1px solid rgba(84,197,222,.28);border-radius:6px;padding:4px 9px;cursor:pointer;font-weight:600;transition:.15s}
  .find .save:hover{background:rgba(84,197,222,.2)}
  .ask-bar{flex:none;padding:12px 16px;border-top:1px solid var(--line-soft);display:flex;gap:10px;align-items:center;background:var(--bg-1)}
  .ask-bar .ic{width:30px;height:30px;border-radius:8px;background:var(--gold-soft);border:1px solid var(--gold-line);display:flex;align-items:center;justify-content:center;color:var(--gold);flex:none}
  .ask-bar input{flex:1;background:var(--bg-2);border:1px solid var(--line);border-radius:9px;padding:10px 14px;color:var(--ink);font-family:var(--f-ui);font-size:13px;outline:none;transition:.18s}
  .ask-bar input::placeholder{color:var(--ink-4)}
  .ask-bar input:focus{border-color:var(--gold-line);box-shadow:0 0 0 3px var(--gold-soft)}
</style>
