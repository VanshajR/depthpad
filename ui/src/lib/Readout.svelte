<!-- Numeric readout beside the controller: sticks (values + a position pad), triggers, buttons. -->
<script lang="ts">
  import type { Report } from "./api";
  import Glyph from "./Glyph.svelte";
  import { BUTTON_BITS, TARGET_LONG } from "./layout";

  let { out }: { out: Report } = $props();
  const c = (v: number, lo = -1, hi = 1) => Math.max(lo, Math.min(hi, v));
  const sign = (v: number) => (v >= 0 ? "+" : "−") + Math.abs(v).toFixed(2);
  // every button, always shown in the same place, so pressing one never moves the layout
  const ORDER = ["A", "B", "X", "Y", "LB", "RB", "LS", "RS", "BACK", "START", "GUIDE", "DPAD_UP", "DPAD_DOWN", "DPAD_LEFT", "DPAD_RIGHT"];
  const on = (n: string) => (out.buttons & BUTTON_BITS[n]) !== 0;
  let sticks = $derived([
    { name: "Left stick", x: c(out.lx), y: c(out.ly) },
    { name: "Right stick", x: c(out.rx), y: c(out.ry) },
  ]);
  let triggers = $derived([{ name: "Left trigger", v: c(out.lt, 0, 1) }, { name: "Right trigger", v: c(out.rt, 0, 1) }]);
</script>

<div class="readout">
  {#each sticks as s}
    <div class="row stick">
      <div class="pad" aria-hidden="true">
        <span class="cross"></span>
        <span class="dot" class:active={Math.abs(s.x) > 0.01 || Math.abs(s.y) > 0.01}
              style:left="{50 + s.x * 40}%" style:top="{50 - s.y * 40}%"></span>
      </div>
      <div class="meta">
        <span class="name">{s.name}</span>
        <span class="vals"><span>X <b>{sign(s.x)}</b></span><span>Y <b>{sign(s.y)}</b></span></span>
      </div>
    </div>
  {/each}

  {#each triggers as t}
    <div class="row trig">
      <div class="meta">
        <span class="name">{t.name}</span>
        <b class="pct">{Math.round(t.v * 100)}<small>%</small></b>
      </div>
      <div class="bar"><span style:width="{t.v * 100}%"></span></div>
    </div>
  {/each}

  <div class="row buttons">
    <span class="name">Buttons</span>
    <div class="grid">
      {#each ORDER as b}
        <span class="btn-cell" class:on={on(b)} title={TARGET_LONG[b]}><Glyph target={b} size={22} active={on(b)} /></span>
      {/each}
    </div>
  </div>
</div>

<style>
  .readout { display: grid; grid-template-columns: 1fr 1fr; column-gap: 28px; }
  .row { padding: 14px 0; border-top: 1px solid var(--stroke-soft); }
  .row:nth-child(-n + 2) { border-top: 0; }
  .buttons { grid-column: 1 / -1; }
  .name { color: var(--ink-2); font-size: var(--fs-label); }
  b { font-weight: 600; color: var(--ink); font-variant-numeric: tabular-nums; }

  .stick { display: flex; align-items: center; gap: 16px; }
  .pad { position: relative; width: 46px; height: 46px; border-radius: 50%; flex: none; background: var(--bg); border: 1px solid var(--stroke); }
  .cross { position: absolute; inset: 0; background:
             linear-gradient(rgba(255, 255, 255, 0.07), rgba(255, 255, 255, 0.07)) center / 1px 100% no-repeat,
             linear-gradient(rgba(255, 255, 255, 0.07), rgba(255, 255, 255, 0.07)) center / 100% 1px no-repeat; border-radius: 50%; }
  .dot { position: absolute; width: 12px; height: 12px; margin: -6px 0 0 -6px; border-radius: 50%; background: #50545c;
         transition: left 50ms linear, top 50ms linear; }
  .dot.active { background: var(--accent); }
  .meta { display: flex; flex-direction: column; gap: 3px; }
  .vals { display: flex; gap: 16px; color: var(--ink-3); font-size: var(--fs-label); }

  .trig .meta { flex-direction: row; justify-content: space-between; align-items: baseline; margin-bottom: 8px; }
  .pct { font-size: 18px; }
  .pct small { font-size: 12px; color: var(--ink-3); margin-left: 1px; }
  .bar { height: 4px; border-radius: 2px; background: var(--raised); overflow: hidden; }
  .bar span { display: block; height: 100%; border-radius: 3px; background: var(--accent); transition: width 50ms linear; }

  .buttons { display: flex; flex-direction: column; gap: 10px; padding-bottom: 0; }
  .grid { display: grid; grid-template-columns: repeat(8, minmax(0, 1fr)); gap: 6px; }
  .btn-cell { height: 34px; display: grid; place-items: center; border-radius: 7px;
              background: var(--bg); border: 1px solid var(--stroke-soft); transition: background 60ms, border-color 60ms; }
  .btn-cell :global(svg) { opacity: 0.35; transition: opacity 60ms; }
  .btn-cell.on { background: var(--accent-wash); border-color: var(--accent); }
  .btn-cell.on :global(svg) { opacity: 1; }
</style>
