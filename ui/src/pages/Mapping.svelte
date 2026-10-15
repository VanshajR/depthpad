<script lang="ts">
  import { ask } from "@tauri-apps/plugin-dialog";
  import { api, curveOf, CURVE_KEYS, type Binding, type CurveKey } from "../lib/api";
  import CurvePlot from "../lib/CurvePlot.svelte";
  import Icon from "../lib/Icon.svelte";
  import Keyboard3D from "../lib/Keyboard3D.svelte";
  import { ANALOG, TARGET_LONG } from "../lib/layout";
  import Segmented from "../lib/Segmented.svelte";
  import SliderField from "../lib/SliderField.svelte";
  import { app } from "../lib/state.svelte";
  import TargetPicker from "../lib/TargetPicker.svelte";

  let selected = $derived(app.mapKey);
  let chipHover = $state<string | null>(null);
  let keyHover = $state<number | null>(null);

  let name = $derived(selected !== null ? app.keyByPos.get(selected)?.key ?? null : null);
  let binding = $derived(name ? app.profile?.bindings.find((b) => b.key === name) : undefined);
  let target = $derived(binding?.target ?? null);
  let custom = $derived(!!binding && CURVE_KEYS.some((k) => binding![k] !== undefined));
  let curve = $derived(app.profile ? curveOf(app.profile, binding) : null);
  let analog = $derived(!!target && ANALOG.has(target));
  let canMap = $derived(selected !== null && app.mappable(selected));
  let live = $derived(selected !== null ? (app.depths.get(selected)?.depth ?? 0) * app.travel : 0);

  function setTarget(pos: number, t: string | null, from: number | null = null) {
    const key = app.keyByPos.get(pos)?.key;
    if (!key) return;
    app.edit((p) => {
      let carried: Partial<Binding> = {};
      const own = p.bindings.find((b) => b.key === key);
      if (own) for (const k of CURVE_KEYS) if (own[k] !== undefined) carried[k] = own[k];
      if (from !== null) {
        const fromKey = app.keyByPos.get(from)?.key;
        const moved = p.bindings.find((b) => b.key === fromKey);
        carried = {};
        if (moved) for (const k of CURVE_KEYS) if (moved[k] !== undefined) carried[k] = moved[k];
        p.bindings = p.bindings.filter((b) => b.key !== fromKey);
      }
      p.bindings = p.bindings.filter((b) => b.key !== key);
      if (t) p.bindings.push({ key, target: t, ...carried });
    });
    app.mapKey = pos;
  }

  function clearKey(pos: number) {
    const key = app.keyByPos.get(pos)?.key;
    const before = app.profile?.bindings.find((b) => b.key === key);
    if (!key || !before) return;
    const saved = $state.snapshot(before) as Binding;
    app.edit((p) => (p.bindings = p.bindings.filter((b) => b.key !== key)));
    app.mapKey = pos;
    app.toast(`${app.legend(pos)} no longer controls ${TARGET_LONG[saved.target]}.`, {
      action: { label: "Undo", run: () => app.edit((p) => { p.bindings = p.bindings.filter((b) => b.key !== key); p.bindings.push(saved); }) },
    });
  }

  function onWindowKey(e: KeyboardEvent) {
    const el = e.target as HTMLElement | null;
    if (el && (el.tagName === "INPUT" || el.closest("[role=button]"))) return;
    if (e.key === "Delete" && selected !== null && target) { e.preventDefault(); clearKey(selected); }
    if (e.key === "Escape") app.mapKey = null;
  }

  function setCustom(on: boolean) {
    if (!name) return;
    app.edit((p) => {
      const b = p.bindings.find((x) => x.key === name);
      if (!b) return;
      for (const k of CURVE_KEYS) {
        if (on) b[k] = p.curve[k];
        else delete b[k];
      }
    });
  }

  function setCurve(k: CurveKey, v: number) {
    app.edit((p) => {
      const b = p.bindings.find((x) => x.key === name);
      if (!b) return;
      const c = curveOf(p, b);
      if (k === "deadzone_mm") v = Math.min(v, c.full_mm - 0.1);
      if (k === "full_mm") v = Math.max(v, c.deadzone_mm + 0.1);
      b[k] = Math.round(v * 100) / 100;
    });
  }

  function replaceBindings(next: Binding[], message: string) {
    const before = $state.snapshot(app.profile!.bindings) as Binding[];
    app.edit((p) => (p.bindings = next));
    app.toast(message, { action: { label: "Undo", run: () => app.edit((p) => (p.bindings = before)) } });
  }

  async function clearAll() {
    if (!app.profile?.bindings.length) return;
    if (await ask(`Remove every mapping from “${app.profile.name}”?`, { title: "Depthpad", kind: "warning" }))
      replaceBindings([], `All mappings removed from “${app.profile.name}”.`);
  }

  async function resetAll() {
    const base = await api.builtinLayout(app.profile!.name);
    if (await ask(`Replace the mappings in “${app.profile!.name}” with the ${base.name} layout?`, { title: "Depthpad", kind: "warning" }))
      replaceBindings(base.bindings, `Mappings reset to the ${base.name} layout.`);
  }

  let note = $derived.by(() => {
    if (!app.profile || !target) return "Select a mapped key to see its response.";
    if (custom) return "This key has its own response.";
    const d = app.profile.curve;
    return analog
      ? `Profile default: dead zone ${d.deadzone_mm.toFixed(2)} mm, full output at ${d.full_mm.toFixed(2)} mm. Change it in Settings, or choose Custom for this key.`
      : `Profile default: the button presses at ${d.press_mm.toFixed(2)} mm.`;
  });
</script>

