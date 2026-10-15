<script lang="ts">
  let { checked = $bindable(false), onchange, label }: { checked?: boolean; onchange?: (v: boolean) => void; label: string } = $props();
</script>

<button class="toggle" class:on={checked} role="switch" aria-checked={checked} aria-label={label}
        onclick={() => { checked = !checked; onchange?.(checked); }}><span></span></button>

<style>
  .toggle { width: 40px; height: 22px; border-radius: 11px; border: 0; padding: 0; cursor: pointer; flex: none;
            background: var(--pressed); position: relative; transition: background 150ms var(--ease); }
  .toggle.on { background: var(--accent); }
  span { position: absolute; top: 3px; left: 3px; width: 16px; height: 16px; border-radius: 50%; background: #fff;
         transition: transform 150ms var(--ease); }
  .on span { transform: translateX(18px); }
  .toggle:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
</style>
