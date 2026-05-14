<script lang="ts">
  let { spectrum, dimmed = false }: { spectrum: number[]; dimmed?: boolean } =
    $props();
</script>

<div class="waveform" class:dimmed>
  {#each spectrum as v, i (i)}
    <div class="bar" style="--h: {Math.max(0.08, v)};"></div>
  {/each}
</div>

<style>
  .waveform {
    display: flex;
    align-items: center;
    gap: 2.5px;
    height: 22px;
    width: auto;
    transition: filter 0.4s ease, opacity 0.4s ease;
  }

  .waveform.dimmed {
    filter: blur(1.5px);
    opacity: 0.4;
  }

  .bar {
    width: 2.5px;
    height: calc(100% * var(--h));
    border-radius: 1.5px;
    background: currentColor;
    /* heights move; positions don't */
    transition: height 0.05s linear;
  }
</style>