<svelte:window onkeydown={onWindowKey} />

<div class="page">
  <div class="left">
    <section class="card board">
      <header class="card-head">
        <div>
          <h2 class="card-title">Key mapping</h2>
          <p class="card-sub">Select a key, or drag an input onto it. Right-click a mapped key to clear it.</p>
        </div>
        <span class="actions">
          <button class="btn ghost" onclick={resetAll}><Icon name="rotate-ccw" size={15} />Reset to defaults</button>
          <button class="btn ghost" onclick={clearAll}><Icon name="trash-2" size={15} />Clear all</button>
        </span>
      </header>
      <Keyboard3D depths={app.depths} bindings={app.bindings} tilt={0} maxWidth="none" mappable={app.mappable} selectable {selected}
                  highlight={chipHover} onselect={(p) => (app.mapKey = p)} onhoverkey={(p) => (keyHover = p)}
                  ondropkey={(pos, t, from) => setTarget(pos, t, from)} onclearkey={clearKey} />
    </section>
    <section class="card response">
      <header class="card-head">
        <div>
          <h2 class="card-title">Response</h2>
          <p class="card-sub">{note}</p>
        </div>
        {#if target}
          <span class="live tabular"><span class="dot" class:on={live > 0.01}></span>{live.toFixed(2)} mm</span>
        {/if}
      </header>
      <div class="plot"><CurvePlot {curve} press={!!target && !analog} liveMm={live} /></div>
    </section>
  </div>

  <aside class="card inspector">
    <div class="who">
      <span class="keycap" class:empty={selected === null}>{selected !== null ? app.legend(selected) : "?"}</span>
      <div>
        <h2 class="card-title">{selected !== null ? app.legend(selected) : "No key selected"}</h2>
        <p class="card-sub">
          {#if selected === null}Click a key on the keyboard to map it.
          {:else if !canMap}Fn works inside the keyboard and can't be mapped.
          {:else if target}{TARGET_LONG[target]}
          {:else}Not mapped yet{/if}
        </p>
      </div>
      {#if target && selected !== null}
        <button class="btn ghost remove" title="Clear this key's mapping (Delete)" onclick={() => clearKey(selected!)}>
          <Icon name="trash-2" size={15} />Clear
        </button>
      {/if}
    </div>
    <div class="sep"></div>
    <TargetPicker value={target} disabled={!canMap} hint={keyHover !== null ? app.bindings.get(keyHover) ?? null : null}
                  onchoose={(t) => selected !== null && setTarget(selected, t)} onhover={(t) => (chipHover = t)} />
    {#if target}
      <div class="sep"></div>
      <div class="resp-head">
        <span class="label">Response</span>
        <Segmented options={["Profile default", "Custom"]} value={custom ? 1 : 0} onchange={(i) => setCustom(i === 1)} />
      </div>
      {#if custom && curve}
        {#if analog}
          <SliderField label="Dead zone" value={curve.deadzone_mm} min={0} max={1.5} onchange={(v) => setCurve("deadzone_mm", v)} />
          <SliderField label="Full output at" value={curve.full_mm} min={0.5} max={app.travel} onchange={(v) => setCurve("full_mm", v)} />
          <SliderField label="Response shape" value={curve.shape} min={0.3} max={3} unit="" onchange={(v) => setCurve("shape", v)} />
        {:else}
          <SliderField label="Press point" value={curve.press_mm} min={0.2} max={3.2} onchange={(v) => setCurve("press_mm", v)} />
        {/if}
      {/if}
    {/if}
  </aside>
</div>

<style>
  .page { display: grid; grid-template-columns: minmax(0, 1fr) 360px; gap: 16px; max-width: 1320px; height: 100%; min-height: 560px; margin: 0 auto; padding-top: 4px; }
  .left { display: flex; flex-direction: column; gap: 16px; min-width: 0; min-height: 0; }
  .card { padding: 16px 20px 18px; }
  .card-head { margin-bottom: 14px; }
  .actions { display: flex; gap: 6px; flex: none; }
  .btn.ghost { display: inline-flex; align-items: center; gap: 7px; height: 30px; padding: 0 10px; border-radius: 7px;
               background: transparent; border-color: transparent; color: var(--ink-2); font-size: var(--fs-label); }
  .btn.ghost:hover { background: var(--raised); color: var(--ink); }
  .response { flex: 1; min-height: 0; display: flex; flex-direction: column; }
  .plot { flex: 1; min-height: 120px; }
  .plot :global(svg) { width: 100%; height: 100%; display: block; }
  .live { display: inline-flex; align-items: center; gap: 7px; flex: none; color: var(--ink-2); font-size: var(--fs-label); }
  .dot { width: 6px; height: 6px; border-radius: 50%; background: var(--ink-3); }
  .dot.on { background: var(--accent); }

  .inspector { display: flex; flex-direction: column; gap: 14px; align-self: start; position: sticky; top: 0; }
  .who { display: flex; align-items: center; gap: 14px; }
  .who > div { flex: 1; min-width: 0; }
  .remove:hover { color: var(--error); background: rgba(239, 83, 80, 0.1); }
  .keycap { min-width: 44px; height: 42px; padding: 0 10px; display: grid; place-items: center; border-radius: 7px; flex: none;
            font: 600 13px/1 var(--font); color: #e8eaee;
            background: linear-gradient(180deg, #3a3d43 0%, #2a2d32 100%); box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.1), 0 3px 0 #111215; }
  .keycap.empty { color: #8a8f98; }
  .sep { height: 1px; background: var(--stroke-soft); }
  .resp-head { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  .label { color: var(--ink-2); font-size: var(--fs-label); font-weight: 600; }
</style>
