<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import HotkeyControl from "./HotkeyControl.svelte";
  import MicPicker from "./MicPicker.svelte";

  type Cfg = {
    available: boolean;
    enabled: boolean;
    api_key: string;
    model: string;
    system_prompt: string;
  };

  let cfg = $state<Cfg>({
    available: false,
    enabled: false,
    api_key: "",
    model: "openai/gpt-4o-mini",
    system_prompt: "",
  });
  let saving = $state(false);
  let savedAt = $state<number | null>(null);
  let revealKey = $state(false);

  let original = $state<Cfg | null>(null);
  const dirty = $derived(
    !!original &&
      (cfg.api_key !== original.api_key ||
        cfg.model !== original.model ||
        cfg.system_prompt !== original.system_prompt),
  );

  async function refresh() {
    const c = await invoke<Cfg>("get_postprocess_config");
    cfg = c;
    original = { ...c };
  }

  async function toggle() {
    try {
      await invoke("set_postprocess_enabled", { enabled: !cfg.enabled });
      cfg.enabled = !cfg.enabled;
    } catch (e) {
      alert(String(e));
    }
  }

  async function save() {
    saving = true;
    try {
      await invoke("set_postprocess_config", {
        apiKey: cfg.api_key,
        model: cfg.model,
        systemPrompt: cfg.system_prompt,
      });
      savedAt = Date.now();
      await refresh();
      setTimeout(() => { savedAt = null; }, 1800);
    } catch (e) {
      alert(String(e));
    } finally {
      saving = false;
    }
  }

  onMount(() => {
    refresh();
  });
</script>

