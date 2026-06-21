<script lang="ts">
  import Modal from "./Modal.svelte";
  import { labels, sessions, refreshLabels, pushToast } from "$lib/stores";
  import { createLabel, updateLabel, deleteLabel } from "$lib/ipc";

  interface Props {
    onClose: () => void;
  }
  let { onClose }: Props = $props();

  let newName = $state("");
  let newColor = $state("#e7b24c");

  /** How many sessions currently carry this label (by id). */
  function usage(id: string): number {
    return $sessions.filter((s) => (s.labels ?? []).some((l) => l.id === id)).length;
  }

  async function add() {
    const name = newName.trim();
    if (!name) return;
    try {
      await createLabel(name, newColor);
      newName = "";
      await refreshLabels();
    } catch (e) {
      pushToast(`Could not create label: ${String(e)}`, { kind: "error" });
    }
  }

  async function rename(id: string, name: string) {
    try {
      await updateLabel(id, name);
      await refreshLabels();
    } catch (e) {
      pushToast(`Could not rename label: ${String(e)}`, { kind: "error" });
    }
  }

  async function recolor(id: string, color: string) {
    try {
      await updateLabel(id, undefined, color);
      await refreshLabels();
    } catch (e) {
      pushToast(`Could not recolor label: ${String(e)}`, { kind: "error" });
    }
  }

  async function del(id: string) {
    try {
      await deleteLabel(id);
      await refreshLabels();
    } catch (e) {
      pushToast(`Could not delete label: ${String(e)}`, { kind: "error" });
    }
  }
</script>

<Modal {onClose} maxWidth={500}>
  <div class="lm">
    <div class="lm-head">
      <div class="lm-title">Labels</div>
      <button class="x" aria-label="Close" onclick={onClose}>✕</button>
    </div>

    <div class="create">
      <input type="color" class="swatch" bind:value={newColor} aria-label="New label color" />
      <input
        class="inp name-in"
        placeholder="New label name…"
        bind:value={newName}
        onkeydown={(e) => e.key === "Enter" && add()}
      />
      <button class="btn btn-gold" onclick={add} disabled={!newName.trim()}>Add</button>
    </div>

    <div class="list scroll">
      {#if $labels.length === 0}
        <div class="empty">No labels yet. Create one above to start tagging sessions.</div>
      {:else}
        {#each $labels as label (label.id)}
          <div class="lrow">
            <input
              type="color"
              class="swatch"
              value={label.color ?? "#827969"}
              aria-label={`Color for ${label.name}`}
              onchange={(e) => recolor(label.id, e.currentTarget.value)}
            />
            <input
              class="inp name-in"
              value={label.name}
              aria-label="Label name"
              onchange={(e) => rename(label.id, e.currentTarget.value)}
            />
            <span class="count">{usage(label.id)} {usage(label.id) === 1 ? "session" : "sessions"}</span>
            <button class="del" aria-label={`Delete ${label.name}`} onclick={() => del(label.id)}>✕</button>
          </div>
        {/each}
      {/if}
    </div>
  </div>
</Modal>

<style>
  .lm { display: flex; flex-direction: column; min-height: 0; }
  .lm-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 18px 20px 14px;
    border-bottom: 1px solid var(--line-soft);
  }
  .lm-title {
    font-family: var(--f-disp);
    font-weight: 600;
    font-size: 19px;
    letter-spacing: -0.01em;
  }
  .x { background: none; border: 0; color: var(--ink-3); cursor: pointer; font-size: 14px; }
  .x:hover { color: var(--ink); }
  .create {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 16px 20px;
    border-bottom: 1px solid var(--line-soft);
  }
  .list { padding: 10px 20px 18px; max-height: 50vh; }
  .lrow {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 8px 0;
  }
  .swatch {
    width: 26px;
    height: 26px;
    flex: none;
    padding: 0;
    border: 1px solid var(--line);
    border-radius: 7px;
    background: var(--bg-2);
    cursor: pointer;
  }
  .swatch::-webkit-color-swatch-wrapper { padding: 2px; }
  .swatch::-webkit-color-swatch { border: 0; border-radius: 5px; }
  .name-in { flex: 1; padding: 8px 11px; font-size: 13px; }
  .count {
    flex: none;
    font-family: var(--f-mono);
    font-size: 10px;
    color: var(--ink-3);
    white-space: nowrap;
  }
  .del {
    flex: none;
    background: none;
    border: 0;
    color: var(--ink-3);
    cursor: pointer;
    font-size: 12px;
    padding: 4px 6px;
    border-radius: 5px;
  }
  .del:hover { color: var(--late); background: rgba(255, 107, 92, 0.1); }
  .empty {
    padding: 26px 8px;
    text-align: center;
    font-size: 13px;
    line-height: 1.6;
    color: var(--ink-3);
  }
</style>
