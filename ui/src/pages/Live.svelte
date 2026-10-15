<script lang="ts">
  import ActiveKeys from "../lib/ActiveKeys.svelte";
  import Analysis from "../lib/Analysis.svelte";
  import DevicePanel from "../lib/DevicePanel.svelte";
  import ControllerArt from "../lib/ControllerArt.svelte";
  import Readout from "../lib/Readout.svelte";
  import Keyboard3D from "../lib/Keyboard3D.svelte";
  import Segmented from "../lib/Segmented.svelte";
  import { app } from "../lib/state.svelte";

  const NEUTRAL = { buttons: 0, lt: 0, rt: 0, lx: 0, ly: 0, rx: 0, ry: 0 };
  let view = $state(Number(localStorage.getItem("depthpad.liveView") ?? 0));
  $effect(() => { localStorage.setItem("depthpad.liveView", String(view)); });
  let out = $derived(app.snapshot?.output ?? NEUTRAL);
</script>

<div class="page">
  <section class="hero">
    <div class="keys-side"><ActiveKeys /></div>
    <div class="board">
      <Keyboard3D depths={app.depths} bindings={app.bindings} />
      <div class="legend">
        <span><i class="m"></i>Measured depth</span>
        <span><i class="h"></i>Held (key down, depth not reported)</span>
      </div>
    </div>
    <div class="device-side"><DevicePanel /></div>
  </section>

  <section class="card output">
    <header class="card-head">
      <div>
        <h2 class="card-title">Controller output</h2>
        <p class="card-sub">Virtual {app.snapshot?.pad_kind === "ds4" ? "DualShock 4" : "Xbox 360"} controller, as games receive it</p>
      </div>
      <Segmented options={["Controller", "Analysis"]} bind:value={view} />
    </header>
    {#if view === 0}
      <div class="split">
        <div class="ctrl"><ControllerArt {out} /></div>
        <Readout {out} />
      </div>
    {:else}
      <Analysis {out} />
    {/if}
  </section>
</div>

<style>
  .page { display: flex; flex-direction: column; gap: 8px; max-width: 1400px; margin: 0 auto; }
  .hero { display: grid; grid-template-columns: 230px minmax(0, 1fr) 230px; gap: 28px; align-items: start; }
  .board { display: flex; flex-direction: column; align-items: center; min-width: 0; }
  .keys-side, .device-side { padding-top: 12px; }
  @media (max-width: 1180px) {
    .hero { grid-template-columns: 220px minmax(0, 1fr); }
    .device-side { display: none; }
  }
  @media (max-width: 900px) {
    .hero { grid-template-columns: minmax(0, 1fr); }
    .keys-side { display: none; }
  }
  .legend { display: flex; gap: 22px; margin-top: -4px; color: var(--ink-3); font-size: var(--fs-caption); }
  .legend span { display: flex; align-items: center; gap: 7px; }
  i { width: 8px; height: 8px; border-radius: 50%; }
  .m { background: var(--accent); }
  .h { background: var(--held); }

  .output { margin-top: 12px; padding: 16px 20px 18px; }
  .card-head { margin-bottom: 8px; }
  .split { display: grid; grid-template-columns: minmax(0, 1fr) minmax(360px, 1fr); gap: 48px; align-items: center; }
  .ctrl { height: clamp(200px, 27vh, 280px); }
</style>
