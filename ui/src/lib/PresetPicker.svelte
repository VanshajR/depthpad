<!-- Curve presets as small pictures of their shapes (Wootility's set). -->
<script lang="ts">
  import { applyCurve, presetCurve, PRESETS, type Curve, type Preset } from "./api";

  let { curve, active = null, onpick }: { curve: Curve; active?: Preset | null; onpick: (p: Preset) => void } = $props();

  const LABEL: Record<Preset, string> = {
    linear: "Linear", aggro: "Aggro", slow: "Slow", smooth: "Smooth", steep: "Steep", instant: "Instant", turbo: "Turbo",
  };
  const TITLE: Record<Preset, string> = {
    linear: "Output rises evenly with key travel.",
    aggro: "Lots of output early, then levels off.",
    slow: "Little output early, for fine control.",
    smooth: "Gentle at both ends, quick in the middle.",
    steep: "Almost off, then almost full, around mid-travel.",
    instant: "Full output as soon as a key passes the dead zone; key depth still sets a stick's direction.",
    turbo: "On or off past the dead zone; sticks snap to 8 directions.",
  };

  // a thumbnail of each preset over a normalised 0..1 travel
  function thumb(p: Preset): string {
    const c = presetCurve({ ...curve, deadzone_mm: 0.15, full_mm: 0.85, anti_deadzone: 0 }, p);
    return Array.from({ length: 41 }, (_, i) => {
      const t = i / 40;
      return `${4 + t * 40},${36 - applyCurve(t, c, 1) * 32}`;
    }).join(" ");
  }
</script>

<div class="presets" role="group" aria-label="Curve presets">
  {#each PRESETS as p}
    <button class="preset" class:on={active === p} title={TITLE[p]} onclick={() => onpick(p)}>
      <svg viewBox="0 0 48 40" aria-hidden="true"><polyline points={thumb(p)} /></svg>
      <span>{LABEL[p]}</span>
    </button>
  {/each}
</div>

<style>
  .presets { display: grid; grid-template-columns: repeat(7, minmax(0, 1fr)); gap: 6px; }
  .preset { display: flex; flex-direction: column; align-items: center; gap: 4px; padding: 8px 4px 7px; border-radius: 8px; cursor: pointer;
            background: var(--bg); border: 1px solid var(--stroke-soft); color: var(--ink-2); font-size: 11.5px; font-weight: 600;
            transition: border-color 120ms, background 120ms, color 120ms; }
  .preset:hover { border-color: var(--stroke); color: var(--ink); }
  .preset.on { border-color: var(--accent); background: var(--accent-wash); color: var(--ink); }
  svg { width: 100%; max-width: 52px; height: 34px; }
  polyline { fill: none; stroke: currentColor; stroke-width: 2.2; stroke-linejoin: round; stroke-linecap: round; }
  .on polyline { stroke: var(--accent); }
</style>
