<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";

  type ModelInfo = {
    id: string;
    family: string;
    label: string;
    filename: string;
    size_bytes: number;
    url: string;
    languages: string;
    notes: string;
    local_path: string;
    downloaded: boolean;
    active: boolean;
  };
  type Progress = {
    id: string;
    downloaded: number;
    total: number;
    done: boolean;
    error: string | null;
  };

  let models = $state<ModelInfo[]>([]);
  let progress = $state<Record<string, Progress>>({});
  let busy = $state<Record<string, boolean>>({});

  const visible = $derived(models.filter((m) => m.family === "whisper"));

  function fmtBytes(n: number): string {
    if (n < 1024) return `${n} B`;
    const k = n / 1024;
    if (k < 1024) return `${k.toFixed(0)} KB`;
    const m = k / 1024;
    if (m < 1024) return `${m.toFixed(m < 10 ? 1 : 0)} MB`;
    const g = m / 1024;
    return `${g.toFixed(g < 10 ? 2 : 1)} GB`;
  }

  async function refresh() { models = await invoke<ModelInfo[]>("list_models"); }
  async function download(id: string) {
    busy[id] = true;
    try { await invoke("download_model", { id }); }
    catch (e) { console.error(e); busy[id] = false; }
  }
  async function setActive(id: string) {
    try { await invoke("set_active_model", { id }); await refresh(); }
    catch (e) { console.error(e); }
  }
  async function del(e: MouseEvent, id: string) {
    e.stopPropagation();
    if (!confirm("Delete this model file?")) return;
    try { await invoke("delete_model", { id }); delete progress[id]; await refresh(); }
    catch (e) { console.error(e); }
  }

  onMount(() => {
    refresh();
    const off = listen<Progress>("model-progress", (e) => {
      const p = e.payload;
      progress = { ...progress, [p.id]: p };
      if (p.done) { busy[p.id] = false; refresh(); }
    });
    return () => { off.then((u) => u()); };
  });
</script>

