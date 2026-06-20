<script lang="ts">
  import { onMount } from "svelte";
  import { get } from "svelte/store";
  import {
    navigate,
    refreshSessions,
    postSessionId,
    selectedSessionId,
    analysisPhase,
    banner,
  } from "$lib/stores";
  import { isTauri, getSession, runPostAnalysis, saveAnalysis } from "$lib/ipc";
  import type {
    Analysis,
    ActionType,
    ActionStatus,
    OwnerType,
    CreatedBy,
    SessionMeta,
    TranscriptEntry,
  } from "$lib/types";

  // M4: real post-analysis. On enter we run `run_post_analysis` over the just-ended
  // session, re-fetch the draft via `get_session` (D18), and let the user edit it
  // before Save & Close persists it and marks the session `completed`.

  type EditRow = {
    id: string;
    title: string;
    owner: string;
    owner_type: OwnerType;
    type: ActionType;
    status: ActionStatus;
    deadline: string; // "" = none (→ null on save)
    transcript_quote: string;
    transcript_t_ms: number;
    notes: string | null;
    created_by: CreatedBy;
    completed_at: string | null;
    include: boolean;
  };

  let phase = $state<"analyzing" | "review" | "error">("analyzing");
  let errorMsg = $state("");
  let saving = $state(false);
  let showTranscript = $state(false);

  let sessionId = "";
  let meta = $state<SessionMeta | null>(null);
  let transcriptEntries = $state<TranscriptEntry[]>([]);

  let summary = $state("");
  let rows = $state<EditRow[]>([]);
  let decisions = $state<string[]>([]);
  let keyTopics = $state<string[]>([]);
  let generatedAt = "";

  onMount(() => {
    const sid = get(postSessionId) ?? get(selectedSessionId);
    if (!sid) {
      phase = "error";
      errorMsg = "No session to analyze.";
      return;
    }
    sessionId = sid;
    if (!isTauri()) {
      loadMock();
      return;
    }
    void runAnalysis(true);
  });

  async function runAnalysis(loadMeta: boolean) {
    phase = "analyzing";
    analysisPhase.set("analyzing");
    try {
      if (loadMeta) {
        const full = await getSession(sessionId);
        meta = full.meta;
        transcriptEntries = full.transcript;
      }
      await runPostAnalysis(sessionId);
      // Re-fetch the freshly-persisted draft + updated cost (D18).
      const full = await getSession(sessionId);
      meta = full.meta;
      applyDraft(full.analysis);
      analysisPhase.set("reviewing");
      phase = "review";
    } catch (e) {
      errorMsg = String(e);
      analysisPhase.set("error");
      phase = "error";
    }
  }

  function applyDraft(a: Analysis | null) {
    summary = a?.summary ?? "";
    decisions = a?.decisions ?? [];
    keyTopics = a?.key_topics ?? [];
    generatedAt = a?.generated_at ?? "";
    rows = (a?.actions ?? []).map((x) => ({
      id: x.id,
      title: x.title,
      owner: x.owner,
      owner_type: x.owner_type,
      type: x.type,
      status: x.status,
      deadline: x.deadline ?? "",
      transcript_quote: x.transcript_quote,
      transcript_t_ms: x.transcript_t_ms,
      notes: x.notes ?? null,
      created_by: x.created_by,
      completed_at: x.completed_at ?? null,
      include: true,
    }));
  }

  function buildAnalysis(includeOnly: boolean): Analysis {
    const src = includeOnly ? rows.filter((r) => r.include) : rows;
    return {
      summary: summary.trim(),
      actions: src.map((r) => ({
        id: r.id,
        title: r.title.trim(),
        owner: r.owner.trim(),
        owner_type: r.owner_type,
        type: r.type,
        status: r.status,
        deadline: r.deadline.trim() ? r.deadline.trim() : null,
        transcript_quote: r.transcript_quote,
        transcript_t_ms: r.transcript_t_ms,
        notes: r.notes,
        created_by: r.created_by,
        completed_at: r.completed_at,
      })),
      decisions,
      key_topics: keyTopics,
      generated_at: generatedAt || new Date().toISOString(),
    };
  }

  async function persist(analysis: Analysis) {
    saving = true;
    try {
      if (isTauri()) await saveAnalysis(sessionId, analysis);
      await refreshSessions();
      postSessionId.set(null);
      analysisPhase.set("idle");
      navigate("dashboard");
    } catch (e) {
      banner.set(`Could not save: ${String(e)}`);
      saving = false;
    }
  }

  const saveAndClose = () => persist(buildAnalysis(true));
  const saveWithoutAnalysis = () =>
    persist({ summary: "", actions: [], decisions: [], key_topics: [], generated_at: new Date().toISOString() });

  function addAction() {
    rows = [
      ...rows,
      {
        id: crypto.randomUUID(),
        title: "",
        owner: "Me",
        owner_type: "mine",
        type: "follow_up",
        status: "pending",
        deadline: "",
        transcript_quote: "",
        transcript_t_ms: 0,
        notes: null,
        created_by: "manual",
        completed_at: null,
        include: true,
      },
    ];
  }

  const removeAction = (id: string) => (rows = rows.filter((r) => r.id !== id));

  function ownerTypeOf(owner: string): OwnerType {
    const o = owner.trim().toLowerCase();
    return o === "me" || o === "you" || o === "i" || o === "myself" ? "mine" : "theirs";
  }
  function setOwner(row: EditRow, owner: string) {
    row.owner = owner;
    row.owner_type = ownerTypeOf(owner);
  }
  function optionsFor(owner: string): string[] {
    const base = ["Me", ...(meta?.participants ?? [])].map((s) => s.trim()).filter(Boolean);
    const uniq: string[] = [];
    for (const o of base) if (!uniq.some((u) => u.toLowerCase() === o.toLowerCase())) uniq.push(o);
    if (owner.trim() && !uniq.some((u) => u.toLowerCase() === owner.trim().toLowerCase())) uniq.push(owner.trim());
    return uniq;
  }

  const includedCount = $derived(rows.filter((r) => r.include).length);

  function fmtDate(iso: string): string {
    if (!iso) return "—";
    const d = new Date(iso);
    return isNaN(d.getTime())
      ? iso
      : d.toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
  }
  const fmtDuration = (ms: number) => `${Math.max(0, Math.round(ms / 60000))} min`;
  function fmtCost(c: number): string {
    if (!c || c <= 0) return "$0.00";
    return c < 0.01 ? "$" + c.toFixed(4) : "$" + c.toFixed(2);
  }
  function fmtTs(ms: number): string {
    const t = Math.floor(ms / 1000);
    return `${String(Math.floor(t / 60)).padStart(2, "0")}:${String(t % 60).padStart(2, "0")}`;
  }
  const typeLabel = (t: ActionType) =>
    t === "commitment" ? "Commitment" : t === "follow_up" ? "Follow-up" : "Suggestion";

  // Browser-preview fallback (no backend): render the review layout with sample data.
  function loadMock() {
    meta = {
      id: sessionId,
      status: "reviewing",
      name: "Board Call Q2",
      labels: [],
      date: "2026-03-28T00:00:00Z",
      duration_ms: 47 * 60_000,
      participants: ["Sarah", "Ahmed"],
      context_notes: null,
      budget_cap: null,
      total_api_cost: 0.85,
    };
    applyDraft({
      summary:
        "Discussed CBUAE Phase 2 timeline — central bank extended the deadline to Aug 2026. KYC module certification is the critical dependency. Ahmed expects auditor results by Apr 15.",
      actions: [
        {
          id: "m1",
          title: "Send CBUAE Phase 2 timeline to board",
          owner: "Me",
          owner_type: "mine",
          type: "commitment",
          status: "pending",
          deadline: "Apr 5",
          transcript_quote: "I'll circulate the updated timeline to the board by Friday",
          transcript_t_ms: 0,
          notes: null,
          created_by: "ai_extracted",
          completed_at: null,
        },
        {
          id: "m2",
          title: "Deliver KYC audit results",
          owner: "Ahmed",
          owner_type: "theirs",
          type: "commitment",
          status: "pending",
          deadline: "Apr 15",
          transcript_quote: "Expecting results by April 15th",
          transcript_t_ms: 0,
          notes: null,
          created_by: "ai_extracted",
          completed_at: null,
        },
      ],
      decisions: ["Phase 2 deadline moved to Aug 2026", "Will proceed with the current KYC vendor"],
      key_topics: ["timeline", "KYC", "budget"],
      generated_at: "2026-03-28T00:00:00Z",
    });
    phase = "review";
  }
