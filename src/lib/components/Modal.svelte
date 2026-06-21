<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    /** Called on backdrop click or Esc. */
    onClose?: () => void;
    /** Max width of the card (px). */
    maxWidth?: number;
    children: Snippet;
  }
  let { onClose, maxWidth = 520, children }: Props = $props();

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") onClose?.();
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="overlay">
  <!-- Backdrop is a real button so closing is keyboard-accessible. -->
  <button class="backdrop" aria-label="Close" onclick={() => onClose?.()}></button>
  <div class="modal" role="dialog" aria-modal="true" style="max-width:min(92vw,{maxWidth}px)">
    {@render children()}
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 1000;
    display: flex;
    align-items: center;
    justify-content: center;
    animation: fade 0.15s var(--ease);
  }
  .backdrop {
    position: absolute;
    inset: 0;
    border: 0;
    cursor: default;
    background: rgba(8, 7, 6, 0.66);
    backdrop-filter: blur(2px);
  }
  .modal {
    position: relative;
    width: 100%;
    max-height: 86vh;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    background: var(--bg-1);
    border: 1px solid var(--line);
    border-radius: var(--r-l);
    box-shadow: 0 24px 70px -20px rgba(0, 0, 0, 0.7);
    animation: pop 0.18s var(--ease);
  }
  @keyframes fade {
    from { opacity: 0; }
    to { opacity: 1; }
  }
  @keyframes pop {
    from { opacity: 0; transform: translateY(8px) scale(0.99); }
    to { opacity: 1; transform: none; }
  }
</style>
