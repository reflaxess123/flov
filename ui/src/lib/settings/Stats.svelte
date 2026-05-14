<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  type DayStats = { recordings: number; chars: number; seconds: number };
  type StatsFile = {
    total_recordings: number;
    total_chars: number;
    total_seconds: number;
    by_day: Record<string, DayStats>;
  };

  let stats = $state<StatsFile>({
    total_recordings: 0,
    total_chars: 0,
    total_seconds: 0,
    by_day: {},
  });

  // Month being displayed. Start at current month; user can step backwards/
  // forwards with the chevrons. We never go past the current month — there's
  // no future activity to show.
  const today = new Date();
  today.setUTCHours(0, 0, 0, 0);
  let viewYear = $state(today.getUTCFullYear());
  let viewMonth = $state(today.getUTCMonth()); // 0-11

  const MONTH_NAMES = [
    "January", "February", "March", "April", "May", "June",
    "July", "August", "September", "October", "November", "December",
  ];
  const isCurrentMonth = $derived(
    viewYear === today.getUTCFullYear() && viewMonth === today.getUTCMonth(),
  );

  function fmtDate(d: Date): string {
    const y = d.getUTCFullYear();
    const m = String(d.getUTCMonth() + 1).padStart(2, "0");
    const day = String(d.getUTCDate()).padStart(2, "0");
    return `${y}-${m}-${day}`;
  }
  function fmtSeconds(s: number): string {
    if (s < 60) return `${s.toFixed(0)}s`;
    const m = s / 60;
    if (m < 60) return `${m.toFixed(1)}m`;
    return `${(m / 60).toFixed(1)}h`;
  }
  function fmtCount(n: number): string {
    if (n < 1000) return String(n);
    if (n < 1_000_000) return `${(n / 1000).toFixed(1)}k`;
    return `${(n / 1_000_000).toFixed(1)}M`;
  }

  // Build a 6×7 grid for the current view-month. Empty cells before the 1st
  // and after the last day are rendered transparent so the grid stays
  // rectangular.
  type Cell = { empty: true } | {
    empty: false;
    day: number;
    date: string;
    stats: DayStats;
    future: boolean;
  };
  const cells = $derived.by((): Cell[] => {
    const first = new Date(Date.UTC(viewYear, viewMonth, 1));
    const last = new Date(Date.UTC(viewYear, viewMonth + 1, 0));
    // Mon=0..Sun=6
    const startOffset = (first.getUTCDay() + 6) % 7;
    const daysInMonth = last.getUTCDate();
    const out: Cell[] = [];
    for (let i = 0; i < startOffset; i++) out.push({ empty: true });
    for (let d = 1; d <= daysInMonth; d++) {
      const date = new Date(Date.UTC(viewYear, viewMonth, d));
      const ds = fmtDate(date);
      out.push({
        empty: false,
        day: d,
        date: ds,
        stats: stats.by_day[ds] ?? { recordings: 0, chars: 0, seconds: 0 },
        future: date > today,
      });
    }
    while (out.length % 7 !== 0) out.push({ empty: true });
    return out;
  });
  // 5 or 6 weeks depending on the month — never render an empty trailing
  // row. The grid template is bound below so cells always fill the
  // available height proportionally.
  const rowCount = $derived(cells.length / 7);

  function level(rec: number): number {
    if (rec === 0) return 0;
    if (rec < 3) return 1;
    if (rec < 8) return 2;
    if (rec < 20) return 3;
    return 4;
  }

  function prevMonth() {
    if (viewMonth === 0) {
      viewMonth = 11;
      viewYear -= 1;
    } else {
      viewMonth -= 1;
    }
  }
  function nextMonth() {
    if (isCurrentMonth) return;
    if (viewMonth === 11) {
      viewMonth = 0;
      viewYear += 1;
    } else {
      viewMonth += 1;
    }
  }

  async function refresh() {
    stats = await invoke<StatsFile>("get_stats");
  }
  onMount(() => {
    refresh();
    const id = setInterval(refresh, 5000);
    return () => clearInterval(id);
  });
</script>

<div class="counters">
  <div class="counter">
    <div class="num">{fmtCount(stats.total_recordings)}</div>
    <div class="cap">Recordings</div>
  </div>
  <div class="counter">
    <div class="num">{fmtCount(Math.round(stats.total_chars / 3))}</div>
    <div class="cap">Tokens</div>
  </div>
  <div class="counter">
    <div class="num">{fmtSeconds(stats.total_seconds)}</div>
    <div class="cap">Talk time</div>
  </div>
</div>

