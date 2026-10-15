<!-- The keyboard as a physical object: a case under a spotlight whose keys sink as you press them
     (the app's whole subject is key depth) with an accent underglow, amber when the depth is held
     from the key press rather than measured. Mapped keys carry their input's glyph.
     With `selectable` it is also the mapping surface: click a key to select it, drop an input on a
     key to map it, drag a mapped key onto another key to move the mapping, or right-click (or
     Delete) a mapped key to clear it. -->
<script lang="ts">
  import type { Source } from "./api";
  import { drag, endDrag, startDrag } from "./dnd.svelte";
  import Glyph from "./Glyph.svelte";
  import { app } from "./state.svelte";

  let {
    depths, bindings, tilt = 22, maxWidth = "min(800px, calc((100vh - 470px) * 2.4))",
    mappable = () => true, selectable = false, selected = null, highlight = null,
    onselect, ondropkey, onhoverkey, onclearkey,
  }: {
    depths: Map<number, { depth: number; source: Source }>;
    bindings: Map<number, string>;
    tilt?: number;
    maxWidth?: string;
    mappable?: (pos: number) => boolean;
    selectable?: boolean;
    selected?: number | null;
    highlight?: string | null;
    onselect?: (pos: number) => void;
    ondropkey?: (pos: number, target: string, from: number | null) => void;
    onhoverkey?: (pos: number | null) => void;
    onclearkey?: (pos: number) => void;
  } = $props();

  let units = $derived(app.device?.width_units ?? 16);
  let height = $derived(Math.max(1, app.device?.height_units ?? 5));
  let keys = $derived((app.device?.layout ?? []).map((k) => ({
    pos: k.pos, legend: k.legend, left: k.x / units, top: k.y / height, width: k.w / units, height: k.h / height,
    // an ISO Enter: the L shape, with the legend in the wide top part
    clip: k.notch
      ? `polygon(0 0, 100% 0, 100% 100%, ${(k.notch[0] / k.w) * 100}% 100%, ${(k.notch[0] / k.w) * 100}% ${(k.notch[1] / k.h) * 100}%, 0 ${(k.notch[1] / k.h) * 100}%)`
      : undefined,
    legendTop: k.notch ? (k.notch[1] / k.h) * 100 : undefined,
  })));

  let dropPos = $state<number | null>(null);
  let popPos = $state<number | null>(null);

  function drop(e: DragEvent, pos: number) {
    e.preventDefault();
    const { target, from } = drag;
    dropPos = null;
    endDrag();
    if (target && mappable(pos) && from !== pos) {
      ondropkey?.(pos, target, from);
      popPos = pos;
      setTimeout(() => { if (popPos === pos) popPos = null; }, 450);
    }
  }
</script>

<div class="stage" style:--tilt="{tilt}deg" style:max-width={maxWidth} class:flat={tilt === 0}
     style:--units={units} style:--rows={height}>
  <div class="case">
    <div class="plate">
      {#each keys as k (k.pos)}
        {@const d = depths.get(k.pos)}
        {@const depth = Math.min(1, d?.depth ?? 0)}
        {@const preview = drag.target !== null && dropPos === k.pos}
        {@const bound = preview ? drag.target : bindings.get(k.pos)}
        <div class="slot" style:left="{k.left * 100}%" style:top="{k.top * 100}%" style:width="{k.width * 100}%"
             style:height="{k.height * 100}%" style:clip-path={k.clip}
             style:--d={depth} class:held={!!d && d.source !== "stream"} class:on={depth > 0.02}>
          {#if depth > 0.02}<div class="glow"></div>{/if}
          <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
          <div class="cap"
               class:selectable
               class:selected={selected === k.pos}
               class:lit={!!bound && highlight === bound}
               class:preview
               class:pop={popPos === k.pos}
               class:muted={drag.target !== null && !mappable(k.pos)}
               role={selectable ? "button" : undefined}
               tabindex={selectable ? 0 : undefined}
               aria-label={selectable ? k.legend || "Space" : undefined}
               aria-pressed={selectable ? selected === k.pos : undefined}
               draggable={selectable && !!bindings.get(k.pos)}
               onclick={() => selectable && onselect?.(k.pos)}
               onkeydown={(e) => {
                 if (!selectable) return;
                 if (e.key === "Enter" || e.key === " ") { e.preventDefault(); onselect?.(k.pos); }
                 if ((e.key === "Delete" || e.key === "Backspace") && bindings.get(k.pos)) { e.preventDefault(); onclearkey?.(k.pos); }
               }}
               oncontextmenu={(e) => { if (selectable && bindings.get(k.pos)) { e.preventDefault(); onclearkey?.(k.pos); } }}
               onmouseenter={() => onhoverkey?.(k.pos)}
               onmouseleave={() => onhoverkey?.(null)}
               ondragstart={(e) => startDrag(e, bindings.get(k.pos)!, k.pos, (e.currentTarget as Element).querySelector(".glyph"))}
               ondragend={() => { endDrag(); dropPos = null; }}
               ondragover={(e) => { if (drag.target && selectable && mappable(k.pos)) { e.preventDefault(); dropPos = k.pos; } }}
               ondragleave={() => { if (dropPos === k.pos) dropPos = null; }}
               ondrop={(e) => drop(e, k.pos)}>
            <span class="legend" class:long={k.legend.length > 2}
                  style:position={k.legendTop ? "absolute" : undefined} style:top={k.legendTop ? `${k.legendTop / 2}%` : undefined}
                  style:translate={k.legendTop ? "0 -50%" : undefined}>{k.legend}</span>
            {#if bound}<span class="glyph" class:ghost={preview}><Glyph target={bound} size={40} active={depth > 0.45} /></span>{/if}
          </div>
        </div>
      {/each}
    </div>
  </div>
</div>

<style>
  .stage { position: relative; width: 100%; margin: 0 auto; perspective: 1700px;
           container-type: inline-size; padding: 1cqw 0 3cqw; }
  .stage.flat { padding-bottom: 1.5cqw; }
  .case {
    position: relative; transform: rotateX(var(--tilt)); transform-origin: 50% 70%; transform-style: preserve-3d;
    padding: 2.2cqw; border-radius: 2.4cqw;
    background: linear-gradient(180deg, #24262b 0%, #17181c 100%);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.08),
      inset 0 -2px 0 rgba(0, 0, 0, 0.5),
      0 1.4cqw 0 #0d0e10,                       /* case thickness */
      0 3cqw 6cqw rgba(0, 0, 0, 0.65);           /* drop shadow */
  }
  .flat .case { box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.08), inset 0 -2px 0 rgba(0, 0, 0, 0.5),
                            0 0.6cqw 0 #0d0e10, 0 2cqw 4cqw rgba(0, 0, 0, 0.5); }
  .plate { position: relative; aspect-ratio: var(--units) / var(--rows); border-radius: 1.2cqw; background: #0c0d0f;
           box-shadow: inset 0 0.4cqw 1.2cqw rgba(0, 0, 0, 0.9); }
  .slot { position: absolute; padding: 0.22cqw; }

  /* light from the switch underneath, stronger the deeper the key goes */
  .glow { position: absolute; inset: 8%; border-radius: 0.6cqw; opacity: calc(var(--d) * 1.2);
          background: color-mix(in srgb, var(--accent) 90%, transparent); filter: blur(1cqw); transition: opacity 60ms linear; pointer-events: none; }
  .held .glow { background: rgba(240, 163, 58, 0.9); }

  .cap {
    --ring: 0 0 0 0 transparent;
    position: relative; height: 100%; border-radius: 0.6cqw; outline: none;
    display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 0.2cqw;
    background: linear-gradient(180deg, #3a3d43 0%, #2c2f34 55%, #26282c 100%);
    /* the keycap skirt: thick at rest, shrinking as the key travels down */
    box-shadow:
      var(--ring),
      inset 0 1px 0 rgba(255, 255, 255, 0.12),
      0 calc((1 - var(--d)) * 0.55cqw + 0.12cqw) 0 #141518,
      0 calc((1 - var(--d)) * 0.55cqw + 0.4cqw) 0.6cqw rgba(0, 0, 0, 0.55);
    transform: translateY(calc(var(--d) * 0.5cqw));
    transition: transform 60ms linear, box-shadow 120ms var(--ease), background 120ms, opacity 120ms;
  }
  .on .cap { background: linear-gradient(180deg, color-mix(in srgb, var(--accent) 42%, #3a3d43) 0%, color-mix(in srgb, var(--accent) 34%, #26282c) 60%, color-mix(in srgb, var(--accent) 28%, #1e2023) 100%); }
  .held.on .cap { background: linear-gradient(180deg, #8c6a3a 0%, #6b4f2a 60%, #594223 100%); }

  .selectable { cursor: pointer; }
  .selectable:hover { background: linear-gradient(180deg, #44474e 0%, #34373d 55%, #2d3034 100%); }
  .selectable:focus-visible, .lit { --ring: 0 0 0 1.5px var(--accent-ink); }
  .selected { --ring: 0 0 0 2px var(--accent); }
  .preview { --ring: 0 0 0 2px var(--accent), 0 0 0 0.5cqw var(--accent-wash);
             background: linear-gradient(180deg, #44474e 0%, #34373d 100%); }
  .muted { opacity: 0.3; }
  .pop { animation: pop 420ms var(--ease); }
  @keyframes pop {
    0% { scale: 1.12; }
    100% { scale: 1; }
  }

  .legend { font: 500 1.35cqw/1 var(--font); color: #c9ccd2; text-shadow: 0 1px 0 rgba(0, 0, 0, 0.6); pointer-events: none; }
  .legend.long { font-size: 1.05cqw; }
  .on .legend, .selected .legend { color: #fff; }
  .glyph { width: 2.2cqw; height: 2.2cqw; pointer-events: none; }
  .glyph :global(svg) { width: 100%; height: 100%; }
  .glyph.ghost { opacity: 0.6; }
</style>
