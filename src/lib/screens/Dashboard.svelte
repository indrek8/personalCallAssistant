<script lang="ts">
  import {
    sessions,
    selectedSessionId,
    selectedSession,
    navigate,
    labels,
    resolveLabel,
    refreshSessions,
    postSessionId,
    postMode,
    pushToast,
  } from "$lib/stores";
  import { getSession, updateActionStatus, deleteSession, revealInFinder } from "$lib/ipc";
  import { shortDate, longDate, fmtDuration } from "$lib/format";
  import type { SessionFull, AnalysisAction, ActionStatus } from "$lib/types";
  import Mark from "$lib/components/Mark.svelte";
  import LabelChip from "$lib/components/LabelChip.svelte";
  import StatusPill from "$lib/components/StatusPill.svelte";
  import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
  import LabelManager from "$lib/components/LabelManager.svelte";

  let search = $state("");
  let labelFilter = $state<string | null>(null); // label id, or null = All

  const filtered = $derived(
    $sessions.filter((s) => {
      const name = (s.name ?? "Untitled session").toLowerCase();
      if (search && !name.includes(search.toLowerCase())) return false;
      if (labelFilter && !s.labels.some((l) => l.id === labelFilter)) return false;
      return true;
    }),
  );

  function select(id: string) {
    selectedSessionId.set(id);
  }

  // ---- detail pane: load the full session (meta + transcript + analysis) ----
  let detail = $state<SessionFull | null>(null);
  let detailState = $state<"idle" | "loading" | "loaded" | "error">("idle");
  let detailErr = $state("");
  let loadedId: string | null = null;

  async function loadDetail(id: string, silent = false) {
    if (!silent) detailState = "loading";
    loadedId = id;
    try {
      const full = await getSession(id);
      if (loadedId !== id) return; // a newer selection won the race
      detail = full;
      detailState = "loaded";
    } catch (e) {
      if (loadedId !== id) return;
      detailErr = String(e);
      detailState = "error";
    }
  }

  // Selection drives the detail fetch. An unreadable placeholder has no detail.
  $effect(() => {
    const id = $selectedSessionId;
    const sel = $selectedSession;
    if (!id || sel?.unreadable) {
      detail = null;
      detailState = "idle";
      loadedId = null;
      return;
    }
    if (id !== loadedId) void loadDetail(id);
  });

  const openCount = $derived(
    (detail?.analysis?.actions ?? []).filter((a) => a.status !== "done" && a.status !== "wont_do").length,
  );

  async function setStatus(actionId: string, status: ActionStatus) {
    const id = $selectedSessionId;
    if (!id) return;
    try {
      await updateActionStatus(id, actionId, status);
      await loadDetail(id, true); // silent re-fetch — no spinner flash
    } catch (e) {
      pushToast(`Could not update status: ${String(e)}`, { kind: "error" });
    }
  }

  // ---- re-analyze / resume / delete ----
  let confirmKind = $state<null | "reanalyze" | "delete">(null);
  let showLabels = $state(false);
  let showTranscript = $state(false);

  function goAnalyze(mode: "reanalyze" | "resume") {
    const id = $selectedSessionId;
    if (!id) return;
    confirmKind = null;
    postMode.set(mode);
    postSessionId.set(id);
    navigate("post");
  }

  async function doDelete() {
    const id = $selectedSessionId;
    if (!id) return;
    confirmKind = null;
    try {
      await deleteSession(id);
      selectedSessionId.set(null);
      detail = null;
      detailState = "idle";
      loadedId = null;
      await refreshSessions();
    } catch (e) {
      pushToast(`Could not delete session: ${String(e)}`, { kind: "error" });
    }
  }

  async function reveal() {
    try {
      await revealInFinder(); // opens the storage directory
    } catch (e) {
      pushToast(`Could not reveal in Finder: ${String(e)}`, { kind: "error" });
    }
  }

  function ownerOf(a: AnalysisAction): string {
    return a.owner?.trim() || (a.owner_type === "mine" ? "Me" : "Them");
  }

  function actionSub(a: AnalysisAction): string {
    const parts = [ownerOf(a)];
    if (a.deadline) parts.push(`due ${a.deadline}`);
    if (a.completed_at) parts.push(`done ${shortDate(a.completed_at)}`);
    return parts.join(" · ");
  }

  /** ms from capture start → "m:ss" / "h:mm:ss". */
  function clock(ms: number): string {
    const s = Math.floor(ms / 1000);
    const sec = s % 60;
    const m = Math.floor(s / 60) % 60;
    const h = Math.floor(s / 3600);
    return h > 0
      ? `${h}:${String(m).padStart(2, "0")}:${String(sec).padStart(2, "0")}`
      : `${m}:${String(sec).padStart(2, "0")}`;
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
        <button class="manage" onclick={() => (showLabels = true)}>Manage labels</button>
      </div>
      <div class="filter-row">
        <button class="fchip" class:on={labelFilter === null} onclick={() => (labelFilter = null)}>All</button>
        {#each $labels as l (l.id)}
          <button class="fchip" class:on={labelFilter === l.id} onclick={() => (labelFilter = l.id)}>
            <LabelChip label={l} />
          </button>
        {/each}
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
            <button class="s-row" class:sel={$selectedSessionId === s.id} class:bad={s.unreadable} onclick={() => select(s.id)}>
              <div class="top">
                <span class="name">{s.unreadable ? "⚠ Unreadable session" : (s.name ?? "Untitled session")}</span>
                {#if !s.unreadable}<span class="date">{shortDate(s.date)}</span>{/if}
              </div>
              <div class="meta">
                <div class="labels">
                  {#if s.unreadable}
                    <span class="bad-id">{s.id}</span>
                  {:else}
                    {#each s.labels as l (l.id)}<LabelChip label={resolveLabel(l, $labels)} />{/each}
                    {#if s.status === "reviewing"}<span class="badge-review">Needs review</span>{/if}
                  {/if}
                </div>
                <div class="right">
                  {#if !s.unreadable}<span class="dur">{fmtDuration(s.duration_ms)}</span>{/if}
                </div>
              </div>
            </button>
          {/each}
        {/if}
      </div>
    </div>

    <!-- detail pane -->
    <div class="detail rise r3">
      {#if $selectedSession?.unreadable}
        {@const sel = $selectedSession}
        <div class="detail-empty">
          <div class="de-warn">⚠</div>
          <div class="de-title">Unreadable session</div>
          <p>This session's <code>metadata.json</code> couldn't be parsed. Its folder is named <code>{sel.id}</code>.</p>
          <div class="de-actions">
            <button class="btn" onclick={reveal}>Reveal in Finder</button>
            <button class="btn btn-danger" onclick={() => (confirmKind = "delete")}>Delete</button>
          </div>
        </div>
      {:else if detailState === "loading" || detailState === "idle" && $selectedSessionId}
        <div class="center"><div class="spinner" aria-hidden="true"></div><div class="c-sub">Loading…</div></div>
      {:else if detailState === "error"}
        <div class="detail-empty">
          <div class="de-title">Couldn't load this session</div>
          <p class="c-err">{detailErr}</p>
          <button class="btn" onclick={() => $selectedSessionId && loadDetail($selectedSessionId)}>Retry</button>
        </div>
      {:else if detailState === "loaded" && detail}
        {@const a = detail.analysis}
        <div class="scroll">
          <div class="d-head">
            <div>
              <div class="d-title">{detail.meta.name ?? "Untitled session"}</div>
              <div class="d-meta">
                <span>📅 <b>{longDate(detail.meta.date)}</b></span>
                <span>⏱ <b>{fmtDuration(detail.meta.duration_ms)}</b></span>
                <span>◈ <b>${detail.meta.total_api_cost.toFixed(2)}</b></span>
              </div>
            </div>
            <div class="d-actions">
              {#if a}
                <button class="btn" onclick={() => (confirmKind = "reanalyze")}>
                  <svg class="icon" viewBox="0 0 24 24"><path d="M21 12a9 9 0 1 1-3-6.7L21 8" /><path d="M21 3v5h-5" /></svg>Re-analyze
                </button>
              {/if}
              <button class="btn btn-danger" aria-label="Delete session" onclick={() => (confirmKind = "delete")}>
                <svg class="icon" viewBox="0 0 24 24"><path d="M3 6h18M8 6V4h8v2M19 6l-1 14H6L5 6" /></svg>
              </button>
            </div>
          </div>

          {#if detail.meta.status === "reviewing"}
            <div class="review-note">
              <span>This analysis is a draft awaiting review.</span>
              <button class="btn btn-gold sm" onclick={() => goAnalyze("resume")}>Resume review</button>
            </div>
          {/if}

          {#if !a}
            <div class="none-an">
              <p>This session hasn't been analyzed yet.</p>
              <button class="btn btn-gold" onclick={() => goAnalyze("reanalyze")}>Analyze now</button>
            </div>
          {:else}
            <div class="sec">
              <div class="sec-h"><div class="eyebrow">Summary</div></div>
              <div class="card summary">{a.summary || "No summary."}</div>
            </div>

            <div class="sec">
              <div class="sec-h"><div class="eyebrow">Actions · {openCount} of {a.actions.length} open</div></div>
              {#if a.actions.length === 0}
                <div class="none">No actions were extracted.</div>
              {:else}
                {#each a.actions as act (act.id)}
                  <div class="act">
                    <div class="body">
                      <div class="t">{act.title}</div>
                      <div class="sub">{actionSub(act)}</div>
                      {#if act.transcript_quote}<div class="quote">“{act.transcript_quote}”</div>{/if}
                    </div>
                    <StatusPill status={act.status} onChange={(s) => setStatus(act.id, s)} />
                  </div>
                {/each}
              {/if}
            </div>

            {#if a.decisions.length}
              <div class="sec">
                <div class="sec-h"><div class="eyebrow">Decisions</div></div>
                <div class="card decisions">
                  <ul>{#each a.decisions as d}<li>{d}</li>{/each}</ul>
                </div>
              </div>
            {/if}
          {/if}

          <div class="sec">
            <div class="sec-h">
              <div class="eyebrow">Transcript · {detail.transcript.length} lines</div>
              {#if detail.transcript.length}
                <button class="mini" onclick={() => (showTranscript = !showTranscript)}>
                  {showTranscript ? "Hide" : "Show"} transcript
                </button>
              {/if}
            </div>
            {#if showTranscript}
              <div class="card" style="padding:8px 18px">
                {#if detail.transcript.length === 0}
                  <div class="none">No transcript.</div>
                {:else}
                  {#each detail.transcript as e, i (e.id)}
                    {#if i > 0}<hr class="divider" />{/if}
                    <div class="tline">
                      <div class="ts">{clock(e.t_ms)}</div>
                      <div><div class="sp">{e.stream === "you" ? "You" : "Remote"}</div><div class="tx">{e.text}</div></div>
                    </div>
                  {/each}
                {/if}
              </div>
            {/if}
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

{#if confirmKind === "reanalyze"}
  <ConfirmDialog
    title="Re-analyze this session?"
    message="Re-runs Sonnet extraction and overwrites the saved analysis. This bills one API call."
    confirmLabel="Re-analyze"
    onConfirm={() => goAnalyze("reanalyze")}
    onCancel={() => (confirmKind = null)}
  />
{:else if confirmKind === "delete"}
  <ConfirmDialog
    title="Delete this session?"
    message="Permanently removes the recording, transcript, and analysis. This cannot be undone."
    confirmLabel="Delete"
    destructive
    onConfirm={doDelete}
    onCancel={() => (confirmKind = null)}
  />
{/if}

{#if showLabels}
  <LabelManager onClose={() => (showLabels = false)} />
{/if}

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
  .list-head{flex:none;display:flex;align-items:center;justify-content:space-between;padding:16px 18px 8px}
  .manage{font-family:var(--f-mono);font-size:10px;letter-spacing:.06em;text-transform:uppercase;color:var(--ink-3);background:transparent;border:0;cursor:pointer;padding:4px 6px;border-radius:5px}
  .manage:hover{color:var(--ink);background:var(--bg-2)}
  .filter-row{flex:none;display:flex;gap:6px;flex-wrap:wrap;padding:0 18px 12px;border-bottom:1px solid var(--line-soft)}
  .fchip{display:inline-flex;align-items:center;background:var(--bg-2);border:1px solid var(--line);border-radius:7px;padding:4px 8px;cursor:pointer;font-family:var(--f-mono);font-size:10px;letter-spacing:.06em;text-transform:uppercase;color:var(--ink-3);transition:.15s}
  .fchip.on{border-color:var(--gold-line);background:var(--bg-sel);color:var(--ink)}
  .fchip:hover{color:var(--ink)}

  .list{flex:1;padding:8px 10px 14px}
  .s-row{display:block;width:100%;text-align:left;padding:13px 14px;border-radius:11px;cursor:pointer;position:relative;transition:.16s var(--ease);border:1px solid transparent;background:transparent;font-family:var(--f-ui);color:var(--ink)}
  .s-row:hover{background:var(--bg-2)}
  .s-row.sel{background:var(--bg-sel);border-color:var(--gold-line)}
  .s-row.sel::before{content:"";position:absolute;left:0;top:14px;bottom:14px;width:3px;border-radius:3px;background:var(--gold);box-shadow:0 0 12px var(--gold)}
  .s-row.bad .name{color:var(--late)}
  .s-row .top{display:flex;align-items:baseline;justify-content:space-between;gap:10px;margin-bottom:7px}
  .s-row .name{font-weight:600;font-size:14px;letter-spacing:-.01em}
  .s-row .date{font-family:var(--f-mono);font-size:10.5px;color:var(--ink-3);flex:none}
  .s-row .meta{display:flex;align-items:center;gap:8px}
  .labels{display:flex;gap:5px;align-items:center;flex-wrap:wrap}
  .bad-id{font-family:var(--f-mono);font-size:10px;color:var(--ink-4)}
  .badge-review{font-family:var(--f-mono);font-size:9px;letter-spacing:.08em;text-transform:uppercase;color:var(--pend);background:rgba(231,178,76,.1);padding:2px 6px;border-radius:4px}
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
  .d-actions{display:flex;gap:8px;flex:none}
  .btn-danger{background:rgba(255,107,92,.1);border:1px solid rgba(255,107,92,.32);color:#ff8278}
  .btn-danger:hover{background:rgba(255,107,92,.2)}

  .review-note{display:flex;align-items:center;justify-content:space-between;gap:14px;margin-top:22px;padding:12px 16px;border-radius:var(--r-m);background:rgba(231,178,76,.08);border:1px solid var(--gold-line);font-size:13px;color:var(--ink-2)}
  .sm{padding:6px 12px;font-size:12px}
  .none-an{margin-top:30px;padding:30px;text-align:center;border:1px dashed var(--line);border-radius:var(--r-m);color:var(--ink-3)}
  .none-an p{margin-bottom:14px;font-size:13.5px}

  .sec{margin-top:30px}
  .sec-h{display:flex;align-items:center;justify-content:space-between;margin-bottom:13px}
  .card{background:var(--bg-2);border:1px solid var(--line-soft);border-radius:var(--r-m);padding:17px 18px}
  .summary{font-size:14px;line-height:1.68;color:var(--ink-2);white-space:pre-wrap}
  .decisions ul{margin:0;padding-left:18px}
  .decisions li{font-size:13.5px;line-height:1.7;color:var(--ink-2)}
  .none{font-size:13px;color:var(--ink-3);padding:6px 2px}
  .act{display:flex;align-items:center;gap:13px;padding:13px 15px;border-radius:var(--r-m);border:1px solid var(--line-soft);background:var(--bg-2);margin-bottom:9px;transition:.16s}
  .act:hover{border-color:var(--line);background:var(--bg-3)}
  .act .body{flex:1;min-width:0}
  .act .t{font-size:13.5px;font-weight:500;margin-bottom:3px}
  .act .sub{font-family:var(--f-mono);font-size:10.5px;color:var(--ink-3)}
  .act .quote{font-size:12px;color:var(--ink-3);font-style:italic;margin-top:5px;line-height:1.5}
  .mini{font-family:var(--f-mono);font-size:11px;color:var(--ink-3);background:var(--bg-3);border:1px solid var(--line);border-radius:6px;padding:4px 9px;display:flex;align-items:center;gap:5px;cursor:pointer}
  .mini:hover{color:var(--ink-2)}
  .tline{display:flex;gap:14px;padding:9px 0}
  .tline .ts{font-family:var(--f-mono);font-size:10.5px;color:var(--ink-4);width:58px;flex:none;padding-top:2px}
  .tline .tx{font-size:13px;line-height:1.6;color:var(--ink-2)}
  .tline .sp{font-family:var(--f-mono);font-size:10px;letter-spacing:.1em;text-transform:uppercase;color:var(--ink-3);margin-bottom:3px}

  .center{flex:1;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:14px;color:var(--ink-3)}
  .spinner{width:30px;height:30px;border:3px solid var(--line);border-top-color:var(--gold);border-radius:50%;animation:spin .9s linear infinite}
  .c-sub{font-family:var(--f-mono);font-size:11px;letter-spacing:.14em;text-transform:uppercase}
  .c-err{font-size:12.5px;color:var(--late);line-height:1.5;max-width:360px;margin:0 auto 14px}
  @keyframes spin{to{transform:rotate(360deg)}}

  .detail-empty{flex:1;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:14px;text-align:center;color:var(--ink-3);padding:40px}
  .detail-empty .de-title{font-family:var(--f-disp);font-weight:600;font-size:22px;color:var(--ink-2)}
  .detail-empty p{font-size:13px;line-height:1.6;max-width:340px}
  .detail-empty code{font-family:var(--f-mono);font-size:11.5px;color:var(--ink-2)}
  .de-warn{font-size:32px}
  .de-actions{display:flex;gap:10px;margin-top:6px}
</style>
