<!-- The controller as a product illustration: a pad silhouette with material shading and a rim
     light; triggers and bumpers are physical parts behind the shoulders; sticks sit in recessed
     wells and their caps move and glow; buttons light in their colours. Everything is live.
     Laid out as an Xbox controller, or as a DualShock 4 (d-pad and face buttons up top, both
     sticks low in the middle, touchpad, PlayStation symbols) when that's the controller in use. -->
<script lang="ts">
  import type { Report } from "./api";
  import { BUTTON_BITS, FACE, PS_FACE } from "./layout";
  import { app } from "./state.svelte";

  let { out }: { out: Report } = $props();
  const on = (n: string) => (out.buttons & BUTTON_BITS[n]) !== 0;
  const clamp = (v: number, lo = -1, hi = 1) => Math.max(lo, Math.min(hi, v));

  // 1000 x 700, symmetric about x = 500. Body starts at y = 190.
  const BODY = `M500 190 C560 190 600 200 640 200 C700 200 740 180 790 182 C870 186 915 230 940 300
    C975 400 1000 530 975 600 C955 655 890 675 840 640 C800 612 770 560 730 530 C700 508 660 500 620 500
    L380 500 C340 500 300 508 270 530 C230 560 200 612 160 640 C110 675 45 655 25 600
    C0 530 25 400 60 300 C85 230 130 186 210 182 C260 180 300 200 360 200 C400 200 440 190 500 190 Z`;
  // Triggers (behind the shoulders) and bumpers (along the top edge).
  const TRIGGER = (x: number, flip: number) =>
    `M${x - 95 * flip} 175 C${x - 98 * flip} 120 ${x - 60 * flip} 78 ${x} 76 C${x + 60 * flip} 78 ${x + 98 * flip} 120 ${x + 95 * flip} 175 Z`;
  const BUMPER = (x: number, flip: number) =>
    `M${x - 120 * flip} 196 C${x - 100 * flip} 168 ${x - 40 * flip} 158 ${x + 10 * flip} 158 C${x + 60 * flip} 158 ${x + 100 * flip} 170 ${x + 118 * flip} 188`;

  const XBOX = { LS: { x: 300, y: 330 }, RS: { x: 615, y: 440 }, DP: { x: 385, y: 440 }, FC: { x: 700, y: 330 } };
  const DS4 = { LS: { x: 405, y: 450 }, RS: { x: 595, y: 450 }, DP: { x: 290, y: 320 }, FC: { x: 710, y: 320 } };
  let L = $derived(app.ps ? DS4 : XBOX);
  let LS = $derived(L.LS), RS = $derived(L.RS), DP = $derived(L.DP), FC = $derived(L.FC);
  let lt = $derived(clamp(out.lt, 0, 1));
  let rt = $derived(clamp(out.rt, 0, 1));
</script>

