<!-- Controller inputs as glyph chips: click to map the selected key, or drag onto any key. -->
<script lang="ts">
  import { endDrag, startDrag } from "./dnd.svelte";
  import Glyph from "./Glyph.svelte";
  import { TARGET_GROUPS, TARGET_LONG } from "./layout";

  let { value, hint = null, disabled = false, onchoose, onhover }:
    { value: string | null; hint?: string | null; disabled?: boolean; onchoose: (t: string | null) => void;
      onhover?: (t: string | null) => void } = $props();
</script>

<div class="picker">
  {#each TARGET_GROUPS as [title, targets]}
    <span class="cap tertiary">{title}</span>
    <div class="chips">
      {#each targets as t}
        <button class="chip" class:on={value === t} class:hint={hint === t} class:dim={disabled}
                title="{TARGET_LONG[t]} — click to map the selected key, or drag onto any key"
                aria-label={TARGET_LONG[t]} aria-pressed={value === t} draggable="true"
                onclick={() => !disabled && onchoose(t)}
                onmouseenter={() => onhover?.(t)} onmouseleave={() => onhover?.(null)}
                ondragstart={(e) => startDrag(e, t, null, e.currentTarget.querySelector("svg"))}
                ondragend={endDrag}>
          <Glyph target={t} size={24} />
        </button>
      {/each}
    </div>
  {/each}
  <span class="cap tertiary">None</span>
  <div class="chips">
    <button class="chip wide" class:on={value === null && !disabled} class:dim={disabled}
            onclick={() => !disabled && onchoose(null)}>Not mapped</button>
  </div>
</div>

<style>
  .picker { display: grid; grid-template-columns: 78px 1fr; align-items: center; gap: 8px 10px; }
  .cap { font-size: var(--fs-caption); }
  .chips { display: grid; grid-template-columns: repeat(4, 1fr); gap: 6px; }
  .chip { height: 36px; display: flex; align-items: center; justify-content: center; cursor: grab;
          background: var(--raised); border: 1px solid var(--stroke-soft); border-radius: 7px;
          color: var(--ink-2); font-size: var(--fs-label); font-weight: 600;
          transition: background 120ms var(--ease), border-color 120ms var(--ease), transform 120ms var(--ease), box-shadow 120ms; }
  .chip:hover { background: var(--hover); color: var(--ink); }
  .chip:active { transform: scale(0.96); cursor: grabbing; }
  .chip.on { background: var(--accent-wash); border-color: var(--accent); color: var(--ink); }
  .chip.hint { border-color: var(--accent-ink); }
  .chip.dim { opacity: 0.45; }
  .chip.wide { grid-column: span 2; cursor: pointer; }
</style>