<div class="month-nav">
  <button class="nav-btn" onclick={prevMonth} aria-label="Previous month">
    <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><polyline points="15 18 9 12 15 6"/></svg>
  </button>
  <div class="month-name">{MONTH_NAMES[viewMonth]} {viewYear}</div>
  <button class="nav-btn" onclick={nextMonth} disabled={isCurrentMonth} aria-label="Next month">
    <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 18 15 12 9 6"/></svg>
  </button>
</div>

<div class="weekday-row">
  <span>Mon</span><span>Tue</span><span>Wed</span><span>Thu</span><span>Fri</span><span>Sat</span><span>Sun</span>
</div>

<div class="month-grid" style:grid-template-rows="repeat({rowCount}, minmax(0, 1fr))">
  {#each cells as c, i (i)}
    {#if c.empty}
      <div class="cell empty"></div>
    {:else}
      <div
        class="cell lvl-{level(c.stats.recordings)}"
        class:future={c.future}
        class:today={c.date === fmtDate(today)}
        title="{c.date} — {c.stats.recordings} recording{c.stats.recordings === 1 ? '' : 's'}, {fmtSeconds(c.stats.seconds)}"
      >
        <span class="d">{c.day}</span>
        {#if c.stats.recordings > 0}
          <span class="n">{c.stats.recordings}</span>
        {/if}
      </div>
    {/if}
  {/each}
</div>

<style>
  /* Equal grid columns with content left-aligned. All three numbers start
     at the same x inside their column, so the visual rhythm reads as a
     single row regardless of digit count. */
  .counters {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--space-16);
    padding: 4px 4px var(--space-16);
    margin-bottom: var(--space-16);
  }
  .counter {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
    min-width: 0;
  }
  .counter .num {
    font-size: 56px;
    font-weight: 800;
    line-height: 0.9;
    letter-spacing: -1.6px;
    font-variant-numeric: tabular-nums;
    color: var(--fg);
    white-space: nowrap;
  }
  .counter .cap {
    font: 700 10px / 1 inherit;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.8px;
  }

  .month-nav {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-12);
    margin-bottom: var(--space-12);
  }
  .nav-btn {
    appearance: none;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background: var(--bg-elevated);
    border: 1px solid var(--border);
    color: var(--fg);
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    transition: background 0.15s var(--ease-out), color 0.15s var(--ease-out), border-color 0.15s var(--ease-out);
  }
  .nav-btn:hover:not(:disabled) {
    background: var(--accent);
    color: var(--accent-fg);
    border-color: var(--accent);
  }
  .nav-btn:disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }
  .month-name {
    font: 600 var(--text-base) / 1 inherit;
    color: var(--fg);
    letter-spacing: -0.2px;
    min-width: 160px;
    text-align: center;
  }

  .weekday-row {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 6px;
    margin-bottom: 6px;
    text-align: center;
  }
  .weekday-row span {
    font: 600 10px / 1 inherit;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.6px;
  }

  /* Six fr rows so all weeks share whatever vertical space is left in the
     activity zone — was using aspect-ratio: 1 which made the cells huge
     and clipped the bottom rows. */
  .month-grid {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    /* grid-template-rows is set inline (5 or 6 rows depending on month) */
    gap: 6px;
    min-height: 0;
  }
  .cell {
    background: var(--surface);
    border-radius: var(--radius-sm);
    padding: 5px 7px;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    color: var(--fg);
    font-variant-numeric: tabular-nums;
    transition: transform 0.15s var(--ease-out);
    overflow: hidden;
    min-height: 0;
  }
  .cell:hover:not(.empty):not(.future) { transform: translateY(-1px); }
  .cell.empty { background: transparent; }
  .cell.future { opacity: 0.35; }
  .cell.today { box-shadow: inset 0 0 0 1.5px var(--accent); }

  .d {
    font: 600 11px / 1 inherit;
    color: var(--muted);
  }
  .n {
    align-self: flex-end;
    font: 700 13px / 1 inherit;
    color: var(--fg);
    letter-spacing: -0.3px;
  }

  .cell.lvl-1 { background: color-mix(in srgb, var(--accent) 22%, var(--surface)); }
  .cell.lvl-2 { background: color-mix(in srgb, var(--accent) 45%, var(--surface)); }
  .cell.lvl-3 { background: color-mix(in srgb, var(--accent) 70%, var(--surface)); }
  .cell.lvl-4 { background: var(--accent); }
  .cell.lvl-3 .d, .cell.lvl-4 .d { color: var(--accent-fg); opacity: 0.7; }
  .cell.lvl-3 .n, .cell.lvl-4 .n { color: var(--accent-fg); }
</style>
