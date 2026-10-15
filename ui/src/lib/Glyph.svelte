<!-- One recognisable mark per controller input (as in HallJoy's remap view): face buttons in their
     colours, bumpers and triggers as their shapes, stick directions as a cap with an arrow and an
     L/R tab, d-pad directions as a cross with the active arm lit. Drawn in a 100x100 box. -->
<script lang="ts">
  import { FACE, PS_FACE, PS_SHORT } from "./layout";
  import { app } from "./state.svelte";
  let { target, size = 24, active = false }: { target: string; size?: number; active?: boolean } = $props();

  const NEUTRAL = "#c9ced6";
  const FILL = "#3a3f47";
  const ACCENT = "var(--accent)";
  const ANGLE: Record<string, number> = { UP: -90, DOWN: 90, LEFT: 180, RIGHT: 0 };

  let kind = $derived(
    target in FACE ? "face"
    : target === "LB" || target === "RB" ? "bumper"
    : target === "LT" || target === "RT" ? "trigger"
    : /^(LS|RS)_/.test(target) ? "stickdir"
    : target === "LS" || target === "RS" ? "click"
    : target.startsWith("DPAD_") ? "dpad"
    : "system",
  );
  let dir = $derived(target.split("_")[1] ?? "");
</script>

<svg width={size} height={size} viewBox="0 0 100 100" aria-hidden="true">
  {#if kind === "face" && app.ps}
    {@const f = PS_FACE[target]}
    <circle cx="50" cy="50" r="44" fill="#2a2d33" stroke={f.color} stroke-width="5" />
    {#if f.symbol === "cross"}<path d="M33 33 L67 67 M67 33 L33 67" stroke={f.color} stroke-width="9" stroke-linecap="round" />
    {:else if f.symbol === "circle"}<circle cx="50" cy="50" r="18" fill="none" stroke={f.color} stroke-width="8" />
    {:else if f.symbol === "square"}<rect x="33" y="33" width="34" height="34" rx="2" fill="none" stroke={f.color} stroke-width="8" />
    {:else}<path d="M50 30 L70 64 H30 Z" fill="none" stroke={f.color} stroke-width="8" stroke-linejoin="round" />{/if}
  {:else if kind === "face"}
    <circle cx="50" cy="50" r="44" fill={FACE[target]} stroke="rgba(0,0,0,.28)" stroke-width="6" />
    <text x="50" y="51" text-anchor="middle" dominant-baseline="central" fill="#fff" font-size="50" font-weight="900">{target}</text>
  {:else if kind === "bumper"}
    <rect x="6" y="29" width="88" height="42" rx="21" fill={active ? ACCENT : FILL} stroke={NEUTRAL} stroke-width="5" />
    <text x="50" y="51" text-anchor="middle" dominant-baseline="central" fill="#fff" font-size="30" font-weight="800">{app.ps ? PS_SHORT[target] : target}</text>
  {:else if kind === "trigger"}
    <path d="M20 94 L20 34 Q20 6 50 6 Q80 6 80 34 L80 94 Z" fill={active ? ACCENT : FILL} stroke={NEUTRAL} stroke-width="5" stroke-linejoin="round" />
    <text x="50" y="66" text-anchor="middle" dominant-baseline="central" fill="#fff" font-size="30" font-weight="800">{app.ps ? PS_SHORT[target] : target}</text>
  {:else if kind === "stickdir"}
    <circle cx="50" cy="50" r="44" fill={FILL} stroke={NEUTRAL} stroke-width="5" />
    <circle cx="50" cy="50" r="30" fill={active ? ACCENT : "#4a5059"} />
    <g transform={`rotate(${ANGLE[dir]} 50 50)`}>
      <polyline points="44,34 62,50 44,66" fill="none" stroke="#fff" stroke-width="9" stroke-linecap="round" stroke-linejoin="round" />
    </g>
    <circle cx="82" cy="82" r="17" fill="#fff" />
    <text x="82" y="83" text-anchor="middle" dominant-baseline="central" fill="#15181d" font-size="22" font-weight="900">{target[0]}</text>
  {:else if kind === "click"}
    <circle cx="50" cy="50" r="44" fill={FILL} stroke={NEUTRAL} stroke-width="5" />
    <circle cx="50" cy="50" r="30" fill={active ? ACCENT : "#4a5059"} />
    <text x="50" y="51" text-anchor="middle" dominant-baseline="central" fill="#fff" font-size="27" font-weight="900">{target[0]}3</text>
  {:else if kind === "dpad"}
    <path d="M38 6 h24 a4 4 0 0 1 4 4 v24 h24 a4 4 0 0 1 4 4 v24 a4 4 0 0 1 -4 4 h-24 v24 a4 4 0 0 1 -4 4 h-24 a4 4 0 0 1 -4 -4 v-24 h-24 a4 4 0 0 1 -4 -4 v-24 a4 4 0 0 1 4 -4 h24 v-24 a4 4 0 0 1 4 -4 z"
          fill={FILL} stroke={NEUTRAL} stroke-width="5" stroke-linejoin="round" />
    {@const off = { UP: [0, -1], DOWN: [0, 1], LEFT: [-1, 0], RIGHT: [1, 0] }[dir] ?? [0, 0]}
    <rect x={50 + off[0] * 28 - 12} y={50 + off[1] * 28 - 12} width="24" height="24" rx="4" fill={active ? ACCENT : "#fff"} />
  {:else}
    <circle cx="50" cy="50" r="44" fill={active ? ACCENT : FILL} stroke={NEUTRAL} stroke-width="5" />
    {#if app.ps && target === "BACK"}
      <text x="50" y="52" text-anchor="middle" dominant-baseline="central" fill="#fff" font-size="21" font-weight="800">SHARE</text>
    {:else if app.ps && target === "START"}
      <text x="50" y="52" text-anchor="middle" dominant-baseline="central" fill="#fff" font-size="18" font-weight="800">OPTIONS</text>
    {:else if app.ps && target === "GUIDE"}
      <text x="50" y="52" text-anchor="middle" dominant-baseline="central" fill="#fff" font-size="34" font-weight="900">PS</text>
    {:else if target === "BACK"}
      <rect x="28" y="30" width="30" height="26" rx="4" fill="none" stroke="#fff" stroke-width="6" />
      <rect x="42" y="44" width="30" height="26" rx="4" fill={active ? ACCENT : FILL} stroke="#fff" stroke-width="6" />
    {:else if target === "START"}
      <path d="M30 36 H70 M30 50 H70 M30 64 H70" stroke="#fff" stroke-width="7" stroke-linecap="round" />
    {:else}
      <circle cx="50" cy="50" r="17" fill="none" stroke="#fff" stroke-width="7" />
    {/if}
  {/if}
</svg>

<style>
  svg { display: block; flex: none; }
  text { font-family: var(--font); }
</style>
