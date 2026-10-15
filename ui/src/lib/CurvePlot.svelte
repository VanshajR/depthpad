<!-- Output vs key travel: dead zone shaded, press point marked, live key position as a dot. -->
<script lang="ts">
  import { applyCurve, type Curve } from "./api";
  import { app } from "./state.svelte";

  let { curve, press = false, liveMm = 0 }: { curve: Curve | null; press?: boolean; liveMm?: number } = $props();

  const W = 600, H = 260, L = 44, T = 10, R = W - 10, B = H - 30;
  let T_MM = $derived(app.travel);
  const x = (mm: number) => L + ((R - L) * mm) / T_MM;
  const y = (v: number) => B - (B - T) * v;

  let points = $derived(
    curve ? Array.from({ length: 201 }, (_, i) => {
      const mm = (T_MM * i) / 200;
      return [x(mm), y(applyCurve(mm / T_MM, curve, T_MM))];
    }) : [],
  );
  let line = $derived(points.map((p) => p.join(",")).join(" "));
  let area = $derived(points.length ? `M${L},${B} L${line.replaceAll(" ", " L")} L${R},${B} Z` : "");
  let live = $derived(curve && liveMm > 0.005 ? [x(Math.min(T_MM, liveMm)), y(applyCurve(Math.min(1, liveMm / T_MM), curve, T_MM))] : null);
</script>

<svg viewBox="0 0 {W} {H}" role="img" aria-label="Response curve">
  {#each [0, 1, 2, 3] as mm}
    <line x1={x(mm)} y1={T} x2={x(mm)} y2={B} class="grid" />
    <text x={x(mm)} y={B + 18} class="tick" text-anchor="middle">{mm} mm</text>
  {/each}
  {#each [0, 0.5, 1] as v}
    <line x1={L} y1={y(v)} x2={R} y2={y(v)} class="grid" />
    <text x={L - 8} y={y(v)} class="tick" text-anchor="end" dominant-baseline="central">{v * 100}%</text>
  {/each}
  {#if curve}
    <rect x={L} y={T} width={x(curve.deadzone_mm) - L} height={B - T} fill="rgba(255,255,255,.03)" />
    <path d={area} fill="color-mix(in srgb, var(--accent) 10%, transparent)" />
    <polyline points={line} fill="none" stroke="var(--accent)" stroke-width="2.4" stroke-linejoin="round" stroke-linecap="round" />
    {#if press}<line x1={x(curve.press_mm)} y1={T} x2={x(curve.press_mm)} y2={B} stroke="var(--held)" stroke-width="1.5" />{/if}
    {#if live}
      <line x1={live[0]} y1={B} x2={live[0]} y2={live[1]} stroke="var(--ink-3)" stroke-width="1" />
      <circle cx={live[0]} cy={live[1]} r="6" fill="var(--ink)" stroke="var(--surface)" stroke-width="2.5" />
    {/if}
  {/if}
</svg>

<style>
  svg { width: 100%; height: 100%; display: block; }
  .grid { stroke: var(--stroke-soft); stroke-width: 1; }
  .tick { fill: var(--ink-3); font: 500 12px var(--font); font-variant-numeric: tabular-nums; }
</style>
