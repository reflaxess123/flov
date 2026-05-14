<script lang="ts">
  import AudioWave from "./AudioWave.svelte";

  type State = "idle" | "recording" | "transcribing" | "polished";
  type Props = { status: State; spectrum: number[]; polishedText?: string };

  let { status, spectrum }: Props = $props();

  const audioAmp = $derived.by(() => {
    if (!spectrum || spectrum.length === 0) return 0;
    let s = 0;
    for (const v of spectrum) s += v;
    return Math.min(1, (s / spectrum.length) * 3.5);
  });

  // Sequenced amplitude during transcribing: dim to flat first, let the
  // colour cross-fade to accent, then ramp the new bold green line back
  // up. AudioWave's smoother does the actual fall/rise — we just feed
  // the right targets.
  let processingAmp = $state(0);
  $effect(() => {
    if (status === "transcribing") {
      processingAmp = 0;
      const id = setTimeout(() => { processingAmp = 0.6; }, 380);
      return () => clearTimeout(id);
    }
  });

  const targetAmp = $derived(
    status === "recording" ? audioAmp
    : status === "transcribing" ? processingAmp
    : 0,
  );

  // Recording shows several interweaving lines; processing collapses to
  // one bold accent line.
  const lineCount = $derived(status === "recording" ? 3 : 1);

  // Spring-ish overshoot for the bubble pop — the "вжух".
  function backOut(t: number): number {
    const c = 1.55;
    const u = t - 1;
    return 1 + (c + 1) * u * u * u + c * u * u;
  }
  function easeOutCubic(t: number): number {
    const u = 1 - t;
    return 1 - u * u * u;
  }

  // Pop and horizontal expansion overlap so the bubble has almost finished
  // its scale-in when the pill starts stretching — reads as one gesture
  // rather than two beats. Floored at 0.1 so a stale frame can't render
  // the pill at scale(0).
  function morphPill(_node: HTMLElement, { duration = 480 }: { duration?: number } = {}) {
    return {
      duration,
      css: (t: number) => {
        const popT = Math.min(1, t / 0.55);
        const expandT = Math.max(0, Math.min(1, (t - 0.4) / 0.6));
        const popEased = backOut(popT);
        const exEased = easeOutCubic(expandT);
        const minWidth = 36 + exEased * 74;
        const safeScale = Math.max(0.1, popEased);
        return `
          transform: scale(${safeScale});
          transform-origin: center;
          min-width: ${minWidth}px;
          overflow: hidden;
        `;
      },
    };
  }

  // Reveal of the inner line. RAF-driven so it stays in sync with the
  // pill's scale animation. The line is "drawn" via stroke-dashoffset
  // inside AudioWave, which never collapses the layout — even if the
  // RAF skips a frame the path is still in the DOM at full size.
  let revealAmount = $state(1);
  let revealRaf = 0;
  $effect(() => {
    cancelAnimationFrame(revealRaf);
    if (status === "idle") {
      revealAmount = 1;
      return;
    }
    revealAmount = 0;
    const start = performance.now();
    const total = 480;
    const animate = (now: number) => {
      const t = Math.min(1, (now - start) / total);
      const ex = Math.max(0, Math.min(1, (t - 0.4) / 0.6));
      revealAmount = easeOutCubic(ex);
      if (t < 1) revealRaf = requestAnimationFrame(animate);
    };
    revealRaf = requestAnimationFrame(animate);
    return () => cancelAnimationFrame(revealRaf);
  });
</script>

{#if status !== "idle"}
  <div class="pill" class:processing={status === "transcribing"} transition:morphPill>
    <AudioWave amplitude={targetAmp} lines={lineCount} reveal={revealAmount} />
  </div>
{/if}

<style>
  .pill {
    --pill-bg: #f5f5f7;
    --pill-fg: #1c1c1e;
    --pill-accent: #18181b;

    height: 36px;
    min-width: 110px;
    padding: 0 8px;
    border-radius: 999px;

    display: flex;
    align-items: center;
    justify-content: center;

    background: var(--pill-bg);
    color: var(--pill-fg);
    box-shadow:
      0 6px 18px rgba(0, 0, 0, 0.18),
      0 1px 3px rgba(0, 0, 0, 0.1);

    /* Smooth colour shift between recording (text colour) and processing
       (accent). currentColor in AudioWave's stroke picks this up. */
    transition:
      color 0.55s cubic-bezier(0.4, 0, 0.2, 1),
      min-width 0.32s cubic-bezier(0.34, 1.56, 0.64, 1);
  }

  @media (prefers-color-scheme: dark) {
    .pill {
      --pill-bg: #1c1c1e;
      --pill-fg: #f5f5f7;
      --pill-accent: #d9ff42;
      box-shadow:
        0 6px 18px rgba(0, 0, 0, 0.55),
        0 1px 3px rgba(0, 0, 0, 0.4);
    }
  }
  .pill.processing { color: var(--pill-accent); }
</style>
