<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  type PendingCombo = { trigger: string; mods: string[] };

  let hotkey = $state("Ctrl+Win");
  let capturing = $state(false);
  let preview = $state("");
  // Buffered combo built up across keydowns; committed on the first keyup.
  // This lets us bind a single key and multi-key combos with the same loop.
  let pending: PendingCombo | null = null;

  async function refreshHotkey() {
    const v = await invoke<{ combo: string }>("get_hotkey");
    hotkey = v.combo;
  }

  function startCapture() {
    capturing = true;
    preview = "";
    pending = null;
    window.addEventListener("keydown", onCaptureKeyDown, { capture: true });
    window.addEventListener("keyup", onCaptureKeyUp, { capture: true });
  }

  function cancelCapture() {
    capturing = false;
    preview = "";
    pending = null;
    window.removeEventListener("keydown", onCaptureKeyDown, { capture: true });
    window.removeEventListener("keyup", onCaptureKeyUp, { capture: true });
  }

  async function commitHotkey(combo: string) {
    cancelCapture();
    try {
      await invoke("set_hotkey", { combo });
      hotkey = combo;
    } catch (e) {
      alert(String(e));
    }
  }

  function baseModifier(e: KeyboardEvent): string | null {
    const map: Record<string, string> = {
      Control: "Ctrl",
      Alt: "Alt",
      Shift: "Shift",
      Meta: "Win",
    };
    return map[e.key] ?? null;
  }

  function pressedToken(e: KeyboardEvent): string | null {
    const baseMod = baseModifier(e);
    if (baseMod) {
      const side = e.location === 2 ? "R" : e.location === 1 ? "L" : "";
      return side + baseMod;
    }
    if (e.key === " ") return "Space";
    if (e.key.length === 1 && /[\w]/.test(e.key)) return e.key.toUpperCase();
    return null;
  }

  function onCaptureKeyDown(e: KeyboardEvent) {
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape") {
      cancelCapture();
      return;
    }

    const trigger = pressedToken(e);
    if (!trigger) return;

    const baseMod = baseModifier(e);
    const mods: string[] = [];
    if (e.ctrlKey && baseMod !== "Ctrl") mods.push("Ctrl");
    if (e.altKey && baseMod !== "Alt") mods.push("Alt");
    if (e.shiftKey && baseMod !== "Shift") mods.push("Shift");
    if (e.metaKey && baseMod !== "Win") mods.push("Win");

    pending = { trigger, mods };
    preview = [...mods, trigger].join("+");
  }

  function onCaptureKeyUp(e: KeyboardEvent) {
    e.preventDefault();
    e.stopPropagation();
    if (!pending) return;
    const combo = [...pending.mods, pending.trigger].join("+");
    commitHotkey(combo);
  }

  onMount(() => {
    refreshHotkey();
    return () => {
      if (capturing) cancelCapture();
    };
  });
</script>

<div class="hotkey-row">
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
        <rect x="2" y="6" width="20" height="12" rx="2" />
        <path d="M6 10h0M10 10h0M14 10h0M18 10h0M7 14h10" />
      </svg>
    </span>
    <div class="text">
      <span class="label">Hotkey</span>
      <span class="sub">
        {#if capturing}
          Press your combo · Esc to cancel
        {:else}
          Hold to record, release to transcribe
        {/if}
      </span>
    </div>
  </div>
  {#if capturing}
    <span class="combo capturing">{preview || "…"}</span>
  {:else}
    <span class="combo">{hotkey}</span>
  {/if}
  <button class="change-btn" onclick={capturing ? cancelCapture : startCapture}>
    {capturing ? "Cancel" : "Change"}
  </button>
</div>

<style>
  .hotkey-row {
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
  .combo {
    font-family:
      "JetBrains Mono", ui-monospace, "Cascadia Code", SFMono-Regular, Menlo,
      monospace;
    font-size: 12px;
    font-weight: 600;
    line-height: 1;
    color: var(--fg);
    background: var(--surface);
    padding: 8px 12px;
    border-radius: var(--radius-sm);
    letter-spacing: 0.2px;
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
  }
  .combo.capturing {
    color: var(--accent);
    background: var(--accent-soft);
    animation: pulse 1.2s var(--ease-out) infinite;
  }
  @keyframes pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.55;
    }
  }
  .change-btn {
    appearance: none;
    border: none;
    background: var(--surface);
    color: var(--fg);
    font: 600 var(--text-xs) / 1 inherit;
    padding: 8px 12px;
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition:
      background 0.15s var(--ease-out),
      color 0.15s var(--ease-out);
    flex-shrink: 0;
  }
  .change-btn:hover {
    background: var(--hover);
  }
</style>