<div class="art" role="img" aria-label="Controller output">
<svg viewBox="50 100 900 570" aria-hidden="true">
  <defs>
    <linearGradient id="c-body" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#2d3036" /><stop offset="0.55" stop-color="#1f2125" /><stop offset="1" stop-color="#16171a" />
    </linearGradient>
    <linearGradient id="c-rim" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="rgba(255,255,255,0.32)" /><stop offset="0.35" stop-color="rgba(255,255,255,0.06)" /><stop offset="1" stop-color="rgba(255,255,255,0.02)" />
    </linearGradient>
    <linearGradient id="c-part" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#34373d" /><stop offset="1" stop-color="#1c1d21" />
    </linearGradient>
    <linearGradient id="c-lit" x1="0" y1="1" x2="0" y2="0">
      <stop offset="0" stop-color="var(--accent)" /><stop offset="1" stop-color="var(--accent-ink)" />
    </linearGradient>
    <radialGradient id="c-well" cx="0.5" cy="0.45" r="0.6">
      <stop offset="0" stop-color="#0c0d0f" /><stop offset="0.85" stop-color="#121316" /><stop offset="1" stop-color="#26282d" />
    </radialGradient>
    <radialGradient id="c-cap" cx="0.42" cy="0.35" r="0.75">
      <stop offset="0" stop-color="#4a4d54" /><stop offset="1" stop-color="#232529" />
    </radialGradient>
    <radialGradient id="c-capOn" cx="0.42" cy="0.35" r="0.75">
      <stop offset="0" stop-color="color-mix(in srgb, var(--accent) 45%, white)" /><stop offset="1" stop-color="color-mix(in srgb, var(--accent) 85%, black)" />
    </radialGradient>
    <radialGradient id="c-floor"><stop offset="0" stop-color="rgba(0,0,0,0.6)" /><stop offset="1" stop-color="rgba(0,0,0,0)" /></radialGradient>
    <filter id="c-inner" x="-10%" y="-10%" width="120%" height="120%">
      <feOffset dy="6" /><feGaussianBlur stdDeviation="8" result="b" />
      <feComposite in="SourceAlpha" in2="b" operator="out" result="edge" />
      <feFlood flood-color="#000" flood-opacity="0.55" /><feComposite in2="edge" operator="in" />
    </filter>
    {#each [{ id: "lt", x: 255, f: 1, v: lt }, { id: "rt", x: 745, f: -1, v: rt }] as t}
      <clipPath id="c-{t.id}"><path d={TRIGGER(t.x, t.f)} /></clipPath>
    {/each}
  </defs>

  <!-- floor shadow -->
  <ellipse cx="500" cy="660" rx="430" ry="34" fill="url(#c-floor)" />

  <!-- triggers: physical parts that fill with light from the bottom -->
  {#each [{ id: "lt", x: 255, f: 1, v: lt, label: "LT" }, { id: "rt", x: 745, f: -1, v: rt, label: "RT" }] as t}
    <path d={TRIGGER(t.x, t.f)} fill="url(#c-part)" stroke="rgba(255,255,255,0.14)" stroke-width="2" />
    {#if t.v > 0.005}
      <rect clip-path="url(#c-{t.id})" x={t.x - 110} y={175 - 100 * t.v} width="220" height={100 * t.v} fill="url(#c-lit)" opacity="0.9" />
      <rect clip-path="url(#c-{t.id})" x={t.x - 110} y={175 - 100 * t.v} width="220" height="3" fill="#fff" opacity="0.7" />
    {/if}
  {/each}

  <!-- bumpers along the top edge -->
  {#each [{ x: 230, f: 1, n: "LB" }, { x: 770, f: -1, n: "RB" }] as b}
    <path d={BUMPER(b.x, b.f)} stroke={on(b.n) ? "var(--accent)" : "#2b2d32"} stroke-width="16" stroke-linecap="round" fill="none" />
    <path d={BUMPER(b.x, b.f)} stroke="rgba(255,255,255,0.12)" stroke-width="2" stroke-linecap="round" fill="none" transform="translate(0 -6)" />
  {/each}

</svg>
<!-- static layer: drawn once, its filter never re-runs when the live parts change -->
<svg class="static" viewBox="50 100 900 570" aria-hidden="true">
  <!-- body: material, inner bevel, rim light -->
  <path d={BODY} fill="url(#c-body)" />
  <path d={BODY} fill="#000" filter="url(#c-inner)" opacity="0.9" />
  <path d={BODY} fill="none" stroke="url(#c-rim)" stroke-width="3" />
  <ellipse cx="500" cy="232" rx="250" ry="26" fill="rgba(255,255,255,0.035)" />

</svg>
<svg viewBox="50 100 900 570" aria-hidden="true">
  {#if app.ps}
    <!-- touchpad (pressing it is the Guide/PS input here too), Share, Options, PS button -->
    <rect x="395" y="212" width="210" height="104" rx="16" fill={on("GUIDE") ? "color-mix(in srgb, var(--accent) 35%, #1a1b1f)" : "#1a1b1f"}
          stroke="rgba(255,255,255,0.16)" stroke-width="2" />
    <rect x="395" y="212" width="210" height="6" rx="3" fill="var(--accent)" opacity="0.55" />
    <g class="sys" class:lit={on("BACK")}><rect class="pill" x="352" y="236" width="18" height="36" rx="9" /></g>
    <g class="sys" class:lit={on("START")}><rect class="pill" x="630" y="236" width="18" height="36" rx="9" /></g>
    <circle cx="500" cy="474" r="19" fill={on("GUIDE") ? "var(--accent)" : "#1a1b1f"} stroke="rgba(255,255,255,0.18)" stroke-width="2" />
    <text x="500" y="475" class="psb" fill={on("GUIDE") ? "#fff" : "rgba(255,255,255,0.6)"}>PS</text>
  {:else}
    <!-- guide, view, menu -->
    <circle cx="500" cy="262" r="30" fill={on("GUIDE") ? "var(--accent)" : "#1a1b1f"} stroke="rgba(255,255,255,0.18)" stroke-width="2" />
    <circle cx="500" cy="262" r="15" fill="none" stroke={on("GUIDE") ? "#fff" : "rgba(255,255,255,0.55)"} stroke-width="4" />
    <g class="sys" class:lit={on("BACK")}>
      <circle cx="438" cy="330" r="17" />
      <rect x="427" y="321" width="14" height="11" rx="2" fill="none" /><rect x="434" y="327" width="14" height="11" rx="2" />
    </g>
    <g class="sys" class:lit={on("START")}>
      <circle cx="562" cy="330" r="17" />
      <path d="M553 324 H571 M553 330 H571 M553 336 H571" />
    </g>
  {/if}

  <!-- sticks: recessed wells, textured caps that move and glow -->
  {#each [{ c: LS, x: out.lx, y: out.ly, click: "LS" }, { c: RS, x: out.rx, y: out.ry, click: "RS" }] as st}
    {@const x = clamp(st.x)}
    {@const y = clamp(st.y)}
    {@const active = Math.abs(x) > 0.01 || Math.abs(y) > 0.01}
    {@const cx = st.c.x + x * 26}
    {@const cy = st.c.y - y * 26}
    <circle cx={st.c.x} cy={st.c.y} r="70" fill="url(#c-well)" stroke="rgba(255,255,255,0.07)" stroke-width="2" />
    <circle cx={cx} cy={cy + 6} r="46" fill="rgba(0,0,0,0.5)" />
    <circle {cx} {cy} r="46" fill={active ? "url(#c-capOn)" : "url(#c-cap)"}
            stroke={on(st.click) ? "#cfe0ff" : "rgba(255,255,255,0.14)"} stroke-width={on(st.click) ? 4 : 1.5} />
    <circle {cx} {cy} r="33" fill="none" stroke={active ? "rgba(255,255,255,0.35)" : "rgba(0,0,0,0.35)"} stroke-width="2.5" />
    <circle {cx} {cy} r="22" fill="none" stroke={active ? "rgba(255,255,255,0.2)" : "rgba(255,255,255,0.05)"} stroke-width="2" />
  {/each}

  <!-- d-pad -->
  <circle cx={DP.x} cy={DP.y} r="58" fill="url(#c-well)" />
  <path d="M{DP.x - 15} {DP.y - 46} h30 a6 6 0 0 1 6 6 v25 h25 a6 6 0 0 1 6 6 v18 a6 6 0 0 1 -6 6 h-25 v25 a6 6 0 0 1 -6 6 h-30 a6 6 0 0 1 -6 -6 v-25 h-25 a6 6 0 0 1 -6 -6 v-18 a6 6 0 0 1 6 -6 h25 v-25 a6 6 0 0 1 6 -6 z"
        fill="url(#c-part)" stroke="rgba(255,255,255,0.14)" stroke-width="2" />
  {#each [["DPAD_UP", 0, -1], ["DPAD_DOWN", 0, 1], ["DPAD_LEFT", -1, 0], ["DPAD_RIGHT", 1, 0]] as [n, dx, dy]}
    {#if on(n as string)}
      <rect x={DP.x + +dx * 30 - 13} y={DP.y + +dy * 30 - 13} width="26" height="26" rx="5" fill="var(--accent)" />
    {/if}
  {/each}

  <!-- face buttons -->
  {#each [["Y", 0, -1], ["A", 0, 1], ["X", -1, 0], ["B", 1, 0]] as [n, dx, dy]}
    {@const fx = FC.x + +dx * 54}
    {@const fy = FC.y + +dy * 54}
    {@const col = app.ps ? PS_FACE[n as string].color : FACE[n as string]}
    {@const p = on(n as string)}
    <circle cx={fx} cy={fy + 4} r="27" fill="rgba(0,0,0,0.55)" />
    <circle cx={fx} cy={fy} r="27" fill={p ? col : "#1d1e22"} stroke={p ? "rgba(255,255,255,0.5)" : "rgba(255,255,255,0.12)"} stroke-width="1.5" />
    <path d="M{fx - 18} {fy - 12} A27 27 0 0 1 {fx + 18} {fy - 12}" stroke="rgba(255,255,255,0.18)" stroke-width="3" fill="none" stroke-linecap="round" />
    {#if app.ps}
      {@const f = PS_FACE[n as string]}
      {@const c = p ? "#fff" : f.color}
      {#if f.symbol === "cross"}<path d="M{fx - 10} {fy - 10} L{fx + 10} {fy + 10} M{fx + 10} {fy - 10} L{fx - 10} {fy + 10}" stroke={c} stroke-width="4.5" stroke-linecap="round" />
      {:else if f.symbol === "circle"}<circle cx={fx} cy={fy} r="11" fill="none" stroke={c} stroke-width="4.5" />
      {:else if f.symbol === "square"}<rect x={fx - 10} y={fy - 10} width="20" height="20" fill="none" stroke={c} stroke-width="4.5" />
      {:else}<path d="M{fx} {fy - 12} L{fx + 12} {fy + 9} H{fx - 12} Z" fill="none" stroke={c} stroke-width="4.5" stroke-linejoin="round" />{/if}
    {:else}
      <text x={fx} y={fy + 1} class="face" fill={p ? "#fff" : col}>{n}</text>
    {/if}
  {/each}
</svg>
</div>

<style>
  .art { position: relative; width: 100%; height: 100%; }
  svg { position: absolute; inset: 0; width: 100%; height: 100%; display: block; overflow: visible; }
  .static { will-change: transform; }
  text { font-family: var(--font); dominant-baseline: central; }
  .face { font-size: 26px; font-weight: 800; text-anchor: middle; }
  .sys circle { fill: #1a1b1f; stroke: rgba(255, 255, 255, 0.16); stroke-width: 2; }
  .sys rect, .sys path { fill: none; stroke: rgba(255, 255, 255, 0.6); stroke-width: 2.5; stroke-linecap: round; }
  .sys rect:last-child { fill: #1a1b1f; }
  .sys.lit circle { fill: var(--accent); stroke: var(--accent-ink); }
  .sys.lit rect, .sys.lit path { stroke: #fff; }
  .sys.lit rect:last-child { fill: var(--accent); }
  .sys .pill { fill: #1a1b1f; stroke: rgba(255, 255, 255, 0.4); stroke-width: 2; }
  .sys.lit .pill { fill: var(--accent); stroke: #fff; }
  .psb { font-size: 13px; font-weight: 900; text-anchor: middle; }
</style>
