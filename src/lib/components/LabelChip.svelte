<script lang="ts">
  import type { LabelRef } from "$lib/types";

  interface Props {
    label: LabelRef;
    removable?: boolean;
    onRemove?: () => void;
  }
  let { label, removable = false, onRemove }: Props = $props();

  // Fall back to a muted ink when a label carries no color.
  const color = $derived(label.color ?? "#827969");
</script>

<span class="chip2" style="--c:{color}">
  <span class="dot"></span>
  {label.name}
  {#if removable}
    <button class="rm" aria-label={`Remove ${label.name}`} onclick={onRemove}>✕</button>
  {/if}
</span>

<style>
  .chip2 {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 10px;
    font-weight: 600;
    padding: 1.5px 7px;
    border-radius: 5px;
    letter-spacing: 0.01em;
    white-space: nowrap;
    color: var(--c);
    background: color-mix(in srgb, var(--c) 13%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--c) 26%, transparent);
  }
  .dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--c);
    flex: none;
  }
  .rm {
    background: none;
    border: 0;
    color: inherit;
    cursor: pointer;
    font-size: 9px;
    opacity: 0.7;
    padding: 0 0 0 1px;
    line-height: 1;
  }
  .rm:hover { opacity: 1; }
</style>
