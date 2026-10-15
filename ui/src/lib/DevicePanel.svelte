<!-- Keyboard, stream, virtual controller and input state at a glance. -->
<script lang="ts">
  import { app } from "./state.svelte";

  // depth update rate over the last ~20 s, for the sparkline
  let rates = $state<number[]>([]);
  $effect(() => {
    const id = setInterval(() => {
      rates = [...rates, app.snapshot?.state === "streaming" ? app.snapshot.rate : 0].slice(-40);
    }, 500);
    return () => clearInterval(id);
  });
  let spark = $derived.by(() => {
    if (rates.length < 2) return "";
    const max = Math.max(1100, ...rates);
    return rates.map((r, i) => `${(i / 39) * 100},${22 - (r / max) * 20}`).join(" ");
  });

  let s = $derived(app.snapshot);
  let connection = $derived.by(() => {
    if (!s) return { text: "Starting", tone: "warn" };
    if (s.state === "searching") return { text: "Not found", tone: "error" };
    if (s.state === "streaming") return { text: "Connected", tone: "ok" };
    if (s.state === "noack") return { text: "Not responding", tone: "warn" };
    return { text: "Waiting for data", tone: "warn" };
  });
</script>

<section class="side">
  <h3>Device</h3>
  <dl>
    <dt>Keyboard</dt>
    <dd title={app.device?.name}>{app.device?.name ?? "—"}</dd>

    <dt>Firmware</dt>
    <dd class="tabular">{s?.firmware ?? "—"}</dd>

    <dt>Connection</dt>
    <dd class="state {connection.tone}"><i></i>{connection.text}</dd>

    <dt>Depth updates</dt>
    <dd class="rate">
      <span class="tabular">{s?.state === "streaming" ? `${Math.round(s.rate)}/s` : "—"}</span>
      <svg viewBox="0 0 100 24" preserveAspectRatio="none" aria-hidden="true">
        {#if spark}<polyline points={spark} />{/if}
      </svg>
    </dd>

    <dt>Controller</dt>
    <dd class="state {s?.pad ? 'ok' : 'error'}" title={s?.pad_error ?? ""}><i></i>{s?.pad ? `${app.ps ? "DualShock 4" : "Xbox 360"}, connected` : s?.pad_parked ? "Unplugged while paused" : "Not connected"}</dd>

    <dt>Output</dt>
    <dd>{s?.paused ? "Paused" : "Live"}</dd>

    <dt>Key blocking</dt>
    <dd>{s?.blocking ? "On, mapped keys hidden" : app.profile?.settings.block_keys ? "On" : "Off"}</dd>
  </dl>
  {#if s?.last_event}<p class="event" title={s.last_event}>{s.last_event}</p>{/if}
</section>

<style>
  .side { display: flex; flex-direction: column; min-width: 0; }
  h3 { margin: 0 0 10px; color: var(--ink-3); font-size: var(--fs-caption); font-weight: 600; }
  dl { margin: 0; display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 9px 14px; font-size: var(--fs-label); }
  dt { color: var(--ink-3); white-space: nowrap; }
  dd { margin: 0; text-align: right; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .state { display: flex; align-items: center; justify-content: flex-end; gap: 7px; }
  .state i { width: 6px; height: 6px; border-radius: 50%; background: var(--ink-3); flex: none; }
  .ok i { background: var(--ok); }
  .warn i { background: var(--warn); }
  .error i { background: var(--error); }
  .rate { display: flex; align-items: center; justify-content: flex-end; gap: 8px; }
  svg { width: 56px; height: 16px; flex: none; }
  polyline { fill: none; stroke: var(--accent); stroke-width: 1.5; vector-effect: non-scaling-stroke; }
  .event { margin: 14px 0 0; padding-top: 10px; border-top: 1px solid var(--stroke-soft); color: var(--ink-3);
           font-size: var(--fs-caption); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .event::first-letter { text-transform: uppercase; }
</style>