<div class="form" data-tauri-no-drag>
  <!-- Hotkey at the top so the most-frequently-tweaked binding is in
       the user's eye-line, not buried below the prompt. -->
  <HotkeyControl />

  <!-- Microphone dropdown — change applies on next launch (recorder
       owns the open WASAPI stream and is created at startup). -->
  <MicPicker />

  <div class="row toggle-row">
    <div class="left">
      <span class="icon" aria-hidden="true">
        <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
          <path d="M9 11l3 3 8-8"/>
          <path d="M20 12v6a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h11"/>
        </svg>
      </span>
      <div class="text">
        <span class="label">OpenRouter cleanup</span>
        <span class="sub">
          {#if !cfg.available}
            Set an API key below
          {:else if cfg.enabled}
            Active · uses settings below
          {:else}
            Off · paste raw transcript
          {/if}
        </span>
      </div>
    </div>
    <button
      class="toggle"
      class:on={cfg.enabled}
      class:disabled={!cfg.available}
      onclick={toggle}
      disabled={!cfg.available}
      aria-label="Toggle post-processing"
    >
      <span class="knob"></span>
    </button>
  </div>

  <label class="field">
    <span class="ftitle">API key</span>
    <div class="input-row">
      <input
        type={revealKey ? "text" : "password"}
        bind:value={cfg.api_key}
        placeholder="sk-or-…"
        spellcheck="false"
        autocomplete="off"
      />
      <button class="reveal" onclick={() => (revealKey = !revealKey)} type="button" aria-label="Reveal">
        {#if revealKey}
          <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94"/><path d="M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19"/><path d="M14.12 14.12a3 3 0 1 1-4.24-4.24"/><line x1="1" y1="1" x2="23" y2="23"/></svg>
        {:else}
          <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/><circle cx="12" cy="12" r="3"/></svg>
        {/if}
      </button>
    </div>
  </label>

  <label class="field">
    <span class="ftitle">Model</span>
    <input class="mono" bind:value={cfg.model} placeholder="openai/gpt-4o-mini" spellcheck="false" />
  </label>

  <div class="field grow">
    <span class="ftitle">System prompt</span>
    <div class="textarea-wrap">
      <textarea class="mono" bind:value={cfg.system_prompt} placeholder="Edit transcripts: fix punctuation, replace mat…"></textarea>
      <button
        class="save-inline primary"
        onclick={save}
        disabled={!dirty || saving}
        aria-live="polite"
      >
        {#if saving}
          Saving…
        {:else if savedAt}
          ✓ Saved
        {:else}
          Save
        {/if}
      </button>
    </div>
  </div>
</div>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
    flex: 1 1 auto;
    min-height: 0;
    /* Visible so the mic dropdown popup can extend past the form
       bounds. The form's natural sizing keeps everything in view
       without needing scroll. */
    overflow: visible;
  }

  .toggle-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-12);
  }
  .left {
    display: flex;
    align-items: center;
    gap: var(--space-12);
    min-width: 0;
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
  .text { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .label { font: 600 var(--text-sm) / 1.2 inherit; color: var(--fg); }
  .sub { font: 500 var(--text-xs) / 1.3 inherit; color: var(--muted); }

  .toggle {
    appearance: none;
    flex-shrink: 0;
    width: 42px;
    height: 24px;
    border: none;
    border-radius: var(--radius-pill);
    background: var(--border);
    cursor: pointer;
    position: relative;
    transition: background-color 0.2s var(--ease-out);
    padding: 0;
  }
  .toggle.on { background: var(--accent); }
  .toggle .knob {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 18px;
    height: 18px;
    background: white;
    border-radius: 50%;
    transition: transform 0.22s cubic-bezier(0.34, 1.56, 0.64, 1);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.25);
  }
  .toggle.on .knob { transform: translateX(18px); }
  .toggle.disabled { opacity: 0.4; cursor: not-allowed; }

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }
  .field.grow {
    flex: 1 1 auto;
    min-height: 0;
  }
  .textarea-wrap {
    position: relative;
    flex: 1 1 auto;
    display: flex;
    min-height: 0;
  }
  .textarea-wrap textarea {
    flex: 1 1 auto;
    min-height: 80px;
    /* Reserve room so the long-form prompt never slides under the inline
       Save button at the bottom-right. */
    padding-bottom: 44px;
  }
  .save-inline {
    position: absolute;
    bottom: 10px;
    right: 10px;
    z-index: 2;
    appearance: none;
    border: none;
    background: var(--accent);
    color: var(--accent-fg);
    font: 600 var(--text-xs) / 1 inherit;
    padding: 8px 14px;
    border-radius: var(--radius-sm);
    cursor: pointer;
    box-shadow: 0 2px 8px rgba(0,0,0,0.15);
    transition: filter 0.15s var(--ease-out), background 0.15s var(--ease-out), color 0.15s var(--ease-out);
  }
  .save-inline:hover:not(:disabled) { filter: brightness(1.1); }
  /* Disabled = fully opaque muted, NOT translucent — the textarea text
     was bleeding through when we used opacity. */
  .save-inline:disabled {
    background: var(--surface);
    color: var(--muted);
    cursor: not-allowed;
    box-shadow: none;
  }

  .ftitle {
    font: 600 11px / 1 inherit;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--muted);
  }

  input, textarea {
    appearance: none;
    width: 100%;
    box-sizing: border-box;
    background: var(--surface);
    border: none;
    color: var(--fg);
    font-family: inherit;
    font-size: var(--text-sm);
    padding: 9px 11px;
    border-radius: var(--radius-sm);
    outline: none;
    box-shadow: inset 0 0 0 1px transparent;
    transition: box-shadow 0.15s var(--ease-out), background 0.15s var(--ease-out);
  }
  textarea {
    resize: none;
    min-height: 60px;
    line-height: 1.5;
    scrollbar-width: none;
  }
  textarea::-webkit-scrollbar { width: 0; height: 0; display: none; }
  /* Monospace stack for code-ish fields (model id, prompts) — gives them a
     editorial / "dev tool" feel and keeps long ids readable. */
  .mono {
    font-family: "JetBrains Mono", ui-monospace, "Cascadia Code", SFMono-Regular, Menlo, "Consolas", monospace;
    font-size: 13px;
    letter-spacing: -0.1px;
  }
  textarea.mono { line-height: 1.55; }
  input:focus, textarea:focus {
    box-shadow: inset 0 0 0 1.5px var(--accent);
    background: var(--bg-elevated);
  }

  .input-row {
    display: flex;
    gap: 4px;
    align-items: stretch;
  }
  .input-row input { flex: 1 1 auto; }
  .reveal {
    appearance: none;
    align-self: stretch;
    width: 34px;
    background: var(--surface);
    border: none;
    border-radius: var(--radius-sm);
    color: var(--muted);
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    transition: background 0.12s var(--ease-out), color 0.12s var(--ease-out);
  }
  .reveal:hover { background: var(--hover); color: var(--fg); }

</style>
