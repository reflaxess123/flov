<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  type State = { choice: string; available: string[] };

  type Opt = { id: string; label: string; sub: string };
  const OPTIONS: Opt[] = [
    { id: "cuda",   label: "CUDA",   sub: "NVIDIA GPU" },
    { id: "vulkan", label: "Vulkan", sub: "AMD / Intel iGPU" },
    { id: "metal",  label: "Metal",  sub: "Apple Silicon" },
    { id: "cpu",    label: "CPU",    sub: "Slow, no GPU" },
  ];
  // Mirrors transcribe::BACKEND_PRIORITY in Rust.
  const PRIORITY = ["cuda", "vulkan", "metal", "cpu"];

  let state = $state<State>({ choice: "auto", available: [] });

  // The first available in priority order — what auto-mode would resolve to.
  const best = $derived(PRIORITY.find((b) => state.available.includes(b)) ?? "cpu");
  // Which row is "active" — the user's explicit pick, or `best` if they're
  // still in auto.
  const activeId = $derived(state.choice === "auto" ? best : state.choice);

  async function refresh() {
    state = await invoke<State>("get_backend_state");
  }
  async function pick(id: string) {
    try { await invoke("set_backend_choice", { choice: id }); await refresh(); }
    catch (e) { alert(String(e)); }
  }

  onMount(() => { refresh(); });
</script>

<ul class="rows">
  {#each OPTIONS as o, i (o.id)}
    {@const enabled = state.available.includes(o.id)}
    {@const selected = activeId === o.id}
    {@const isBest = best === o.id}
    <li
      class="row"
      class:active={selected}
      class:disabled={!enabled}
      style:--i={i}
    >
      <button class="bare" onclick={() => enabled && pick(o.id)} disabled={!enabled} aria-label={o.label}>
        <span class="name">{o.label}</span>
        <span class="size">{o.sub}</span>
        {#if isBest && enabled}
          <span class="badge">Best</span>
        {/if}
        <span class="spacer"></span>
        <span class="check" class:on={selected} aria-hidden="true">
          {#if selected}
            <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>
          {/if}
        </span>
      </button>
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
  }

  li.row {
    /* Identical fixed height to Models rows so the two columns align
       row-by-row visually. */
    height: 44px;
    border-radius: var(--radius-md);
    background: var(--surface);
    transition: background 0.15s var(--ease-out), color 0.15s var(--ease-out);
    color: var(--fg);
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
  li.row.disabled {
    pointer-events: none;
    color: var(--muted);
  }
  li.row.disabled .name { opacity: 0.55; }
  li.row.disabled .size { opacity: 0.55; }
  li.row.disabled .check {
    opacity: 0.4;
    background: transparent;
  }

  button.bare {
    width: 100%;
    height: 100%;
    appearance: none;
    background: transparent !important;
    border: none !important;
    padding: 0 14px !important;
    border-radius: inherit !important;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: var(--space-12);
  }
  li.row.disabled button.bare { cursor: not-allowed; }

  .name { font-weight: 600; flex-shrink: 0; font-size: var(--text-sm); }
  .size {
    font: 500 var(--text-xs) / 1 inherit;
    opacity: 0.7;
    flex-shrink: 0;
  }
  .spacer { flex: 1 1 auto; }

  .badge {
    font: 700 9px / 1 inherit;
    text-transform: uppercase;
    letter-spacing: 0.6px;
    padding: 4px 8px;
    border-radius: var(--radius-pill);
    background: color-mix(in srgb, currentColor 14%, transparent);
    color: inherit;
    flex-shrink: 0;
  }
  li.row.active .badge {
    background: color-mix(in srgb, var(--accent-fg) 22%, transparent);
    color: var(--accent-fg);
  }

  .check {
    width: 26px;
    height: 26px;
    border-radius: 50%;
    border: 1px solid var(--border);
    background: var(--bg-elevated);
    color: var(--fg);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    transition: background 0.15s var(--ease-out), color 0.15s var(--ease-out), border-color 0.15s var(--ease-out);
  }
  li.row.active .check {
    background: var(--accent-fg);
    color: var(--accent);
    border-color: transparent;
  }
</style>
