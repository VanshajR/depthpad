<!-- Slider plus an editable value with its unit. `inline` puts it on one line (settings rows). -->
<script lang="ts">
  let { label = "", value, min, max, step = 0.05, unit = "mm", digits = 2, inline = false, onchange }:
    { label?: string; value: number; min: number; max: number; step?: number; unit?: string; digits?: number; inline?: boolean;
      onchange: (v: number) => void } = $props();

  let pct = $derived(((value - min) / (max - min)) * 100);
  const clamp = (v: number) => Math.round(Math.min(max, Math.max(min, v)) / step) * step;
</script>

<div class="field" class:inline>
  {#if label}<span class="label">{label}</span>{/if}
  <input type="range" {min} {max} {step} {value} style:--pct="{pct}%" aria-label={label}
         oninput={(e) => onchange(clamp(+e.currentTarget.value))} />
  <span class="value">
    <input type="number" {min} {max} {step} value={value.toFixed(digits)} aria-label="{label} value"
           onchange={(e) => onchange(clamp(+e.currentTarget.value))} />
    <span class="unit">{unit}</span>
  </span>
</div>

<style>
  .field { display: grid; grid-template-columns: 1fr auto; grid-template-areas: "label value" "range range";
           align-items: center; gap: 6px 12px; }
  .field.inline { grid-template-columns: 180px auto; grid-template-areas: "range value"; }
  .label { grid-area: label; color: var(--ink-2); font-size: var(--fs-label); }
  input[type="range"] { grid-area: range; width: 100%; height: 22px; margin: 0; background: transparent; -webkit-appearance: none; appearance: none; cursor: pointer; }
  input[type="range"]::-webkit-slider-runnable-track { height: 4px; border-radius: 2px;
    background: linear-gradient(to right, var(--accent) var(--pct), var(--pressed) var(--pct)); }
  input[type="range"]::-webkit-slider-thumb { -webkit-appearance: none; width: 16px; height: 16px; margin-top: -6px;
    border-radius: 50%; background: var(--ink); border: 0; transition: transform 120ms var(--ease); }
  input[type="range"]:hover::-webkit-slider-thumb { transform: scale(1.12); background: #fff; }
  .value { grid-area: value; display: flex; align-items: center; gap: 6px; }
  input[type="number"] { width: 64px; height: 30px; padding: 0 8px; text-align: right; border-radius: 7px;
    background: var(--bg); border: 1px solid var(--stroke); color: var(--ink); font-weight: 600;
    font-variant-numeric: tabular-nums; -moz-appearance: textfield; appearance: textfield; }
  input[type="number"]::-webkit-inner-spin-button { display: none; }
  input[type="number"]:focus { outline: none; border-color: var(--accent); }
  .unit { color: var(--ink-3); font-size: var(--fs-caption); width: 20px; }
</style>
