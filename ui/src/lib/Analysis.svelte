<script lang="ts">
  import type { Report } from "./api";
  import LiveChart from "./LiveChart.svelte";

  let { out }: { out: Report } = $props();

  let meters = $derived([
    { label: "Left stick X", v: out.lx, bipolar: true }, { label: "Left stick Y", v: out.ly, bipolar: true },
    { label: "Right stick X", v: out.rx, bipolar: true }, { label: "Right stick Y", v: out.ry, bipolar: true },
    { label: "Left trigger", v: out.lt, bipolar: false }, { label: "Right trigger", v: out.rt, bipolar: false },
  ]);
  const c1 = "#3987e5", c2 = "#d95926";
</script>

<div class="analysis">
  <div class="meters">
    {#each meters as m}
      {@const v = Math.max(-1, Math.min(1, m.v))}
      <div class="meter">
        <div class="head"><span class="tertiary">{m.label}</span>
          <span class="val tabular">{m.bipolar ? (v >= 0 ? "+" : "") + v.toFixed(2) : Math.round(v * 100) + "%"}</span></div>
        <div class="track" class:bipolar={m.bipolar}>
          {#if m.bipolar}
            <span class="bar" style:left="{50 + Math.min(0, v) * 50}%" style:width="{Math.abs(v) * 50}%"></span>
            <span class="mid"></span>
          {:else}
            <span class="bar" style:left="0" style:width="{Math.max(0, v) * 100}%"></span>
          {/if}
        </div>
      </div>
    {/each}
  </div>
  <div class="charts">
    <LiveChart title="Left stick" series={[{ name: "X", color: c1, index: 1 }, { name: "Y", color: c2, index: 2 }]} lo={-1} hi={1} />
    <LiveChart title="Right stick" series={[{ name: "X", color: c1, index: 3 }, { name: "Y", color: c2, index: 4 }]} lo={-1} hi={1} />
    <LiveChart title="Triggers" series={[{ name: "LT", color: c1, index: 5 }, { name: "RT", color: c2, index: 6 }]} lo={0} hi={1} />
  </div>
  <span class="tertiary">Last 5 seconds. Hover a chart to read values.</span>
</div>

<style>
  .analysis { display: flex; flex-direction: column; gap: 20px; }
  .meters { display: grid; grid-template-columns: repeat(6, 1fr); gap: 22px; }
  .head { display: flex; justify-content: space-between; align-items: baseline; margin-bottom: 10px; }
  .val { font-size: 17px; font-weight: 600; }
  .track { position: relative; height: 6px; border-radius: 3px; background: var(--pressed); }
  .bar { position: absolute; top: 0; bottom: 0; border-radius: 3px; background: var(--accent); }
  .mid { position: absolute; left: 50%; top: -2px; bottom: -2px; width: 1px; background: var(--ink-3); }
  .charts { display: grid; grid-template-columns: repeat(3, 1fr); gap: 24px; }
</style>
