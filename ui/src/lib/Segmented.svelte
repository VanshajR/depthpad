<script lang="ts">
  let { options, value = $bindable(0), onchange }: { options: string[]; value?: number; onchange?: (i: number) => void } = $props();
</script>

<div class="seg" role="tablist" style:--n={options.length} style:--i={value}>
  <span class="thumb"></span>
  {#each options as o, i}
    <button role="tab" onmousedown={(e) => e.preventDefault()} aria-selected={i === value} class:on={i === value}
            onclick={() => { if (i !== value) { value = i; onchange?.(i); } }}>{o}</button>
  {/each}
</div>

<style>
  .seg { position: relative; flex: none; display: inline-grid; grid-template-columns: repeat(var(--n), 1fr); padding: 3px;
         background: var(--bg); border: 1px solid var(--stroke-soft); border-radius: 7px; height: 30px; }
  .thumb { position: absolute; top: 3px; bottom: 3px; left: 3px; width: calc((100% - 6px) / var(--n));
           transform: translateX(calc(var(--i) * 100%)); background: var(--hover); border-radius: 5px;
           transition: transform 180ms var(--ease); }
  button { position: relative; background: none; border: 0; padding: 0 12px; cursor: pointer; white-space: nowrap;
           color: var(--ink-2); font-size: var(--fs-label); font-weight: 600; transition: color 120ms; }
  button.on { color: var(--ink); }
  button:hover:not(.on) { color: var(--ink); }
</style>