<ul class="rows">
  {#each visible as m, i (m.id)}
    {@const p = progress[m.id]}
    {@const downloading = busy[m.id] && (!p || !p.done)}
    {@const pct = p ? Math.min(100, (p.downloaded / Math.max(1, p.total)) * 100) : 0}
    <li class="row" class:active={m.active} class:dl={m.downloaded && !m.active} title={m.notes} style:--i={i}>
      <span class="name">{m.label}</span>
      <span class="size">{fmtBytes(m.size_bytes)}</span>
      <span class="spacer"></span>

      {#if downloading}
        <span class="progress"><span class="bar" style:width="{pct.toFixed(1)}%"></span></span>
        <span class="pct">{pct.toFixed(0)}%</span>
        <span class="actions">
          <button class="round dim" disabled aria-label="Downloading">
            <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round">
              <line x1="12" y1="2" x2="12" y2="6"/>
              <line x1="12" y1="18" x2="12" y2="22"/>
              <line x1="4.93" y1="4.93" x2="7.76" y2="7.76"/>
              <line x1="16.24" y1="16.24" x2="19.07" y2="19.07"/>
              <line x1="2" y1="12" x2="6" y2="12"/>
              <line x1="18" y1="12" x2="22" y2="12"/>
            </svg>
          </button>
        </span>
      {:else if p?.error}
        <span class="err" title={p.error}>error</span>
        <span class="actions">
          <button class="round" onclick={() => download(m.id)} aria-label="Retry download">
            <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
          </button>
        </span>
      {:else if m.downloaded}
        <span class="actions">
          <button class="round check-btn" onclick={() => setActive(m.id)} aria-label={m.active ? "Active" : "Use this model"}>
            <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>
          </button>
          <button class="round del-btn" onclick={(e) => del(e, m.id)} aria-label="Delete">
            <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18"/><path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/><path d="M10 11v6"/><path d="M14 11v6"/></svg>
          </button>
        </span>
      {:else}
        <span class="actions">
          <button class="round dl-btn" onclick={() => download(m.id)} aria-label="Download">
            <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
          </button>
        </span>
      {/if}
    </li>
  {/each}
</ul>

<style>
  ul.rows {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    overflow-y: auto;
    flex: 1 1 auto;
    min-height: 0;
    padding-right: 4px;
    margin-right: -4px;
  }

  li.row {
    position: relative;
    display: flex;
    align-items: center;
    gap: var(--space-12);
    /* Fixed height (matches Backend rows) instead of vertical padding —
       this avoids 1–2px box-model drift between the two lists. */
    height: 44px;
    padding: 0 14px;
    background: var(--surface);
    border-radius: var(--radius-md);
    font-size: var(--text-sm);
    line-height: 1;
    color: var(--fg);
    transition: background 0.15s var(--ease-out), color 0.15s var(--ease-out);
    animation: rowIn 0.25s var(--ease-out) both;
    animation-delay: calc(var(--i) * 18ms);
  }
  @keyframes rowIn {
    from { opacity: 0; transform: translateY(4px); }
    to   { opacity: 1; transform: translateY(0); }
  }
  li.row:hover { background: var(--hover); }
  li.row.active {
    background: var(--accent);
    color: var(--accent-fg);
  }
  li.row.active:hover { background: var(--accent); }

  .name { font-weight: 600; flex-shrink: 0; }
  .size {
    font: 500 var(--text-xs) / 1 inherit;
    opacity: 0.7;
    font-variant-numeric: tabular-nums;
    flex-shrink: 0;
  }
  .spacer { flex: 1 1 auto; }

  .progress {
    width: 90px;
    height: 4px;
    border-radius: var(--radius-pill);
    background: color-mix(in srgb, currentColor 18%, transparent);
    overflow: hidden;
  }
  .progress .bar {
    display: block;
    height: 100%;
    background: var(--accent);
    transition: width 0.3s var(--ease-out);
  }
  li.row.active .progress .bar { background: var(--accent-fg); }
  .pct {
    font: 500 var(--text-xs) / 1 inherit;
    opacity: 0.75;
    font-variant-numeric: tabular-nums;
    min-width: 32px;
    text-align: right;
  }
  .err { font-size: var(--text-xs); color: var(--danger); }

  /* === actions cluster ===
     Two-button cluster (delete + check) for downloaded rows. The delete is
     translated off the right edge and faded out by default; on row hover it
     slides in and the check shifts left to make room. Single-button rows
     (download, retry) just sit at the right edge. */
  .actions {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }

  button.round {
    appearance: none;
    width: 26px;
    height: 26px;
    border-radius: 50%;
    border: 1px solid var(--border);
    background: var(--bg-elevated);
    color: var(--fg);
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    transition:
      background 0.15s var(--ease-out),
      color 0.15s var(--ease-out),
      transform 0.2s var(--ease-out),
      opacity 0.18s var(--ease-out),
      border-color 0.15s var(--ease-out);
  }
  button.round:hover:not(:disabled) {
    background: var(--accent);
    color: var(--accent-fg);
    border-color: var(--accent);
  }
  button.round:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
  button.round.dim {
    color: var(--muted);
    border-color: var(--border);
    background: transparent;
  }

  /* check is filled white-on-accent inside the active row so it stands out */
  li.row.active button.check-btn {
    background: var(--accent-fg);
    color: var(--accent);
    border-color: transparent;
  }
  li.row.active button.check-btn:hover {
    filter: brightness(0.95);
    background: var(--accent-fg);
    color: var(--accent);
  }

  /* === delete slides in to the RIGHT of the check on row hover ===
     Hidden via negative margin-left so it takes no space when collapsed.
     Hovering the row gives it room (margin → 0), check naturally shifts
     left because the actions cluster grew. */
  button.del-btn {
    opacity: 0;
    margin-left: -32px;
    pointer-events: none;
    transform: scale(0.7);
    color: var(--muted);
    transition:
      opacity 0.18s var(--ease-out),
      margin-left 0.22s var(--ease-out),
      transform 0.22s var(--ease-spring),
      background 0.15s var(--ease-out),
      color 0.15s var(--ease-out),
      border-color 0.15s var(--ease-out);
  }
  li.row:hover button.del-btn {
    opacity: 1;
    margin-left: 0;
    transform: scale(1);
    pointer-events: auto;
  }
  button.del-btn:hover:not(:disabled) {
    background: var(--danger) !important;
    border-color: var(--danger) !important;
    color: #ffffff !important;
  }
  li.row.active button.del-btn {
    color: var(--accent-fg);
    background: transparent;
    border-color: color-mix(in srgb, var(--accent-fg) 30%, transparent);
  }
</style>
