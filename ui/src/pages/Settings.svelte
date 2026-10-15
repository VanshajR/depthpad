<script lang="ts">
  import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
  import { api, presetCurve, PRESETS, type CurveKey, type Preset, type RunningApp, type Settings, type Theme, ACCENTS, THEMES, applyAppearance } from "../lib/api";
  import CurveEditor from "../lib/CurveEditor.svelte";
  import PresetPicker from "../lib/PresetPicker.svelte";
  import HotkeyField from "../lib/HotkeyField.svelte";
  import Segmented from "../lib/Segmented.svelte";
  import SliderField from "../lib/SliderField.svelte";
  import { app } from "../lib/state.svelte";
  import SupportBadge from "../lib/SupportBadge.svelte";
  import Toggle from "../lib/Toggle.svelte";

  let autostart = $state(false);
  isEnabled().then((v) => (autostart = v)).catch(() => {});


  const set = <K extends keyof Settings>(k: K, v: Settings[K]) => app.edit((p) => (p.settings[k] = v));
  function setDefault(k: CurveKey, v: number) {
    app.edit((p) => {
      if (k === "deadzone_mm") v = Math.min(v, p.curve.full_mm - 0.1);
      if (k === "full_mm") v = Math.max(v, p.curve.deadzone_mm + 0.1);
      v = Math.round(v * 100) / 100;
      p.curve[k] = v;
      // a drawn curve's first and last points are the dead zone and full output: rescale it
      const pts = p.curve.points;
      if (pts && (k === "deadzone_mm" || k === "full_mm")) {
        const [a0, a1] = [pts[0][0], pts[3][0]];
        const [b0, b1] = [p.curve.deadzone_mm, p.curve.full_mm];
        p.curve.points = pts.map(([mm, out]) => [b0 + ((mm - a0) / Math.max(0.01, a1 - a0)) * (b1 - b0), out] as [number, number]);
      }
    });
  }

  function setPoints(points: [number, number][]) {
    app.edit((p) => {
      p.curve.points = points;
      p.curve.deadzone_mm = points[0][0];
      p.curve.full_mm = points[3][0];
    });
  }

  function pickPreset(name: Preset) {
    app.edit((p) => {
      const c = presetCurve(p.curve, name);
      p.curve.points = c.points;
      p.curve.mode = c.mode;
    });
  }

  // which preset the curve currently is, if any
  function presetOf(c: Settings extends never ? never : NonNullable<typeof app.profile>["curve"]): Preset | null {
    if (!c.points) return c.mode === "turbo" ? "turbo" : c.mode === "instant" ? "instant" : c.shape === 1 ? "linear" : null;
    const same = (a: [number, number][], b: [number, number][]) => a.every((p, i) => Math.abs(p[0] - b[i][0]) < 0.005 && Math.abs(p[1] - b[i][1]) < 0.005);
    return PRESETS.find((n) => { const q = presetCurve(c, n); return q.mode === (c.mode ?? "analog") && same(q.points!, c.points!); }) ?? null;
  }

  function setAppearance(theme: Theme, accent: string) {
    if (!app.prefs) return;
    app.prefs.theme = theme;
    app.prefs.accent = accent;
    applyAppearance(theme, accent);
    api.setAppearance(theme, accent);
  }

  // App linking
  let apps = $state<RunningApp[]>([]);
  async function loadApps() {
    apps = await api.runningApps().catch(() => []);
  }
  const short = (t: string) => (t.length > 34 ? t.slice(0, 33) + "…" : t);
  function setLink(exe: string, profile: string) {
    if (!app.prefs) return;
    if (profile) app.prefs.app_links[exe] = profile;
    else delete app.prefs.app_links[exe];
    api.setAppLink(exe, profile);
  }

  // the deepest key being pressed, shown on the curve
  let liveMm = $derived(Math.max(0, ...(app.snapshot?.keys ?? []).map((k) => Math.min(1, k.depth) * app.travel)));
  function setPref(k: "close_to_tray" | "start_minimized" | "check_updates", v: boolean) {
    if (app.prefs) app.prefs[k] = v;
    api.setPref(k, v);
  }
  async function setAutostart(v: boolean) {
    try { v ? await enable() : await disable(); } catch (e) { app.toast(String(e), { error: true }); }
    autostart = await isEnabled().catch(() => v);
  }

