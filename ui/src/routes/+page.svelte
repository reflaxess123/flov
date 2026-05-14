<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import Pill from "$lib/Pill.svelte";

  type State = "idle" | "recording" | "transcribing" | "polished";

  const BAR_COUNT = 20;
  let state = $state<State>("idle");
  let polishedText = $state<string>("");
  let spectrum = $state<number[]>(Array(BAR_COUNT).fill(0));

  onMount(() => {
    const unlisteners: Array<Promise<() => void>> = [
      listen<State>("state-changed", (e) => {
        const next = e.payload;
        if (next === "recording") {
          spectrum = Array(BAR_COUNT).fill(0);
        }
        if (next === "polished") {
          state = next;
          setTimeout(() => {
            // Paste the polished text, then trigger morph-out
            invoke("polished_shown").catch(console.error);
            state = "idle";
          }, 1500);
          return;
        }
        if (next === "idle") {
          // Drive morph-out via {#if}; hide the OS window once transition ends.
          state = "idle";
          // morphPill duration is 460ms; pad slightly so we don't cut it off.
          setTimeout(() => invoke("hide_window").catch(console.error), 520);
          return;
        }
        state = next;
      }),
      listen<number[]>("audio-spectrum", (e) => {
        spectrum = e.payload;
      }),
      listen<string>("polished-text", (e) => {
        polishedText = e.payload;
      }),
    ];
    return () => {
      unlisteners.forEach((p) => p.then((u) => u()));
    };
  });
</script>

<div class="stage">
  <Pill {state} {spectrum} {polishedText} />
</div>

<style>
  :global(html, body) {
    background: transparent !important;
    margin: 0;
    overflow: hidden;
    font-family:
      -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif;
  }
  .stage {
    width: 100vw;
    height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
  }
</style>
