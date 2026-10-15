<!-- The profile's response curve, drawn and edited directly: drag the four points. The first
     point is the dead zone (travel only), the last is full output (travel and output). Changes
     are reported when a drag ends. The key under your finger shows as a live dot. -->
<script lang="ts">
  import { applyCurve, pointsOf, type Curve } from "./api";
  import { app } from "./state.svelte";

  let { curve, liveMm = 0, onchange }: { curve: Curve; liveMm?: number; onchange: (points: [number, number][]) => void } = $props();

  const W = 640, H = 300, L = 46, T = 16, R = W - 16, B = H - 34;
  let travel = $derived(app.travel);
  const x = (mm: number) => L + ((R - L) * mm) / travel;
  const y = (v: number) => B - (B - T) * v;

  // the points being dragged (local until the drag ends)
  let draft = $state<[number, number][] | null>(null);
  let pts = $derived(draft ?? pointsOf(curve));
  let shown = $derived({ ...curve, points: pts });
  let line = $derived(
    Array.from({ length: 241 }, (_, i) => {
      const mm = (travel * i) / 240;
      return `${x(mm)},${y(applyCurve(mm / travel, shown, travel))}`;
    }).join(" "),
  );
  let area = $derived(`M${L},${B} L${line.replaceAll(" ", " L")} L${R},${B} Z`);
  let live = $derived(liveMm > 0.005 ? [x(Math.min(travel, liveMm)), y(applyCurve(Math.min(1, liveMm / travel), shown, travel))] : null);

  let svg: SVGSVGElement;
  let dragging = $state<number | null>(null);
  let hover = $state<number | null>(null);

  function toCurve(e: PointerEvent): [number, number] {
    const m = svg.getScreenCTM();
    if (!m) return [0, 0];
    const p = new DOMPoint(e.clientX, e.clientY).matrixTransform(m.inverse());
    return [((p.x - L) / (R - L)) * travel, (B - p.y) / (B - T)];
  }

  const round = (v: number, step: number) => Math.round(v / step) * step;

  function move(e: PointerEvent) {
    if (dragging === null) return;
    const i = dragging;
    const next = pts.map((p) => [p[0], p[1]] as [number, number]);
    let [mm, v] = toCurve(e);
    const lo = i === 0 ? 0 : next[i - 1][0] + 0.05;
    const hi = i === 3 ? travel : next[i + 1][0] - 0.05;
    mm = Math.min(hi, Math.max(lo, round(mm, 0.01)));
    v = Math.min(1, Math.max(0, round(v, 0.01)));
    next[i] = [mm, i === 0 ? 0 : v];
    draft = next;
  }

  function end() {
    if (dragging !== null && draft) onchange(draft);
    dragging = null;
    draft = null;
  }
</script>

<svelte:window onpointermove={move} onpointerup={end} />

<svg bind:this={svg} viewBox="0 0 {W} {H}" role="img" aria-label="Response curve editor" class:dragging={dragging !== null}>
  {#each Array.from({ length: Math.floor(travel) + 1 }, (_, i) => i) as mm}
    <line x1={x(mm)} y1={T} x2={x(mm)} y2={B} class="grid" />
    <text x={x(mm)} y={B + 20} class="tick" text-anchor="middle">{mm} mm</text>
  {/each}
  {#each [0, 0.5, 1] as v}
    <line x1={L} y1={y(v)} x2={R} y2={y(v)} class="grid" />
    <text x={L - 8} y={y(v)} class="tick" text-anchor="end" dominant-baseline="central">{v * 100}%</text>
  {/each}

  <rect x={L} y={T} width={Math.max(0, x(pts[0][0]) - L)} height={B - T} class="dead" />
  <path d={area} class="area" />
  <polyline points={line} class="line" />

  {#if live}
    <line x1={live[0]} y1={B} x2={live[0]} y2={live[1]} class="liveline" />
    <circle cx={live[0]} cy={live[1]} r="5" class="live" />
  {/if}

  {#if curve.mode !== "turbo"}
    {#each pts as p, i}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <g class="handle" class:on={dragging === i || hover === i}
         onpointerdown={(e) => { e.preventDefault(); dragging = i; draft = pts.map((q) => [q[0], q[1]] as [number, number]); }}
         onpointerenter={() => (hover = i)} onpointerleave={() => (hover = null)}>
        <circle cx={x(p[0])} cy={y(p[1])} r="16" class="hit" />
        <circle cx={x(p[0])} cy={y(p[1])} r="6.5" class="dot" />
      </g>
      {#if dragging === i || hover === i}
        <text x={Math.min(R - 60, Math.max(L + 60, x(p[0])))} y={Math.max(T + 12, y(p[1]) - 16)} class="tip" text-anchor="middle">
          {p[0].toFixed(2)} mm · {Math.round(p[1] * 100)}%
        </text>
      {/if}
    {/each}
  {/if}
</svg>

<style>
  svg { width: 100%; height: 100%; display: block; touch-action: none; user-select: none; }
  .grid { stroke: var(--stroke-soft); stroke-width: 1; }
  .tick { fill: var(--ink-3); font: 500 12px var(--font); font-variant-numeric: tabular-nums; }
  .dead { fill: color-mix(in srgb, var(--ink) 4%, transparent); }
  .area { fill: color-mix(in srgb, var(--accent) 12%, transparent); }
  .line { fill: none; stroke: var(--accent); stroke-width: 2.4; stroke-linejoin: round; stroke-linecap: round; }
  .liveline { stroke: var(--ink-3); stroke-width: 1; }
  .live { fill: var(--ink); stroke: var(--surface); stroke-width: 2.5; }
  .handle { cursor: grab; }
  .dragging, .dragging .handle { cursor: grabbing; }
  .hit { fill: transparent; }
  .dot { fill: var(--surface); stroke: var(--accent); stroke-width: 2.5; transition: r 100ms; }
  .handle.on .dot { r: 8; fill: var(--accent); }
  .tip { fill: var(--ink); font: 600 12px var(--font); font-variant-numeric: tabular-nums;
         paint-order: stroke; stroke: var(--surface); stroke-width: 4px; }
</style>
