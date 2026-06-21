<script lang="ts">
  import { toasts, dismissToast } from "$lib/stores";
</script>

<div class="toasts">
  {#each $toasts as t (t.id)}
    <div class="toast {t.kind}" role="status">
      {#if t.code}<span class="code">{t.code}</span>{/if}
      <span class="msg">{t.message}</span>
      {#if t.action}
        {@const action = t.action}
        <button class="act" onclick={() => { action.run(); dismissToast(t.id); }}>{action.label}</button>
      {/if}
      <button class="x" aria-label="Dismiss" onclick={() => dismissToast(t.id)}>✕</button>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    right: 16px;
    bottom: 16px;
    z-index: 1100;
    display: flex;
    flex-direction: column;
    gap: 9px;
    max-width: min(92vw, 420px);
    pointer-events: none;
  }
  .toast {
    pointer-events: auto;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 11px 13px;
    border-radius: var(--r-m);
    background: var(--bg-3);
    border: 1px solid var(--line);
    box-shadow: 0 14px 40px -16px rgba(0, 0, 0, 0.7);
    font-size: 12.5px;
    color: var(--ink-2);
    animation: slide 0.2s var(--ease);
  }
  .toast.error { border-color: rgba(255, 107, 92, 0.4); }
  .toast.success { border-color: rgba(138, 196, 121, 0.4); }
  .code {
    font-family: var(--f-mono);
    font-size: 9.5px;
    letter-spacing: 0.06em;
    color: var(--late);
    flex: none;
  }
  .toast.success .code { color: var(--done); }
  .msg { flex: 1; line-height: 1.45; }
  .act {
    flex: none;
    font-family: var(--f-ui);
    font-weight: 600;
    font-size: 12px;
    color: var(--gold);
    background: var(--gold-soft);
    border: 1px solid var(--gold-line);
    border-radius: var(--r-s);
    padding: 5px 10px;
    cursor: pointer;
  }
  .act:hover { background: rgba(231, 178, 76, 0.2); }
  .x {
    flex: none;
    background: none;
    border: 0;
    color: var(--ink-3);
    cursor: pointer;
    font-size: 12px;
  }
  .x:hover { color: var(--ink); }
  @keyframes slide {
    from { opacity: 0; transform: translateY(8px); }
    to { opacity: 1; transform: none; }
  }
</style>
