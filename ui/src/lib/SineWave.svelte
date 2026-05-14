<script lang="ts">
  import { onMount } from "svelte";

  let { width = 160, height = 22 }: { width?: number; height?: number } =
    $props();

  let canvas: HTMLCanvasElement;

  onMount(() => {
    const dpr = window.devicePixelRatio || 1;
    canvas.width = width * dpr;
    canvas.height = height * dpr;
    const ctx = canvas.getContext("2d")!;
    ctx.scale(dpr, dpr);

    // Pull the live text color so curves match the theme automatically
    const stroke = getComputedStyle(canvas).color;

    let raf = 0;
    const start = performance.now();

    // Three intertwining sine waves with slightly different frequencies and
    // phases. Their amplitude is itself modulated by a slow sine envelope, so
    // the curves swell and shrink as they pass each other — that's what gives
    // the "twisting" feel.
    const curves = [
      { freq: 0.055, phaseSpd: 0.0042, ampBase: 0.42, ampSpd: 0.0009, lw: 1.6, alpha: 0.95 },
      { freq: 0.040, phaseSpd: -0.0035, ampBase: 0.32, ampSpd: 0.0013, lw: 1.3, alpha: 0.65 },
      { freq: 0.072, phaseSpd: 0.0058, ampBase: 0.22, ampSpd: 0.0017, lw: 1.0, alpha: 0.4 },
    ];

    function frame(now: number) {
      const t = now - start;
      ctx.clearRect(0, 0, width, height);
      const cy = height / 2;

      for (const c of curves) {
        const env = (Math.sin(t * c.ampSpd) * 0.4 + 0.6) * c.ampBase * height;
        ctx.beginPath();
        for (let x = 0; x <= width; x++) {
          const fade = Math.sin((x / width) * Math.PI); // taper at edges
          const y = cy + Math.sin(x * c.freq + t * c.phaseSpd) * env * fade;
          if (x === 0) ctx.moveTo(x, y);
          else ctx.lineTo(x, y);
        }
        ctx.lineWidth = c.lw;
        ctx.globalAlpha = c.alpha;
        ctx.strokeStyle = stroke;
        ctx.stroke();
      }
      ctx.globalAlpha = 1;
      raf = requestAnimationFrame(frame);
    }
    raf = requestAnimationFrame(frame);
    return () => cancelAnimationFrame(raf);
  });
</script>

<canvas
  bind:this={canvas}
  style="width: {width}px; height: {height}px; display: block;"
></canvas>

<style>
  canvas {
    /* match parent text color so curves auto-theme via getComputedStyle */
    color: inherit;
  }
</style>
