<!-- Records a key combination. Click, then press the keys (modifiers first); releasing ends it.
     Esc removes the hotkey; clicking elsewhere cancels. Depthpad's own hotkeys are switched off
     while recording, so a combo that is already a hotkey reaches the window. -->
<script lang="ts">
  import { api } from "./api";
  import { prettyHotkey } from "./layout";
  import { app } from "./state.svelte";

  let { value, label, onchange }: { value: string; label: string; onchange: (combo: string) => void } = $props();

  let recording = $state(false);
  let keys = $state<string[]>([]);

  function start() {
    keys = [];
    recording = true;
    api.suspendHotkeys(true);
  }
  function finish(combo: string | null) {
    recording = false;
    api.suspendHotkeys(false);
    if (combo !== null && combo !== value) onchange(combo);
  }
  // KeyboardEvent.code matches the key names profiles use (KeyP, ControlRight...)
  function down(e: KeyboardEvent) {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.code === "Escape") return finish("");
    if (!e.repeat && app.posByName.has(e.code) && !keys.includes(e.code)) keys = [...keys, e.code];
  }
  function up(e: KeyboardEvent) {
    if (recording && keys.length) {
      e.preventDefault();
      finish(keys.slice(0, 3).join("+"));
    }
  }
</script>

<svelte:window onkeydown={down} onkeyup={up} />

<button class="btn hotkey" class:recording aria-label={label} title={recording ? "Press the keys. Esc removes the hotkey." : "Click to change"}
        onclick={() => (recording ? finish(null) : start())} onblur={() => recording && finish(null)}>
  {#if recording}{keys.length ? prettyHotkey(keys.join("+")) + " …" : "Press keys…"}
  {:else if value}{prettyHotkey(value)}
  {:else}<span class="none">None</span>{/if}
</button>

<style>
  .hotkey { min-width: 170px; height: 30px; padding: 0 12px; font-size: var(--fs-label); border-radius: 7px; flex: none; }
  .recording { border-color: var(--accent); color: var(--accent-ink); }
  .none { color: var(--ink-3); }
</style>
