<!-- Asked when several supported keyboards are connected and none is the default. -->
<script lang="ts">
  import { fade, scale } from "svelte/transition";
  import { app } from "./state.svelte";
  import SupportBadge from "./SupportBadge.svelte";

  let k = $derived(app.keyboards);
  let options = $derived(k?.list.filter((x) => x.connected) ?? []);
  let choice = $state<string | null>(null);
  let remember = $state(false);
  let picked = $derived(choice ?? k?.active ?? null);

  function confirm() {
    if (picked) app.useKeyboard(picked, remember);
  }
  // Escape keeps the keyboard already in use, without making it the default.
  function dismiss() {
    if (k) app.useKeyboard(k.active, false);
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && dismiss()} />

<div class="scrim" transition:fade={{ duration: 120 }}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="kc-title" transition:scale={{ start: 0.97, duration: 140 }}>
    <h2 id="kc-title">Which keyboard should Depthpad use?</h2>
    <p class="sub">More than one supported keyboard is connected. Depthpad reads one at a time.</p>

    <div class="list" role="radiogroup" aria-label="Keyboards">
      {#each options as o (o.id)}
        <button class="opt" role="radio" aria-checked={picked === o.id} class:on={picked === o.id} onclick={() => (choice = o.id)}>
          <span class="radio"></span>
          <span class="who"><span class="name">{o.name}</span><span class="vendor">{o.vendor}</span></span>
          <SupportBadge status={o.status} />
        </button>
      {/each}
    </div>

    <label class="remember">
      <input type="checkbox" bind:checked={remember} />
      <span>Always use this keyboard<small>You can change this in Settings.</small></span>
    </label>

    <div class="actions">
      <button class="btn quiet" onclick={dismiss}>Not now</button>
      <button class="btn primary" onclick={confirm} disabled={!picked}>Use keyboard</button>
    </div>
  </div>
</div>

<style>
  .scrim { position: fixed; inset: 0; z-index: 80; display: grid; place-items: center; background: rgba(5, 6, 8, 0.66); }
  .dialog { width: min(460px, calc(100vw - 32px)); padding: 22px 22px 18px; border-radius: 12px;
            background: var(--surface); border: 1px solid var(--stroke); box-shadow: 0 24px 64px rgba(0, 0, 0, 0.55); }
  h2 { margin: 0; font: 600 16px/1.3 var(--font); }
  .sub { margin: 6px 0 16px; color: var(--ink-2); font-size: var(--fs-label); }
  .list { display: flex; flex-direction: column; gap: 6px; }
  .opt { display: flex; align-items: center; gap: 12px; padding: 11px 12px; border-radius: 8px; cursor: pointer; text-align: left;
         background: var(--bg); border: 1px solid var(--stroke-soft); transition: border-color 120ms, background 120ms; }
  .opt:hover { border-color: var(--stroke); }
  .opt.on { border-color: var(--accent); background: var(--accent-wash); }
  .radio { width: 14px; height: 14px; border-radius: 50%; border: 1.5px solid var(--ink-3); flex: none; }
  .on .radio { border: 4px solid var(--accent); }
  .who { flex: 1; display: flex; flex-direction: column; gap: 1px; min-width: 0; }
  .name { font-weight: 600; }
  .vendor { color: var(--ink-3); font-size: var(--fs-caption); }
  .remember { display: flex; align-items: flex-start; gap: 10px; margin: 16px 2px 0; cursor: pointer; }
  .remember input { margin: 2px 0 0; accent-color: var(--accent); }
  .remember span { display: flex; flex-direction: column; gap: 1px; }
  .remember small { color: var(--ink-3); font-size: var(--fs-caption); }
  .actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 20px; }
  .actions .btn { height: 32px; font-size: var(--fs-label); }
  .btn[disabled] { opacity: 0.5; cursor: default; }
</style>
