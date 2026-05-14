<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import Models from "$lib/settings/Models.svelte";
  import Backend from "$lib/settings/Backend.svelte";
  import Postprocess from "$lib/settings/Postprocess.svelte";
  import Stats from "$lib/settings/Stats.svelte";

  const win = getCurrentWindow();

  // X just hides the window — actually quitting flov is done from the tray.
  function close() {
    win.hide();
  }
</script>

<div class="app-container">
  <!-- Top strip is the drag region. Frameless windows can only be moved by
       elements explicitly tagged data-tauri-drag-region; cards/inputs would
       otherwise eat the click. The strip also reserves space for the X
       and the brand mark on the left. -->
  <div class="drag-strip" data-tauri-drag-region>
    <span class="brand" data-tauri-drag-region>
      <span class="brand-name">Flov</span>
      <span class="brand-by">by puzix</span>
    </span>
    <button class="close-x" onclick={close} aria-label="Close">
      <svg viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round">
        <path d="M4 4 L12 12 M12 4 L4 12"/>
      </svg>
    </button>
  </div>

  <main class="grid">
    <div class="left-col">
      <section class="zone engine-zone">
        <Models />
        <Backend />
      </section>
      <section class="zone activity-zone">
        <Stats />
      </section>
    </div>
    <section class="zone pp-zone">
      <Postprocess />
    </section>
  </main>
</div>

<style>
  /* ===== TOKENS ===== */
  :global(:root) {
    --bg: #f3f3f6;
    --bg-elevated: #ffffff;
    --surface: #f8f9fa;
    --fg: #18181b;
    --muted: #71717a;
    --border: #e4e4e7;
    --accent: #7c3aed;
    --accent-fg: #ffffff;
    --accent-soft: rgba(124, 58, 237, 0.10);
    --ok: #10b981;
    --warn: #f59e0b;
    --danger: #ef4444;
    --hover: #ececf0;
    --pressed: #e4e4e7;
    --shadow-card: 0 1px 2px rgba(0, 0, 0, 0.04), 0 4px 12px -4px rgba(0, 0, 0, 0.05);

    --text-xs: 12px;
    --text-sm: 13px;
    --text-base: 15px;
    --text-lg: 18px;
    --text-xl: 22px;
    --text-display: 30px;

    --space-4: 4px;
    --space-8: 8px;
    --space-12: 12px;
    --space-16: 16px;
    --space-20: 20px;
    --space-24: 24px;

    --radius-sm: 8px;
    --radius-md: 12px;
    --radius-lg: 16px;
    --radius-xl: 20px;
    --radius-pill: 999px;

    --ease-out: cubic-bezier(0.2, 0.8, 0.2, 1);
    --ease-spring: cubic-bezier(0.34, 1.56, 0.64, 1);
  }

  @media (prefers-color-scheme: dark) {
    :global(:root) {
      --bg: #0e0e11;
      --bg-elevated: #18181b;
      --surface: #232327;
      --fg: #fafafa;
      --muted: #a1a1aa;
      --border: #2e2e33;
      --accent: #d9ff42;
      --accent-fg: #0a0a0c;
      --accent-soft: rgba(217, 255, 66, 0.12);
      --ok: #d9ff42;
      --warn: #fbbf24;
      --danger: #f87171;
      --hover: #2d2d33;
      --pressed: #3f3f46;
      --shadow-card: 0 1px 2px rgba(0, 0, 0, 0.4), 0 4px 16px -4px rgba(0, 0, 0, 0.4);
    }
  }

  :global(html, body) {
    margin: 0;
    padding: 0;
    height: 100%;
    background: transparent !important;
    color: var(--fg);
    font-family:
      -apple-system, BlinkMacSystemFont, "Segoe UI", Inter, system-ui, sans-serif;
    -webkit-font-smoothing: antialiased;
    overflow: hidden;
    font-size: var(--text-sm);
  }

  :global(::-webkit-scrollbar) { width: 6px; height: 6px; }
  :global(::-webkit-scrollbar-track) { background: transparent; }
  :global(::-webkit-scrollbar-thumb) { background: var(--border); border-radius: 99px; }
  :global(::-webkit-scrollbar-thumb:hover) { background: var(--muted); }

  /* ===== APP CONTAINER (no titlebar — whole surface is draggable) ===== */
  .app-container {
    position: fixed;
    inset: 0;
    background: var(--bg);
    border-radius: var(--radius-lg);
    overflow: hidden;
    border: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    animation: appear 0.3s var(--ease-out);
  }
  @keyframes appear {
    from { opacity: 0; transform: scale(0.985); }
    to   { opacity: 1; transform: scale(1); }
  }

  /* ===== DRAG STRIP at the top — also hosts the close X ===== */
  .drag-strip {
    flex: 0 0 36px;
    position: relative;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    padding: 0 14px;
  }
  .brand {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    display: inline-flex;
    align-items: baseline;
    gap: 6px;
    pointer-events: none;
  }
  .brand-name {
    font: 700 13px / 1 inherit;
    color: var(--fg);
    letter-spacing: -0.2px;
  }
  .brand-by {
    font: 500 11px / 1 inherit;
    color: var(--muted);
  }

  .close-x {
    width: 28px;
    height: 28px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--muted);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    padding: 0;
    transition: background-color 0.15s var(--ease-out), color 0.15s var(--ease-out);
  }
  .close-x:hover { background: var(--danger); color: #ffffff; }

  /* ===== GRID =====
     Padding + gap are unified on `--space-20` so every visible gutter in
     the window matches: between Models/Backend, between engine/activity,
     between left column / postprocess, and around the outside. The zones
     themselves have no padding — spacing comes only from the grid. */
  .grid {
    flex: 1 1 auto;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    grid-template-rows: minmax(0, 1fr);
    gap: var(--space-20);
    padding: var(--space-20);
    box-sizing: border-box;
  }
  .left-col {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    gap: var(--space-20);
    min-width: 0;
    min-height: 0;
  }
  .zone {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }
  .engine-zone {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: var(--space-20);
  }
  /* Activity zone uses an explicit grid: counters / nav / weekday / month
     stack into 4 rows where only the last (month-grid) is `1fr`. */
  .activity-zone {
    display: grid;
    grid-template-rows: auto auto auto minmax(0, 1fr);
  }
  .pp-zone {}
</style>
