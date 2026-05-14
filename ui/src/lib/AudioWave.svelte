<script lang="ts">
  // Stack of SVG sine-like lines, all pinned at both endpoints and bowing
  // in the middle. Multiple lines breathe in slightly different phases /
  // frequencies so the cluster feels alive even on a steady amplitude.
  // Reused across all pill states — props animate reactively for a
  // seamless recording → transcribing morph.
  let {
    amplitude = 0,
    width = 94,
    height = 22,
    frequency = 4,
    lines = 1,
    reveal = 1,
  }: {
    amplitude?: number;
    width?: number;
    height?: number;
    frequency?: number;
    lines?: number;
    /** 0..1 — line is "drawn" from the centre outward via stroke-dashoffset. */
    reveal?: number;
  } = $props();

  // Higher resolution → curves stop reading as polylines on small pills
  // (geometricPrecision rendering helps too).
  const POINTS = 64;

  // RAF-driven phase keeps the wave moving even at low amplitude. Smoothed
  // amplitude follows the prop with snappier attack so the line *reacts*
  // to speech rather than mushing through it.
  let t = $state(0);
  let smoothed = $state(0);

  $effect(() => {
    let raf = 0;
    let last = performance.now();
    const tick = (now: number) => {
      const dt = (now - last) / 1000;
      last = now;
      t += dt;
      const target = Math.max(0, Math.min(1, amplitude));
      const k = target > smoothed ? 30 : 7;
      smoothed += (target - smoothed) * Math.min(1, k * dt);
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  });

  // Always render the same number of <path> elements so that switching
  // `lines` from 3 → 1 doesn't unmount paths 1 & 2 (which would visually
  // "snap"). Visibility is controlled below via opacity.
  const MAX_LINES = 3;

  const paths = $derived.by(() => {
    void t;
    const W = width;
    const H = height;
    const cy = H / 2;
    const ampScale = (H / 2 - 1.5) * (0.04 + 0.96 * smoothed);
    const dynFreq = frequency + smoothed * 1.8;
    const out: string[] = [];
    for (let line = 0; line < MAX_LINES; line++) {
      const phaseOffset = (line / MAX_LINES) * Math.PI * 1.7;
      const freq = dynFreq + (line - (MAX_LINES - 1) / 2) * 0.6;
      const ampMul = 0.7 + 0.3 * Math.cos(line * 1.7);
      const speedMul = 1 + line * 0.18;

      const pts: Array<[number, number]> = [];
      for (let i = 0; i <= POINTS; i++) {
        const u = i / POINTS;
        const x = u * W;
        const window = Math.sin(u * Math.PI);
        const phase = u * Math.PI * freq + t * 2.6 * speedMul + phaseOffset;
        const y = cy + Math.sin(phase) * window * ampScale * ampMul;
        pts.push([x, y]);
      }
      let d = `M ${pts[0][0].toFixed(2)} ${pts[0][1].toFixed(2)}`;
      for (let i = 1; i < pts.length; i++) {
        const [x0, y0] = pts[i - 1];
        const [x1, y1] = pts[i];
        const cx = (x0 + x1) / 2;
        d += ` Q ${cx.toFixed(2)} ${y0.toFixed(2)} ${x1.toFixed(2)} ${y1.toFixed(2)}`;
      }
      out.push(d);
    }
    return out;
  });

  // SVG `pathLength` normalises the path's effective length to 100,
  // letting us reveal it with a constant 0..100 dashoffset regardless of
  // the actual bezier length.
  const dashoffset = $derived(100 * (1 - Math.max(0, Math.min(1, reveal))));
</script>

<svg
  {width}
  {height}
  viewBox="0 0 {width} {height}"
  shape-rendering="geometricPrecision"
  aria-hidden="true"
>
  {#each paths as d, i (i)}
    <path
      {d}
      pathLength="100"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
      stroke-dasharray="100"
      stroke-dashoffset={dashoffset}
      opacity={i < lines ? (i === 0 ? 1 : 0.5) : 0}
    />
  {/each}
</svg>

<style>
  svg { display: block; overflow: visible; }
  /* Smooth secondary-line fade-out so the 3 → 1 collapse looks like
     the cluster melting into a single stroke, not snapping. */
  path { transition: opacity 0.45s cubic-bezier(0.4, 0, 0.2, 1); }
</style>
