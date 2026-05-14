<script lang="ts">
  import { fade } from "svelte/transition";
  import { cubicInOut } from "svelte/easing";
  import Waveform from "./Waveform.svelte";
  import SineWave from "./SineWave.svelte";

  type State = "idle" | "recording" | "transcribing" | "polished";

  let { state, spectrum, polishedText }: {
    state: State;
    spectrum: number[];
    polishedText: string;
  } = $props();

  // Custom transition: a small circle pops in, then expands horizontally into
  // the capsule. Svelte runs the same css(t) for both in and out — for `out:`
  // it feeds t from 1 → 0, giving us a perfectly mirrored collapse.
  function morphPill(_node: HTMLElement, { duration = 460 }: { duration?: number } = {}) {
    return {
      duration,
      easing: cubicInOut,
      css: (t: number) => {
        // Phase 1 (0–35%): bubble pops in (scale + opacity), width stays at 44 (circle)
        // Phase 2 (35–100%): bubble expands horizontally to full capsule width
        const popT = Math.min(t / 0.35, 1);
        const expandT = Math.max((t - 0.35) / 0.65, 0);
        const scl = 0.25 + popT * 0.75;
        const op = popT;
        const minWidth = 44 + expandT * 116; // 44 (circle) → 160 (capsule)
        return `
          transform: scale(${scl});
          transform-origin: center;
          opacity: ${op};
          min-width: ${minWidth}px;
          overflow: hidden;
        `;
      },
    };
  }
</script>

{#if state !== "idle"}
  <div class="pill" class:wide={state === "polished"} transition:morphPill>
    {#key state}
      <div
        class="content"
        in:fade={{ duration: 220, delay: 220, easing: cubicInOut }}
        out:fade={{ duration: 140, easing: cubicInOut }}
      >
        {#if state === "recording"}
          <Waveform {spectrum} />
        {:else if state === "transcribing"}
          <SineWave width={140} height={20} />
        {:else if state === "polished"}
          <span class="polished-text">{polishedText}</span>
        {/if}
      </div>
    {/key}
  </div>
{/if}

<style>
  .pill {
    height: 44px;
    min-width: 160px;
    padding: 0 16px;
    border-radius: 999px;

    display: flex;
    align-items: center;
    justify-content: center;

    background: rgba(245, 245, 247, 0.92);
    color: #1c1c1e;
    box-shadow:
      0 1px 0 rgba(255, 255, 255, 0.6) inset,
      0 8px 24px rgba(0, 0, 0, 0.18),
      0 1px 3px rgba(0, 0, 0, 0.1);

    backdrop-filter: blur(20px) saturate(180%);
    -webkit-backdrop-filter: blur(20px) saturate(180%);

    /* Container width morphs naturally to fit fading content */
    transition: min-width 0.35s cubic-bezier(0.34, 1.56, 0.64, 1);
  }

  .pill.wide {
    min-width: 220px;
  }

  @media (prefers-color-scheme: dark) {
    .pill {
      background: rgba(28, 28, 30, 0.78);
      color: #f5f5f7;
      box-shadow:
        0 1px 0 rgba(255, 255, 255, 0.06) inset,
        0 8px 24px rgba(0, 0, 0, 0.55),
        0 1px 3px rgba(0, 0, 0, 0.4);
    }
  }

  .content {
    display: flex;
    align-items: center;
    justify-content: center;
    /* keyed children stack so they cross-fade in place during state changes */
    grid-area: 1 / 1;
  }

  /* Stack the cross-fading content blocks via grid so they overlap cleanly */
  .pill {
    display: grid;
    place-items: center;
    grid-template-columns: minmax(0, 1fr);
  }

  .polished-text {
    font-size: 13px;
    font-weight: 500;
    letter-spacing: -0.01em;
    white-space: nowrap;
  }
</style>
