<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  type MicState = { devices: string[]; selected: string | null };

  let mics = $state<MicState>({ devices: [], selected: null });
  let micValue = $derived(mics.selected ?? "");
  let micOpen = $state(false);
  let micWrapEl: HTMLElement | undefined = $state();

  async function refreshMics() {
    try {
      const v = await invoke<MicState>("list_audio_inputs");
      mics = v;
    } catch (e) {
      console.error("list_audio_inputs failed", e);
    }
  }

  async function pickMic(value: string) {
    const next = value.trim() === "" ? null : value;
    try {
      await invoke("set_audio_input", { device: next });
      mics = { ...mics, selected: next };
    } catch (e) {
      alert(String(e));
    }
  }

  function chooseMic(value: string) {
    pickMic(value);
    micOpen = false;
  }

  $effect(() => {
    if (!micOpen) return;

    const onClick = (e: MouseEvent) => {
      if (micWrapEl && !micWrapEl.contains(e.target as Node)) micOpen = false;
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") micOpen = false;
    };

    document.addEventListener("mousedown", onClick, true);
    document.addEventListener("keydown", onKey, true);
    return () => {
      document.removeEventListener("mousedown", onClick, true);
      document.removeEventListener("keydown", onKey, true);
    };
  });

  onMount(() => {
    refreshMics();
  });
</script>

<div class="mic-row">
  <div class="left">
    <span class="icon" aria-hidden="true">
      <svg
        viewBox="0 0 24 24"
        width="14"
        height="14"
        fill="none"
        stroke="currentColor"
        stroke-width="1.7"
        stroke-linecap="round"
        stroke-linejoin="round"
      >
        <path d="M12 2a3 3 0 0 0-3 3v6a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3z" />
        <path d="M19 11a7 7 0 0 1-14 0" />
        <line x1="12" y1="18" x2="12" y2="22" />
        <line x1="8" y1="22" x2="16" y2="22" />
      </svg>
    </span>
    <div class="text">
      <span class="label">Microphone</span>
      <span class="sub">
        {#if mics.devices.length === 0}
          No inputs detected
        {:else}
          Restart flov after changing
        {/if}
      </span>
    </div>
  </div>
  <div class="mic-dd" bind:this={micWrapEl}>
    <button
      type="button"
      class="mic-trigger"
      class:open={micOpen}
      onclick={() => (micOpen = !micOpen)}
      disabled={mics.devices.length === 0}
      aria-haspopup="listbox"
      aria-expanded={micOpen}
    >
      <span class="mic-value">{micValue || "System default"}</span>
      <svg
        class="chev"
        viewBox="0 0 24 24"
        width="12"
        height="12"
        fill="none"
        stroke="currentColor"
        stroke-width="2.2"
        stroke-linecap="round"
        stroke-linejoin="round"
      >
        <polyline points="6 9 12 15 18 9" />
      </svg>
    </button>
    {#if micOpen}
      <ul class="mic-menu" role="listbox">
        <li>
          <button
            type="button"
            class="mic-opt"
            class:active={micValue === ""}
            onclick={() => chooseMic("")}
          >
            System default
            {#if micValue === ""}
              <svg
                viewBox="0 0 24 24"
                width="12"
                height="12"
                fill="none"
                stroke="currentColor"
                stroke-width="2.6"
                stroke-linecap="round"
                stroke-linejoin="round"
              >
                <polyline points="20 6 9 17 4 12" />
              </svg>
            {/if}
          </button>
        </li>
        {#each mics.devices as d (d)}
          <li>
            <button
              type="button"
              class="mic-opt"
              class:active={d === micValue}
              onclick={() => chooseMic(d)}
            >
              <span class="mic-opt-name">{d}</span>
              {#if d === micValue}
                <svg
                  viewBox="0 0 24 24"
                  width="12"
                  height="12"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2.6"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                >
                  <polyline points="20 6 9 17 4 12" />
                </svg>
              {/if}
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</div>

<style>
  .mic-row {
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    gap: var(--space-12);
  }
  .left {
    display: flex;
    align-items: center;
    gap: var(--space-12);
    min-width: 0;
    flex: 1 1 auto;
  }
  .icon {
    width: 28px;
    height: 28px;
    border-radius: var(--radius-sm);
    background: var(--surface);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--muted);
    flex-shrink: 0;
  }
  .text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .label {
    font: 600 var(--text-sm) / 1.2 inherit;
    color: var(--fg);
  }
  .sub {
    font: 500 var(--text-xs) / 1.3 inherit;
    color: var(--muted);
  }
  .mic-dd {
    position: relative;
    flex: 0 1 auto;
    min-width: 0;
    max-width: 60%;
  }
  .mic-trigger {
    appearance: none;
    width: 100%;
    background: var(--surface);
    border: none;
    color: var(--fg);
    font: 600 var(--text-xs) / 1 inherit;
    padding: 8px 10px 8px 12px;
    border-radius: var(--radius-sm);
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 8px;
    transition: background-color 0.15s var(--ease-out);
  }
  .mic-trigger:hover:not(:disabled) {
    background-color: var(--hover);
  }
  .mic-trigger:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .mic-value {
    flex: 1 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-align: left;
  }
  .chev {
    flex-shrink: 0;
    color: var(--muted);
    transition: transform 0.18s var(--ease-out);
  }
  .mic-trigger.open .chev {
    transform: rotate(180deg);
    color: var(--fg);
  }
  .mic-menu {
    position: absolute;
    top: calc(100% + 4px);
    right: 0;
    width: max-content;
    min-width: 100%;
    max-width: 360px;
    z-index: 50;
    list-style: none;
    margin: 0;
    padding: 4px;
    background: var(--bg-elevated);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-card);
    max-height: 240px;
    overflow-y: auto;
    animation: ddIn 0.16s var(--ease-out);
  }
  @keyframes ddIn {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
  .mic-menu li {
    display: block;
  }
  .mic-opt {
    appearance: none;
    width: 100%;
    background: transparent;
    border: none;
    color: var(--fg);
    font: 500 var(--text-xs) / 1.3 inherit;
    text-align: left;
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    transition: background-color 0.12s var(--ease-out);
  }
  .mic-opt:hover {
    background: var(--hover);
  }
  .mic-opt.active {
    background: var(--accent);
    color: var(--accent-fg);
  }
  .mic-opt.active:hover {
    background: var(--accent);
  }
  .mic-opt-name {
    flex: 1 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
