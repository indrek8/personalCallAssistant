<script lang="ts">
  import {
    sessions,
    selectedSessionId,
    selectedSession,
    navigate,
  } from "$lib/stores";
  import { labelClass, shortDate, longDate, fmtDuration } from "$lib/mock";
  import Mark from "$lib/components/Mark.svelte";

  let search = $state("");
  let segment = $state<"All" | "Acme" | "Globex">("All");

  const filtered = $derived(
    $sessions.filter((s) => {
      const name = (s.name ?? "Untitled session").toLowerCase();
      if (search && !name.includes(search.toLowerCase())) return false;
      if (segment === "All") return true;
      return s.labels.some((l) =>
        l.name.toLowerCase().includes(segment.toLowerCase()),
      );
    }),
  );

  function select(id: string) {
    selectedSessionId.set(id);
  }
</script>

<section class="screen">
  <div class="topbar rise r1">
    <div class="brand">
      <Mark />
      <h1>Call Assistant</h1>
    </div>
    <div class="search">
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="7" /><path d="m21 21-4.3-4.3" /></svg>
      <input placeholder="Search sessions…" bind:value={search} />
    </div>
    <div class="spacer"></div>
    <button class="btn btn-gold" onclick={() => navigate("new")}>
      <svg class="icon" viewBox="0 0 24 24"><path d="M12 5v14M5 12h14" /></svg>New Session
    </button>
    <button class="gear" aria-label="Settings" onclick={() => navigate("settings")}>
      <svg class="icon" viewBox="0 0 24 24"><circle cx="12" cy="12" r="3" /><path d="M12 2v3M12 19v3M2 12h3M19 12h3M5 5l2 2M17 17l2 2M5 19l2-2M17 7l2-2" /></svg>
    </button>
  </div>

  <div class="split">
    <!-- list pane -->
    <div class="list-pane rise r2">
      <div class="list-head">
        <div class="eyebrow">{filtered.length} session{filtered.length === 1 ? "" : "s"}</div>
        <div class="seg">
          {#each ["All", "Acme", "Globex"] as const as seg}
            <button class:on={segment === seg} onclick={() => (segment = seg)}>{seg}</button>
          {/each}
        </div>
      </div>
      <div class="list scroll">
        {#if filtered.length === 0}
          <div class="empty-list">
            <div class="el-title">No sessions yet</div>
            <p>Create your first session to start capturing calls.</p>
            <button class="btn btn-gold" onclick={() => navigate("new")}>
              <svg class="icon" viewBox="0 0 24 24"><path d="M12 5v14M5 12h14" /></svg>New Session
            </button>
          </div>
        {:else}
          {#each filtered as s (s.id)}
            <button class="s-row" class:sel={$selectedSessionId === s.id} onclick={() => select(s.id)}>
              <div class="top">
                <span class="name">{s.name ?? "Untitled session"}</span>
                <span class="date">{shortDate(s.date)}</span>
              </div>
              <div class="meta">
                <div class="labels">
                  {#each s.labels as l}
                    <span class="lbl {labelClass(l.name)}">{l.name}</span>
                  {/each}
                </div>
                <div class="right">
                  <span class="dur">{fmtDuration(s.duration_ms)}</span>
                </div>
              </div>
            </button>
          {/each}
        {/if}
      </div>
    </div>

    <!-- detail pane -->
    <div class="detail rise r3">
      {#if $selectedSession}
        {@const sel = $selectedSession}
        <div class="scroll">
          <div class="d-head">
            <div>
              <div class="d-title">{sel.name ?? "Untitled session"}</div>
              <div class="d-meta">
                <span>📅 <b>{longDate(sel.date)}</b></span>
                <span>⏱ <b>{fmtDuration(sel.duration_ms)}</b></span>
                <span>◈ <b>${sel.total_api_cost.toFixed(2)}</b></span>
              </div>
            </div>
            <button class="btn">
              <svg class="icon" viewBox="0 0 24 24"><path d="M21 12a9 9 0 1 1-3-6.7L21 8" /><path d="M21 3v5h-5" /></svg>Re-analyze
            </button>
          </div>

          <div class="sec">
            <div class="sec-h"><div class="eyebrow">Summary</div></div>
            <div class="card summary">
              Discussed <b>CBUAE Phase 2</b> timeline — the central bank extended the deadline to
              <b>Aug 2026</b>. KYC module certification is the critical dependency. Ahmed expects
              auditor results by <b>Apr 15</b>. A budget revision is needed for Q3 planning.
            </div>
          </div>

          <div class="sec">
            <div class="sec-h"><div class="eyebrow">Actions · 3 of 4 open</div></div>
            <div class="act"><span class="st st-done"></span><div class="body"><div class="t">Send CBUAE Phase 2 timeline to board</div><div class="sub">Me · due Apr 5 · done Apr 4</div></div><span class="tag tag-done">Done</span></div>
            <div class="act"><span class="st st-pend"></span><div class="body"><div class="t">Review KYC audit results</div><div class="sub">Me · due Apr 15</div></div><span class="tag tag-pend">Pending</span></div>
            <div class="act"><span class="st st-late"></span><div class="body"><div class="t">Send revised cost estimates</div><div class="sub">Ahmed · due Apr 8 · 2 days late</div></div><span class="tag tag-late">Late</span></div>
          </div>

          <div class="sec">
            <div class="sec-h"><div class="eyebrow">Transcript</div><button class="mini" onclick={() => navigate("live")}>Open playback</button></div>
            <div class="card" style="padding:8px 18px">
              <div class="tline"><div class="ts">00:00:05</div><div><div class="sp">You</div><div class="tx">Thanks everyone for joining. Let's nail down the CBUAE submission timeline today.</div></div></div>
              <hr class="divider" />
              <div class="tline"><div class="ts">00:03:28</div><div><div class="sp">Sarah</div><div class="tx">The central bank pushed their deadline to August — but there's a hard dependency on the KYC module being certified first.</div></div></div>
            </div>
          </div>
        </div>
      {:else}
        <div class="detail-empty">
          <Mark size={40} />
          <div class="de-title">Select a session</div>
          <p>Choose a session on the left to view its summary, actions, and transcript.</p>
        </div>
      {/if}
    </div>
  </div>
</section>

<style>
  .topbar{flex:none;height:60px;display:flex;align-items:center;gap:14px;padding:0 18px;border-bottom:1px solid var(--line-soft)}
  .brand{display:flex;align-items:center;gap:10px}
  .brand h1{font-family:var(--f-disp);font-weight:600;font-size:18px;letter-spacing:-.01em}
  .search{flex:1;max-width:340px;margin-left:6px;position:relative}
  .search input{width:100%;background:var(--bg-2);border:1px solid var(--line);border-radius:9px;padding:8px 12px 8px 34px;color:var(--ink);font-family:var(--f-ui);font-size:13px;outline:none;transition:.18s}
  .search input::placeholder{color:var(--ink-4)}
  .search input:focus{border-color:var(--gold-line);background:var(--bg-1);box-shadow:0 0 0 3px var(--gold-soft)}
  .search svg{position:absolute;left:11px;top:50%;transform:translateY(-50%);color:var(--ink-4)}
  .gear{width:34px;height:34px;border-radius:9px;display:flex;align-items:center;justify-content:center;color:var(--ink-3);cursor:pointer;transition:.18s;border:1px solid transparent;background:transparent}
  .gear:hover{background:var(--bg-2);color:var(--ink);border-color:var(--line)}

  .split{flex:1;display:flex;overflow:hidden}
  .list-pane{width:392px;flex:none;border-right:1px solid var(--line-soft);display:flex;flex-direction:column}
  .list-head{flex:none;display:flex;align-items:center;justify-content:space-between;padding:16px 18px 10px}
  .seg{display:flex;gap:2px;background:var(--bg-2);border:1px solid var(--line);border-radius:8px;padding:3px}
  .seg button{font-family:var(--f-mono);font-size:10px;letter-spacing:.08em;text-transform:uppercase;color:var(--ink-3);background:transparent;border:0;padding:4px 9px;border-radius:5px;cursor:pointer;transition:.15s}
  .seg button.on{background:var(--bg-3);color:var(--ink);box-shadow:0 1px 4px rgba(0,0,0,.3)}
  .list{flex:1;padding:4px 10px 14px}
  .s-row{display:block;width:100%;text-align:left;padding:13px 14px;border-radius:11px;cursor:pointer;position:relative;transition:.16s var(--ease);border:1px solid transparent;background:transparent;font-family:var(--f-ui);color:var(--ink)}
  .s-row:hover{background:var(--bg-2)}
  .s-row.sel{background:var(--bg-sel);border-color:var(--gold-line)}
  .s-row.sel::before{content:"";position:absolute;left:0;top:14px;bottom:14px;width:3px;border-radius:3px;background:var(--gold);box-shadow:0 0 12px var(--gold)}
  .s-row .top{display:flex;align-items:baseline;justify-content:space-between;gap:10px;margin-bottom:7px}
  .s-row .name{font-weight:600;font-size:14px;letter-spacing:-.01em}
  .s-row .date{font-family:var(--f-mono);font-size:10.5px;color:var(--ink-3);flex:none}
  .s-row .meta{display:flex;align-items:center;gap:8px}
  .labels{display:flex;gap:5px}
  .s-row .right{margin-left:auto;display:flex;align-items:center;gap:6px;font-family:var(--f-mono);font-size:10.5px;color:var(--ink-3)}
  .dur{font-family:var(--f-mono);font-size:10.5px;color:var(--ink-4)}

  .empty-list{padding:48px 22px;text-align:center;color:var(--ink-3)}
  .empty-list .el-title{font-family:var(--f-disp);font-weight:600;font-size:18px;color:var(--ink-2);margin-bottom:8px}
  .empty-list p{font-size:13px;line-height:1.6;margin-bottom:18px}

  .detail{flex:1;display:flex;flex-direction:column;overflow:hidden}
  .detail .scroll{padding:26px 32px 40px}
  .d-head{display:flex;align-items:flex-start;justify-content:space-between;gap:16px;margin-bottom:6px}
  .d-title{font-family:var(--f-disp);font-weight:600;font-size:30px;letter-spacing:-.02em;line-height:1.05}
  .d-meta{display:flex;gap:14px;margin-top:11px;font-family:var(--f-mono);font-size:11.5px;color:var(--ink-3)}
  .d-meta span{display:flex;align-items:center;gap:6px}
  .d-meta b{color:var(--ink-2);font-weight:500}
  .sec{margin-top:30px}
  .sec-h{display:flex;align-items:center;justify-content:space-between;margin-bottom:13px}
  .card{background:var(--bg-2);border:1px solid var(--line-soft);border-radius:var(--r-m);padding:17px 18px}
  .summary{font-size:14px;line-height:1.68;color:var(--ink-2)}
  .summary :global(b){color:var(--ink);font-weight:600}
  .act{display:flex;align-items:center;gap:13px;padding:13px 15px;border-radius:var(--r-m);border:1px solid var(--line-soft);background:var(--bg-2);margin-bottom:9px;transition:.16s}
  .act:hover{border-color:var(--line);background:var(--bg-3)}
  .st{width:9px;height:9px;border-radius:50%;flex:none}
  .st-done{background:var(--done);box-shadow:0 0 9px rgba(138,196,121,.5)}
  .st-pend{background:var(--pend);box-shadow:0 0 9px rgba(231,178,76,.4)}
  .st-late{background:var(--late);box-shadow:0 0 9px rgba(255,107,92,.5)}
  .act .body{flex:1;min-width:0}
  .act .t{font-size:13.5px;font-weight:500;margin-bottom:3px}
  .act .sub{font-family:var(--f-mono);font-size:10.5px;color:var(--ink-3)}
  .act .tag{font-family:var(--f-mono);font-size:9.5px;letter-spacing:.12em;text-transform:uppercase;padding:3px 8px;border-radius:5px;font-weight:600;flex:none}
  .tag-done{color:var(--done);background:rgba(138,196,121,.1)}
  .tag-pend{color:var(--pend);background:rgba(231,178,76,.1)}
  .tag-late{color:var(--late);background:rgba(255,107,92,.12)}
  .mini{font-family:var(--f-mono);font-size:11px;color:var(--ink-3);background:var(--bg-3);border:1px solid var(--line);border-radius:6px;padding:4px 9px;display:flex;align-items:center;gap:5px;cursor:pointer}
  .mini:hover{color:var(--ink-2)}
  .tline{display:flex;gap:14px;padding:9px 0}
  .tline .ts{font-family:var(--f-mono);font-size:10.5px;color:var(--ink-4);width:58px;flex:none;padding-top:2px}
  .tline .tx{font-size:13px;line-height:1.6;color:var(--ink-2)}
  .tline .sp{font-family:var(--f-mono);font-size:10px;letter-spacing:.1em;text-transform:uppercase;color:var(--ink-3);margin-bottom:3px}

  .detail-empty{flex:1;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:14px;text-align:center;color:var(--ink-3);padding:40px}
  .detail-empty .de-title{font-family:var(--f-disp);font-weight:600;font-size:22px;color:var(--ink-2)}
  .detail-empty p{font-size:13px;line-height:1.6;max-width:300px}
</style>
