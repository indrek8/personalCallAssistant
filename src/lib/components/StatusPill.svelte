<script lang="ts">
  import type { ActionStatus } from "$lib/types";

  interface Props {
    status: ActionStatus;
    onChange: (s: ActionStatus) => void;
    disabled?: boolean;
  }
  let { status, onChange, disabled = false }: Props = $props();

  const OPTIONS: { value: ActionStatus; label: string }[] = [
    { value: "pending", label: "Pending" },
    { value: "in_progress", label: "In progress" },
    { value: "done", label: "Done" },
    { value: "wont_do", label: "Won't do" },
    { value: "postponed", label: "Postponed" },
  ];
</script>

<!-- A styled native <select>: the most robust inline status editor (keyboard +
     screen-reader friendly). Colored by current status. -->
<select
  class="pill {status}"
  value={status}
  {disabled}
  aria-label="Action status"
  onchange={(e) => onChange(e.currentTarget.value as ActionStatus)}
>
  {#each OPTIONS as o}
    <option value={o.value}>{o.label}</option>
  {/each}
</select>

<style>
  .pill {
    font-family: var(--f-mono);
    font-size: 9.5px;
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    padding: 3px 8px;
    border-radius: 5px;
    border: 1px solid transparent;
    cursor: pointer;
    appearance: none;
    outline: none;
    transition: 0.15s;
  }
  .pill:focus { box-shadow: 0 0 0 2px var(--gold-soft); }
  .pill:disabled { cursor: default; opacity: 0.6; }
  .pill.pending { color: var(--pend); background: rgba(231, 178, 76, 0.1); }
  .pill.in_progress { color: var(--commit); background: rgba(84, 197, 222, 0.1); }
  .pill.done { color: var(--done); background: rgba(138, 196, 121, 0.1); }
  .pill.wont_do { color: var(--ink-3); background: rgba(130, 121, 105, 0.12); }
  .pill.postponed { color: var(--ask); background: rgba(178, 149, 232, 0.1); }
  .pill option { background: var(--bg-3); color: var(--ink); }
</style>
