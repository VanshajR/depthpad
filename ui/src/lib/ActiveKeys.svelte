<!-- Keys that are down right now: exact depth, whether it was measured or held, and what it drives. -->
<script lang="ts">
  import { applyCurve, curveOf } from "./api";
  import Glyph from "./Glyph.svelte";
  import { ANALOG, TARGET_LONG } from "./layout";
  import { app } from "./state.svelte";

  const MAX_ROWS = 6;

  let rows = $derived.by(() => {
    const p = app.profile;
    return (app.snapshot?.keys ?? [])
      .filter((k) => k.depth > 0.005)
      .sort((a, b) => a.pos - b.pos)
      .slice(0, MAX_ROWS)
      .map((k) => {
        const target = app.bindings.get(k.pos) ?? null;
        const key = app.keyByPos.get(k.pos)?.key;
        const binding = p?.bindings.find((b) => b.key === key);
        const curve = p ? curveOf(p, binding) : null;
        const mm = Math.min(app.travel, k.depth * app.travel);
        let out = "";
        if (target && curve) {
          out = ANALOG.has(target) ? `${Math.round(applyCurve(Math.min(1, k.depth), curve, app.travel) * 100)}%`
                                   : mm >= curve.press_mm ? "On" : "Off";
        }
        return { pos: k.pos, legend: app.legend(k.pos), mm, depth: Math.min(1, k.depth), held: k.source !== "stream", target, out };
      });
  });
</script>

<section class="side">
  <h3>Keys down</h3>
  {#if rows.length}
    <ul>
      {#each rows as r (r.pos)}
        <li>
          <span class="cap">{r.legend}</span>
          <span class="depth">
            <span class="mm tabular">{r.held ? "Held" : `${r.mm.toFixed(2)} mm`}</span>
            <span class="track"><span class="fill" class:held={r.held} style:width="{r.depth * 100}%"></span></span>
          </span>
          {#if r.target}
            <span class="to" title={TARGET_LONG[r.target]}><Glyph target={r.target} size={18} active /><b class="tabular">{r.out}</b></span>
          {:else}
            <span class="to none">—</span>
          {/if}
        </li>
      {/each}
    </ul>
  {:else}
    <p class="empty">Press a key to see its depth and what it controls.</p>
  {/if}
</section>

<style>
  .side { display: flex; flex-direction: column; min-width: 0; }
  h3 { margin: 0 0 10px; color: var(--ink-3); font-size: var(--fs-caption); font-weight: 600; }
  ul { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 6px; }
  li { display: grid; grid-template-columns: 40px minmax(0, 1fr) 64px; align-items: center; gap: 10px;
       height: 40px; padding: 0 8px 0 6px; border-radius: 7px; background: var(--surface); border: 1px solid var(--stroke-soft); }
  .cap { height: 26px; display: grid; place-items: center; border-radius: 5px; font-size: 11px; font-weight: 600;
         background: linear-gradient(180deg, #3a3d43, #2a2d32); box-shadow: 0 2px 0 #111215; color: #e8eaee; white-space: nowrap; overflow: hidden; }
  .depth { display: flex; flex-direction: column; gap: 5px; min-width: 0; }
  .mm { font-size: var(--fs-label); font-weight: 600; }
  .track { height: 3px; border-radius: 2px; background: var(--raised); overflow: hidden; }
  .fill { display: block; height: 100%; background: var(--accent); }
  .fill.held { background: var(--held); }
  .to { display: flex; align-items: center; justify-content: flex-end; gap: 6px; }
  .to b { font-size: var(--fs-label); min-width: 30px; text-align: right; }
  .none { color: var(--ink-3); }
  .empty { margin: 0; color: var(--ink-3); font-size: var(--fs-caption); line-height: 1.5; }
</style>
