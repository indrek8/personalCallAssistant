<script lang="ts">
  import Modal from "./Modal.svelte";

  interface Props {
    title: string;
    message?: string;
    confirmLabel?: string;
    cancelLabel?: string;
    /** Style the confirm button as destructive (delete/discard). */
    destructive?: boolean;
    onConfirm: () => void;
    onCancel: () => void;
  }
  let {
    title,
    message,
    confirmLabel = "Confirm",
    cancelLabel = "Cancel",
    destructive = false,
    onConfirm,
    onCancel,
  }: Props = $props();
</script>

<Modal onClose={onCancel} maxWidth={440}>
  <div class="cd">
    <div class="cd-title">{title}</div>
    {#if message}<p class="cd-msg">{message}</p>{/if}
    <div class="cd-actions">
      <button class="btn btn-ghost" onclick={onCancel}>{cancelLabel}</button>
      <button class="btn" class:btn-danger={destructive} class:btn-gold={!destructive} onclick={onConfirm}>
        {confirmLabel}
      </button>
    </div>
  </div>
</Modal>

<style>
  .cd { padding: 24px 24px 22px; }
  .cd-title {
    font-family: var(--f-disp);
    font-weight: 600;
    font-size: 20px;
    letter-spacing: -0.01em;
    margin-bottom: 10px;
  }
  .cd-msg {
    font-size: 13.5px;
    line-height: 1.6;
    color: var(--ink-2);
    margin-bottom: 22px;
  }
  .cd-actions { display: flex; justify-content: flex-end; gap: 10px; }
  .btn-danger {
    background: rgba(255, 107, 92, 0.14);
    border: 1px solid rgba(255, 107, 92, 0.4);
    color: #ff8278;
  }
  .btn-danger:hover { background: rgba(255, 107, 92, 0.22); }
</style>
