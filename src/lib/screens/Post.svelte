<script lang="ts">
  import { navigate, refreshSessions } from "$lib/stores";

  // M1: post-analysis is mocked. The action checkboxes toggle locally;
  // Save & Close returns to the dashboard (refreshing the real list).
  let actions = $state([
    { title: "Send CBUAE Phase 2 timeline to board", owner: "Me", due: "Apr 5", quote: "“I'll circulate the updated timeline to the board by Friday”", checked: true, faint: false },
    { title: "Deliver KYC audit results", owner: "Ahmed", due: "Apr 15", quote: "“Expecting results by April 15th”", checked: true, faint: false },
    { title: "Prepare budget revision for Q3", owner: "Me", due: "Apr 10", quote: "“Let's get the revised numbers together before the next board”", checked: true, faint: false },
    { title: "Share updated risk assessment", owner: "Sarah", due: "— set", quote: "AI suggestion, not a clear commitment — leave unchecked to discard.", checked: false, faint: true },
  ]);

  async function saveAndClose() {
    await refreshSessions();
    navigate("dashboard");
  }
</script>

<section class="screen">
  <div class="duo">
    <div class="post-main scroll rise r1">
      <div class="eyebrow">Review &amp; save</div>
      <h2>Post-Analysis</h2>

      <div class="sec-h">
        <div class="eyebrow">Summary</div>
        <button class="mini"><svg class="icon" width="12" height="12" viewBox="0 0 24 24"><path d="M21 12a9 9 0 1 1-3-6.7L21 8" /><path d="M21 3v5h-5" /></svg>Regenerate</button>
      </div>
      <div class="card summary" style="margin-bottom:32px">
        Discussed <b>CBUAE Phase 2</b> timeline — central bank extended the deadline to
        <b>Aug 2026</b>. KYC module certification is the critical dependency. Ahmed expects auditor
        results by <b>Apr 15</b>. Budget revision needed for Q3 planning.
      </div>

      <div class="sec-h"><div class="eyebrow">Extracted actions · {actions.length} found</div></div>
      {#each actions as a}
        <div class="act-edit" style={a.faint && !a.checked ? "opacity:.6" : ""}>
          <button class="ck" class:on={a.checked} aria-label="Toggle action" onclick={() => (a.checked = !a.checked)}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><path d="m5 12 5 5 9-11" /></svg>
          </button>
          <div class="ae-body">
            <div class="ae-top">
              <div class="ae-title">{a.title}</div>
              <div class="ae-controls"><span class="mini">{a.owner} ▾</span><span class="mini">{a.due}</span></div>
            </div>
            <div class="quote">{a.quote}</div>
          </div>
        </div>
      {/each}
    </div>

    <div class="post-rail rise r2">
      <div class="pr-body scroll">
        <div class="meta-card">
          <div class="mc-t">Board Call Q2</div>
          <div class="mc-row"><span>DATE</span><b>Mar 28, 2026</b></div>
          <div class="mc-row"><span>DURATION</span><b>47 min</b></div>
          <div class="mc-row"><span>PARTICIPANTS</span><b>3</b></div>
          <div class="mc-row"><span>API COST</span><b style="color:var(--gold)">$0.85</b></div>
        </div>
        <div class="sec-h"><div class="eyebrow">Decisions</div></div>
        <div class="dec">Phase 2 deadline moved to Aug 2026</div>
        <div class="dec">Will proceed with the current KYC vendor</div>
        <div class="dec">Q3 budget revision triggered</div>
      </div>
      <div class="pr-foot">
        <button class="btn btn-gold" style="justify-content:center" onclick={saveAndClose}>Save &amp; Close</button>
        <button class="btn btn-ghost" style="justify-content:center" onclick={() => navigate("live")}>Back to Transcript</button>
      </div>
    </div>
  </div>
</section>

<style>
  .duo{flex:1;display:flex;overflow:hidden}
  .post-main{flex:1;overflow-y:auto;padding:34px 44px 50px}
  .post-main>.eyebrow{margin-bottom:10px;display:block}
  .post-main h2{font-family:var(--f-disp);font-weight:600;font-size:34px;letter-spacing:-.025em;line-height:1;margin-bottom:30px}
  .sec-h{display:flex;align-items:center;justify-content:space-between;margin-bottom:13px}
  .card{background:var(--bg-2);border:1px solid var(--line-soft);border-radius:var(--r-m);padding:17px 18px}
  .summary{font-size:14px;line-height:1.68;color:var(--ink-2)}
  .summary :global(b){color:var(--ink);font-weight:600}
  .mini{font-family:var(--f-mono);font-size:11px;color:var(--ink-3);background:var(--bg-3);border:1px solid var(--line);border-radius:6px;padding:4px 9px;display:flex;align-items:center;gap:5px;cursor:pointer}
  .mini:hover{color:var(--ink-2)}

  .act-edit{display:flex;gap:13px;align-items:flex-start;padding:15px 16px;border:1px solid var(--line-soft);background:var(--bg-2);border-radius:var(--r-m);margin-bottom:10px}
  .ck{width:19px;height:19px;border-radius:6px;border:1.5px solid var(--line);flex:none;margin-top:1px;cursor:pointer;display:flex;align-items:center;justify-content:center;transition:.15s;background:transparent;padding:0}
  .ck.on{background:var(--gold);border-color:var(--gold)}
  .ck.on svg{color:#27200C;opacity:1}
  .ck svg{opacity:0;width:12px;height:12px}
  .ae-body{flex:1}
  .ae-top{display:flex;align-items:center;justify-content:space-between;gap:12px;margin-bottom:6px}
  .ae-title{font-size:14px;font-weight:600}
  .ae-controls{display:flex;align-items:center;gap:8px;flex:none}
  .quote{font-family:var(--f-disp);font-style:italic;font-size:13px;color:var(--ink-3);line-height:1.5;padding-left:12px;border-left:2px solid var(--line);margin-top:8px}

  .post-rail{width:340px;flex:none;border-left:1px solid var(--line-soft);background:var(--bg-2);display:flex;flex-direction:column}
  .post-rail .pr-body{flex:1;overflow-y:auto;padding:30px 26px}
  .post-rail .pr-foot{padding:16px 22px;border-top:1px solid var(--line-soft);display:flex;flex-direction:column;gap:9px}
  .meta-card{border:1px solid var(--line-soft);border-radius:var(--r-m);padding:6px 16px;margin-bottom:28px;background:var(--bg-1)}
  .meta-card .mc-t{font-family:var(--f-disp);font-weight:600;font-size:18px;margin:12px 0 6px;letter-spacing:-.01em}
  .meta-card .mc-row{display:flex;justify-content:space-between;align-items:center;font-family:var(--f-mono);font-size:11.5px;padding:9px 0;color:var(--ink-3);border-top:1px solid var(--line-soft)}
  .meta-card .mc-row b{color:var(--ink-2);font-weight:500}
  .dec{display:flex;gap:11px;align-items:flex-start;padding:8px 0;font-size:13.5px;color:var(--ink-2);line-height:1.5}
  .dec::before{content:"";width:5px;height:5px;border-radius:50%;background:var(--gold);margin-top:8px;flex:none;box-shadow:0 0 8px var(--gold)}
</style>