</script>

{#if app.profile}
  {@const s = app.profile.settings}
  {@const c = app.profile.curve}
  <div class="page">
    {#if app.keyboards}
      {@const kb = app.keyboards}
      <section class="sec">
        <header>
          <h2>Keyboard</h2>
          <p>Depthpad reads one keyboard at a time. Supported keyboards appear here when connected.</p>
        </header>
        <div class="card rows">
          {#each kb.list as k (k.id)}
            <div class="row">
              <div><span class="kname">{k.name} <SupportBadge status={k.status} /></span>
                <small>{k.vendor} · {k.connected ? "Connected" : "Not connected"}{kb.default === k.id ? " · Default" : ""}</small>
                {#if k.note}<small class="knote">{k.note}</small>{/if}</div>
              {#if k.id === kb.active}
                <span class="inuse">In use</span>
              {:else}
                <button class="btn" disabled={!k.connected} onclick={() => app.useKeyboard(k.id, false)}>Use</button>
              {/if}
            </div>
          {/each}
          <div class="row"><div><span>Keyboard not listed?</span><small>Create a keyboard report: a short guided recording of what your keyboard sends, to attach to a support request on GitHub. It only listens and changes nothing.</small></div>
            <button class="btn" onclick={() => api.createKeyboardReport().catch((e) => app.toast(String(e), { error: true }))}>Create report</button></div>
          <div class="row"><div><span>Default keyboard</span><small>Used whenever it's connected. Automatic picks the only supported keyboard, and asks when there are several.</small></div>
            <select class="select" value={kb.default ?? ""} onchange={(e) => app.setDefaultKeyboard(e.currentTarget.value || null)}>
              <option value="">Automatic</option>
              {#each kb.list as k (k.id)}<option value={k.id}>{k.name}</option>{/each}
            </select></div>
        </div>
      </section>
    {/if}

    {#if app.prefs}
      {@const pr = app.prefs}
      <section class="sec">
        <header>
          <h2>Hotkeys</h2>
          <p>Work in any app, including games. The keys you press for a hotkey don't reach the app.</p>
        </header>
        <div class="card rows">
          <div class="row"><div><span>Pause and resume</span><small>Controller neutral and keys type normally while paused.</small></div>
            <HotkeyField label="Pause hotkey" value={pr.pause_hotkey ?? ""} onchange={(c) => app.setHotkey("pause", c)} /></div>
          <div class="row"><div><span>Next profile</span><small>Goes through your profiles in order.</small></div>
            <HotkeyField label="Next profile hotkey" value={pr.next_profile_hotkey} onchange={(c) => app.setHotkey("next", c)} /></div>
          {#each app.profiles as name (name)}
            <div class="row"><div><span>Switch to {name}</span></div>
              <HotkeyField label="Hotkey for {name}" value={pr.profile_hotkeys[name] ?? ""} onchange={(c) => app.setProfileHotkey(name, c)} /></div>
          {/each}
          <div class="row"><div><span>Switch indicator</span><small>Shown while a game is in front: a badge at the top of the screen and a short tone. The badge needs the game in windowed or borderless mode; the tone works in fullscreen too.</small></div>
            <Segmented options={["Badge + sound", "Badge", "Sound", "Off"]} value={["both", "badge", "sound", "off"].indexOf(pr.indicator ?? "both")}
                       onchange={(i) => { const m = (["both", "badge", "sound", "off"] as const)[i]; pr.indicator = m; api.setIndicator(m); }} /></div>
        </div>
      </section>
    {/if}

    {#if app.prefs}
      {@const pr = app.prefs}
      <section class="sec">
        <header>
          <h2>Appearance</h2>
          <p>The window's colours. The keyboard and controller drawings stay dark, like the real things.</p>
        </header>
        <div class="card rows">
          <div class="row"><div><span>Theme</span></div>
            <Segmented options={THEMES.map((t) => t.label)} value={Math.max(0, THEMES.findIndex((t) => t.id === (pr.theme ?? "dark")))}
                       onchange={(i) => setAppearance(THEMES[i].id, pr.accent ?? ACCENTS[0].color)} /></div>
          <div class="row"><div><span>Accent colour</span><small>Selection, measured depth and the controller's lights.</small></div>
            <div class="swatches" role="radiogroup" aria-label="Accent colour">
              {#each ACCENTS as a}
                <button class="swatch" class:on={(pr.accent ?? ACCENTS[0].color) === a.color} role="radio" aria-checked={(pr.accent ?? ACCENTS[0].color) === a.color}
                        aria-label={a.label} title={a.label} style:--c={a.color} onclick={() => setAppearance(pr.theme ?? "dark", a.color)}></button>
              {/each}
            </div></div>
        </div>
      </section>

      <section class="sec">
        <header>
          <h2>Controller</h2>
          <p>The virtual controller games see.</p>
        </header>
        <div class="card rows">
          <div class="row"><div><span>Controller type</span><small>Xbox works with almost every PC game. PlayStation (DualShock 4) is for games and tools that want one; buttons map Cross, Circle, Square, Triangle.</small></div>
            <Segmented options={["Xbox 360", "PlayStation"]} value={pr.controller === "ds4" ? 1 : 0}
                       onchange={(i) => { pr.controller = i ? "ds4" : "xbox"; api.setController(pr.controller, pr.unplug_when_paused); }} /></div>
          <div class="row"><div><span>Unplug while paused</span><small>Some games ignore the mouse while a controller is connected. Unplugging it on pause hands them back to mouse and keyboard.</small></div>
            <Toggle label="Unplug while paused" checked={pr.unplug_when_paused}
                    onchange={(v) => { pr.unplug_when_paused = v; api.setController(pr.controller, v); }} /></div>
        </div>
      </section>

      <section class="sec">
        <header>
          <h2>App linking</h2>
          <p>Switch profiles by themselves: a linked program's profile is used while it's in front, and the previous one comes back when you leave it.</p>
        </header>
        <div class="card rows">
          {#each Object.entries(pr.app_links) as [exe, profile] (exe)}
            <div class="row"><div><span class="mono">{exe}</span></div>
              <span class="link-actions">
                <select class="select" value={profile} onchange={(e) => setLink(exe, e.currentTarget.value)}>
                  {#each app.profiles as n}<option value={n}>{n}</option>{/each}
                </select>
                <button class="btn quiet" onclick={() => setLink(exe, "")}>Remove</button>
              </span></div>
          {/each}
          <div class="row"><div><span>Link a program</span><small>Pick one that's open now, such as a running game.</small></div>
            <select class="select" value="" onfocus={loadApps} onchange={(e) => { const v = e.currentTarget.value; e.currentTarget.value = ""; if (v) setLink(v, app.profile?.name ?? "Default"); }}>
              <option value="">Choose a program…</option>
              {#each apps.filter((a) => !(a.exe in pr.app_links)) as a (a.exe)}<option value={a.exe} title="{a.title} ({a.exe})">{short(a.title)} · {a.exe}</option>{/each}
            </select></div>
        </div>
      </section>
    {/if}

    <section class="sec">
      <header>
        <h2>Sticks</h2>
        <p>How keys combine into stick directions.</p>
        <span class="scope">{app.profile.name} profile</span>
      </header>
      <div class="card rows">
        <div class="row"><div><span>Stick shape</span><small>Square treats each axis on its own. Circle keeps diagonals at the stick's edge.</small></div>
          <Segmented options={["Square", "Circle"]} value={s.stick_shape === "circle" ? 1 : 0} onchange={(i) => set("stick_shape", i ? "circle" : "square")} /></div>
        <div class="row"><div><span>Last key priority</span><small>With opposite keys held, the newer one wins instead of cancelling out.</small></div>
          <Toggle label="Last key priority" checked={s.last_key_priority} onchange={(v) => set("last_key_priority", v)} /></div>
        <div class="row"><div><span>Snappy stick</span><small>Opposite keys give the deeper key's full value instead of the difference.</small></div>
          <Toggle label="Snappy stick" checked={s.snappy_joystick} onchange={(v) => set("snappy_joystick", v)} /></div>
        <div class="row"><div><span>Diagonal angle</span><small>Where two keys together point, like W + D. 45° is an even diagonal; higher leans toward up and down, lower toward the sides.</small></div>
          <SliderField inline value={s.diagonal_angle ?? 45} min={10} max={80} step={1} unit="°" digits={0} onchange={(v) => set("diagonal_angle", v)} /></div>
        <div class="row"><div><span>Keys without depth data</span><small>Some keyboards (the AJAZZ AK680) report only the key that moved last. A key held meanwhile counts as pressed to its actuation point or fully; with Measured only it reads 0 until it moves, so every value is real.</small></div>
          <Segmented options={["Actuation point", "Fully pressed", "Measured only"]} value={s.unread_depth === "full" ? 1 : s.unread_depth === "measured" ? 2 : 0} onchange={(i) => set("unread_depth", (["actuation", "full", "measured"] as const)[i])} /></div>
      </div>
    </section>

    <section class="sec">
      <header>
        <h2>Default response</h2>
        <p>How key travel turns into output. Keys can override this on the Mapping page.</p>
        <span class="scope">{app.profile.name} profile</span>
      </header>
      <div class="card rows">
        <div class="curve">
          <div class="editor"><CurveEditor curve={c} {liveMm} onchange={setPoints} /></div>
          <PresetPicker curve={c} active={presetOf(c)} onpick={pickPreset} />
          <p class="hint">{c.mode === "instant" ? "Instant: full output as soon as a key passes the dead zone. Key depths still set a stick's direction."
            : c.mode === "turbo" ? "Turbo: on or off past the dead zone. Sticks snap to the 8 directions."
            : "Drag the points to shape the response. Press a key to see where it lands."}</p>
        </div>
        <div class="row"><div><span>Dead zone</span><small>Travel ignored at the top of a key. The curve's first point.</small></div>
          <SliderField inline value={c.deadzone_mm} min={0} max={1.5} onchange={(v) => setDefault("deadzone_mm", v)} /></div>
        <div class="row"><div><span>Full output at</span><small>Key travel where sticks and triggers reach their top output. The curve's last point.</small></div>
          <SliderField inline value={c.full_mm} min={0.5} max={app.travel} onchange={(v) => setDefault("full_mm", v)} /></div>
        <div class="row"><div><span>Anti-deadzone</span><small>Output jumps to at least this as soon as a key passes the dead zone, so a game's own stick dead zone doesn't swallow light presses.</small></div>
          <SliderField inline value={Math.round((c.anti_deadzone ?? 0) * 100)} min={0} max={50} step={1} unit="%" digits={0}
                       onchange={(v) => app.edit((p) => (p.curve.anti_deadzone = Math.round(v) / 100))} /></div>
        <div class="row"><div><span>Button press point</span><small>Travel at which a mapped button counts as pressed.</small></div>
          <SliderField inline value={c.press_mm} min={0.2} max={3.2} onchange={(v) => setDefault("press_mm", v)} /></div>
      </div>
    </section>

    <section class="sec">
      <header>
        <h2>Keyboard input</h2>
        <p>What happens to the keys themselves while the controller is active.</p>
        <span class="scope">{app.profile.name} profile</span>
      </header>
      <div class="card rows">
        <div class="row"><div><span>Keys while the controller is active</span><small>Blocked keys reach only the controller, not games or other apps. Applies to every keyboard, because Windows doesn't say which keyboard a key came from. The pause hotkey always works, and Depthpad's own window always gets keys.</small></div>
          <Segmented options={["Type normally", "Block mapped keys", "Block all keys"]}
                     value={s.block_all_keys ? 2 : s.block_keys ? 1 : 0}
                     onchange={(i) => app.edit((p) => { p.settings.block_keys = i >= 1; p.settings.block_all_keys = i === 2; })} /></div>
        <div class="row"><div><span>Let Alt and Tab through</span><small>Keep Alt+Tab working while blocking.</small></div>
          <Toggle label="Let Alt and Tab through" checked={s.block_allow_alt_tab} onchange={(v) => set("block_allow_alt_tab", v)} /></div>
      </div>
    </section>

    <section class="sec">
      <header>
        <h2>App</h2>
        <p>Applies to Depthpad on this PC.</p>
      </header>
      <div class="card rows">
        <div class="row"><div><span>Start with Windows</span><small>Starts in the tray.</small></div>
          <Toggle label="Start with Windows" checked={autostart} onchange={setAutostart} /></div>
        {#if app.prefs}
          <div class="row"><div><span>Start in the tray</span></div>
            <Toggle label="Start in the tray" checked={app.prefs.start_minimized} onchange={(v) => setPref("start_minimized", v)} /></div>
          <div class="row"><div><span>Keep running when the window is closed</span><small>Quit from the tray icon.</small></div>
            <Toggle label="Keep running when closed" checked={app.prefs.close_to_tray} onchange={(v) => setPref("close_to_tray", v)} /></div>
          <div class="row"><div><span>Check for updates</span><small>Once at start-up.</small></div>
            <Toggle label="Check for updates" checked={app.prefs.check_updates} onchange={(v) => setPref("check_updates", v)} /></div>
        {/if}
        <div class="row"><div><span>Profile files</span><small class="path" title={app.info?.profile_dir}>{app.info?.profile_dir}</small></div>
          <button class="btn" onclick={() => api.openFolder("profiles")}>Open folder</button></div>
      </div>
    </section>
  </div>
{/if}

<style>
  .page { max-width: 1000px; margin: 0 auto; padding: 8px 0 24px; display: flex; flex-direction: column; gap: 28px; }
  .sec { display: grid; grid-template-columns: 220px minmax(0, 1fr); gap: 32px; }
  .sec > header h2 { margin: 6px 0 4px; font-size: 14px; font-weight: 600; }
  .sec > header p { margin: 0; color: var(--ink-3); font-size: var(--fs-caption); line-height: 1.5; }
  .scope { display: inline-block; margin-top: 8px; padding: 1px 7px; border-radius: 4px; font-size: 11px;
           color: var(--ink-2); background: var(--raised); border: 1px solid var(--stroke-soft); }
  .rows { padding: 0 18px; }
  .row { display: flex; align-items: center; justify-content: space-between; gap: 32px; padding: 13px 0; min-height: 56px; }
  .row + .row { border-top: 1px solid var(--stroke-soft); }
  .row > div:first-child { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  small { color: var(--ink-3); font-size: var(--fs-caption); line-height: 1.45; }
  .path { font-family: "Cascadia Mono", Consolas, monospace; font-size: 11.5px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .btn { height: 30px; padding: 0 12px; font-size: var(--fs-label); border-radius: 7px; flex: none; }
  .kname { display: flex; align-items: center; gap: 8px; }
  .knote { color: var(--ink-2); }
  .curve { display: flex; flex-direction: column; gap: 12px; padding: 16px 0 14px; }
  .editor { height: 250px; }
  .hint { margin: 0; color: var(--ink-3); font-size: var(--fs-caption); }
  .swatches { display: flex; gap: 8px; flex: none; }
  .swatch { width: 26px; height: 26px; border-radius: 50%; cursor: pointer; padding: 0; background: var(--c);
            border: 2px solid var(--surface); box-shadow: 0 0 0 1px var(--stroke); transition: box-shadow 120ms; }
  .swatch.on { box-shadow: 0 0 0 2px var(--c); }
  .swatch:focus-visible { outline: 2px solid var(--accent); outline-offset: 3px; }
  .mono { font-family: "Cascadia Mono", Consolas, monospace; font-size: 12.5px; }
  .link-actions { display: flex; align-items: center; gap: 6px; flex: none; }
  .inuse { flex: none; font-size: var(--fs-label); color: var(--ok); font-weight: 600; padding: 0 4px; }
  .btn[disabled] { opacity: 0.45; cursor: default; }
  .select { flex: none; height: 30px; min-width: 200px; max-width: 280px; text-overflow: ellipsis; padding: 0 28px 0 10px; border-radius: 7px; cursor: pointer;
            background: var(--raised) url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 24 24' fill='none' stroke='%23a6a6a6' stroke-width='2'%3E%3Cpath d='m6 9 6 6 6-6'/%3E%3C/svg%3E") no-repeat right 9px center;
            border: 1px solid var(--stroke); color: var(--ink); font-size: var(--fs-label); appearance: none; }
  .select:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }
  .select option { background: var(--raised); color: var(--ink); }
</style>