</script>

<section class="screen">
  {#if phase === "analyzing"}
    <div class="center">
      <div class="spinner" aria-hidden="true"></div>
      <div class="c-title">Analyzing your session…</div>
      {#if meta}
        <div class="c-sub">{meta.name ?? "Session"} · {fmtDuration(meta.duration_ms)}</div>
      {/if}
      <div class="c-note">Extracting a summary, actions, and decisions with Claude.</div>
    </div>
  {:else if phase === "error"}
    <div class="center">
      <div class="c-title">Analysis didn’t complete</div>
      <div class="c-err">{errorMsg}</div>
      <div class="err-actions">
        <button class="btn btn-gold" onclick={() => runAnalysis(false)}>Retry analysis</button>
        <button class="btn btn-ghost" disabled={saving} onclick={saveWithoutAnalysis}>Save without analysis</button>
        <button class="btn btn-ghost" onclick={() => navigate("dashboard")}>Back to dashboard</button>
      </div>
      <div class="c-note">Your transcript and recording are safe either way.</div>
    </div>
  {:else}
    <div class="duo">
      <div class="post-main scroll rise r1">
        <div class="eyebrow">Review &amp; save</div>
        <h2>Post-Analysis</h2>

        <div class="sec-h">
          <div class="eyebrow">Summary</div>
          <button class="mini" disabled={saving} onclick={() => runAnalysis(false)} title="Re-run extraction (uses one Sonnet call)">
            <svg class="icon" width="12" height="12" viewBox="0 0 24 24"><path d="M21 12a9 9 0 1 1-3-6.7L21 8" /><path d="M21 3v5h-5" /></svg>Regenerate
          </button>
        </div>
        <textarea class="summary-edit" bind:value={summary} placeholder="Session summary…"></textarea>

        <div class="sec-h" style="margin-top:30px">
          <div class="eyebrow">Extracted actions · {includedCount} of {rows.length}</div>
          <button class="mini" onclick={addAction}>+ Add action</button>
        </div>

        {#each rows as a (a.id)}
          <div class="act-edit" style={a.include ? "" : "opacity:.5"}>
            <button class="ck" class:on={a.include} aria-label="Include action" onclick={() => (a.include = !a.include)}>
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><path d="m5 12 5 5 9-11" /></svg>
            </button>
            <div class="ae-body">
              <div class="ae-top">
                <input class="ae-title-input" bind:value={a.title} placeholder="Action title" />
                <button class="del" aria-label="Delete action" onclick={() => removeAction(a.id)}>×</button>
              </div>
              <div class="ae-controls">
                <span class="tag tag-{a.type}">{typeLabel(a.type)}</span>
                <select class="ae-select" value={a.owner} onchange={(e) => setOwner(a, e.currentTarget.value)}>
                  {#each optionsFor(a.owner) as opt}<option value={opt}>{opt}</option>{/each}
                </select>
                <input class="ae-due" value={a.deadline} oninput={(e) => (a.deadline = e.currentTarget.value)} placeholder="Due —" />
                {#if a.created_by === "manual"}<span class="tag tag-manual">added</span>{/if}
              </div>
              {#if a.transcript_quote}<div class="quote">“{a.transcript_quote}”</div>{/if}
            </div>
          </div>
        {/each}
        {#if rows.length === 0}
          <div class="none">No actions extracted. Use <b>+ Add action</b> to add one.</div>
        {/if}
      </div>

      <div class="post-rail rise r2">
        <div class="pr-body scroll">
          <div class="meta-card">
            <div class="mc-t">{meta?.name ?? "Session"}</div>
            <div class="mc-row"><span>DATE</span><b>{fmtDate(meta?.date ?? "")}</b></div>
            <div class="mc-row"><span>DURATION</span><b>{fmtDuration(meta?.duration_ms ?? 0)}</b></div>
            <div class="mc-row"><span>PARTICIPANTS</span><b>{meta?.participants?.length ?? 0}</b></div>
            <div class="mc-row"><span>API COST</span><b style="color:var(--gold)">{fmtCost(meta?.total_api_cost ?? 0)}</b></div>
          </div>

          {#if keyTopics.length}
            <div class="topics">
              {#each keyTopics as t}<span class="topic">{t}</span>{/each}
            </div>
          {/if}

          <div class="sec-h"><div class="eyebrow">Decisions</div></div>
          {#if decisions.length === 0}
            <div class="none small">No decisions recorded.</div>
          {:else}
            {#each decisions as d}<div class="dec">{d}</div>{/each}
          {/if}
        </div>
        <div class="pr-foot">
          <button class="btn btn-gold" style="justify-content:center" disabled={saving} onclick={saveAndClose}>
            {saving ? "Saving…" : "Save & Close"}
          </button>
          <button class="btn btn-ghost" style="justify-content:center" onclick={() => (showTranscript = true)}>Back to Transcript</button>
        </div>
      </div>
    </div>
  {/if}

  {#if showTranscript}
    <div class="tr-overlay">
      <div class="tr-head">
        <div class="eyebrow">Transcript — {meta?.name ?? "Session"}</div>
        <button class="mini" onclick={() => (showTranscript = false)}>Close</button>
      </div>
      <div class="tr-scroll scroll">
        {#each transcriptEntries as line (line.id)}
          <div class="tr-line">
            <div class="tr-ts">{fmtTs(line.t_ms)}</div>
            <div>
              <div class="tr-who {line.stream === 'you' ? 'who-you' : 'who-remote'}">{line.stream === "you" ? "You" : "Remote"}</div>
              <div class="tr-said">{line.text}</div>
            </div>
          </div>
        {/each}
        {#if transcriptEntries.length === 0}<div class="none">No transcript captured.</div>{/if}
      </div>
    </div>
  {/if}
</section>

<style>
  .center{flex:1;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:14px;text-align:center;padding:40px}
  .spinner{width:38px;height:38px;border-radius:50%;border:3px solid var(--line);border-top-color:var(--gold);animation:spin 0.9s linear infinite}
  @keyframes spin{to{transform:rotate(360deg)}}
  .c-title{font-family:var(--f-disp);font-weight:600;font-size:22px;letter-spacing:-.02em}
  .c-sub{font-family:var(--f-mono);font-size:12px;color:var(--ink-3)}
  .c-note{font-size:13px;color:var(--ink-4);max-width:420px;line-height:1.6}
  .c-err{font-family:var(--f-mono);font-size:12px;color:var(--rec);max-width:560px;line-height:1.6;background:var(--bg-2);border:1px solid var(--line-soft);border-radius:var(--r-m);padding:12px 14px;word-break:break-word}
  .err-actions{display:flex;gap:10px;flex-wrap:wrap;justify-content:center;margin-top:6px}

  .duo{flex:1;display:flex;overflow:hidden}
  .post-main{flex:1;overflow-y:auto;padding:34px 44px 50px}
  .post-main>.eyebrow{margin-bottom:10px;display:block}
  .post-main h2{font-family:var(--f-disp);font-weight:600;font-size:34px;letter-spacing:-.025em;line-height:1;margin-bottom:30px}
  .sec-h{display:flex;align-items:center;justify-content:space-between;margin-bottom:13px}
  .mini{font-family:var(--f-mono);font-size:11px;color:var(--ink-3);background:var(--bg-3);border:1px solid var(--line);border-radius:6px;padding:4px 9px;display:flex;align-items:center;gap:5px;cursor:pointer}
  .mini:hover{color:var(--ink-2)}
  .mini:disabled{opacity:.5;cursor:default}

  .summary-edit{width:100%;min-height:96px;resize:vertical;background:var(--bg-2);border:1px solid var(--line-soft);border-radius:var(--r-m);padding:15px 16px;color:var(--ink-2);font-family:var(--f-ui);font-size:14px;line-height:1.68;outline:none;transition:.16s}
  .summary-edit:focus{border-color:var(--gold-line);box-shadow:0 0 0 3px var(--gold-soft)}

  .act-edit{display:flex;gap:13px;align-items:flex-start;padding:15px 16px;border:1px solid var(--line-soft);background:var(--bg-2);border-radius:var(--r-m);margin-bottom:10px}
  .ck{width:19px;height:19px;border-radius:6px;border:1.5px solid var(--line);flex:none;margin-top:3px;cursor:pointer;display:flex;align-items:center;justify-content:center;transition:.15s;background:transparent;padding:0}
  .ck.on{background:var(--gold);border-color:var(--gold)}
  .ck.on svg{color:#27200C;opacity:1}
  .ck svg{opacity:0;width:12px;height:12px}
  .ae-body{flex:1;min-width:0}
  .ae-top{display:flex;align-items:center;justify-content:space-between;gap:10px;margin-bottom:8px}
  .ae-title-input{flex:1;background:transparent;border:none;border-bottom:1px solid transparent;font-size:14px;font-weight:600;color:var(--ink);font-family:var(--f-ui);outline:none;padding:2px 0}
  .ae-title-input:hover{border-bottom-color:var(--line)}
  .ae-title-input:focus{border-bottom-color:var(--gold-line)}
  .del{flex:none;width:22px;height:22px;border-radius:6px;border:1px solid var(--line);background:var(--bg-3);color:var(--ink-3);font-size:15px;line-height:1;cursor:pointer}
  .del:hover{color:var(--rec);border-color:var(--rec-soft)}
  .ae-controls{display:flex;align-items:center;gap:8px;flex-wrap:wrap;margin-bottom:4px}
  .ae-select,.ae-due{font-family:var(--f-mono);font-size:11px;color:var(--ink-2);background:var(--bg-3);border:1px solid var(--line);border-radius:6px;padding:4px 8px;outline:none}
  .ae-select:focus,.ae-due:focus{border-color:var(--gold-line)}
  .ae-due{width:96px}
  .tag{font-family:var(--f-mono);font-size:9.5px;letter-spacing:.1em;text-transform:uppercase;padding:3px 7px;border-radius:5px;border:1px solid var(--line-soft);color:var(--ink-3)}
  .tag-commitment{color:var(--commit,#6ea8fe);border-color:var(--commit,#6ea8fe)}
  .tag-follow_up{color:var(--suggest,#8ac479);border-color:var(--suggest,#8ac479)}
  .tag-suggestion{color:var(--unanswered,#b794f6);border-color:var(--unanswered,#b794f6)}
  .tag-manual{color:var(--gold);border-color:var(--gold-line)}
  .quote{font-family:var(--f-disp);font-style:italic;font-size:13px;color:var(--ink-3);line-height:1.5;padding-left:12px;border-left:2px solid var(--line);margin-top:8px}
  .none{font-size:13px;color:var(--ink-4);padding:8px 0;line-height:1.6}
  .none.small{padding:2px 0}
  .none :global(b){color:var(--ink-2)}

  .post-rail{width:340px;flex:none;border-left:1px solid var(--line-soft);background:var(--bg-2);display:flex;flex-direction:column}
  .post-rail .pr-body{flex:1;overflow-y:auto;padding:30px 26px}
  .post-rail .pr-foot{padding:16px 22px;border-top:1px solid var(--line-soft);display:flex;flex-direction:column;gap:9px}
  .meta-card{border:1px solid var(--line-soft);border-radius:var(--r-m);padding:6px 16px;margin-bottom:22px;background:var(--bg-1)}
  .meta-card .mc-t{font-family:var(--f-disp);font-weight:600;font-size:18px;margin:12px 0 6px;letter-spacing:-.01em}
  .meta-card .mc-row{display:flex;justify-content:space-between;align-items:center;font-family:var(--f-mono);font-size:11.5px;padding:9px 0;color:var(--ink-3);border-top:1px solid var(--line-soft)}
  .meta-card .mc-row b{color:var(--ink-2);font-weight:500}
  .topics{display:flex;flex-wrap:wrap;gap:6px;margin-bottom:24px}
  .topic{font-family:var(--f-mono);font-size:10.5px;color:var(--ink-3);background:var(--bg-3);border:1px solid var(--line);border-radius:6px;padding:4px 8px}
  .dec{display:flex;gap:11px;align-items:flex-start;padding:8px 0;font-size:13.5px;color:var(--ink-2);line-height:1.5}
  .dec::before{content:"";width:5px;height:5px;border-radius:50%;background:var(--gold);margin-top:8px;flex:none;box-shadow:0 0 8px var(--gold)}

  .tr-overlay{position:absolute;inset:0;background:var(--bg-1);display:flex;flex-direction:column;z-index:10}
  .tr-head{flex:none;display:flex;align-items:center;justify-content:space-between;padding:18px 24px;border-bottom:1px solid var(--line-soft)}
  .tr-scroll{flex:1;overflow-y:auto;padding:24px 30px}
  .tr-line{display:flex;gap:18px;margin-bottom:18px;max-width:880px}
  .tr-ts{font-family:var(--f-mono);font-size:10.5px;color:var(--ink-4);width:54px;flex:none;padding-top:3px}
  .tr-who{font-family:var(--f-mono);font-size:10.5px;letter-spacing:.12em;text-transform:uppercase;font-weight:600;margin-bottom:5px}
  .tr-said{font-size:15px;line-height:1.62;color:var(--ink)}
  :global(.who-you){color:var(--gold)} :global(.who-remote){color:var(--commit)}
</style>
