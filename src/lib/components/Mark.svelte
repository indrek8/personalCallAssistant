<script lang="ts">
  // The gold waveform brand mark. Optional `size` (px) for larger placements.
  let { size = 23 }: { size?: number } = $props();
  const barH = $derived(Math.round((size / 23) * 11));
  const radius = $derived(Math.round(size / 3));
</script>

<span class="mark" style={`width:${size}px;height:${size}px;border-radius:${radius}px`}>
  <span style={`height:${barH}px`}><i></i><i></i><i></i><i></i><i></i></span>
</span>
