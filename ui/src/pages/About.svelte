<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api } from "../lib/api";
  import { app } from "../lib/state.svelte";

  const REPO = "https://github.com/VanshajR/depthpad";
</script>

<div class="page">
  <section class="sec">
    <header class="id">
      <img src="/icon.png" alt="" width="40" height="40" />
      <h2>Depthpad</h2>
      <p>Version {app.info?.version}</p>
    </header>
    <div class="card rows">
      <div class="row"><div><span>Analog controller for Hall-effect keyboards</span>
        <small>Turns key depth into a virtual game controller. Free and open source.</small></div>
        <span class="links">
          <button class="btn" onclick={() => openUrl(REPO)}>Source code</button>
          <button class="btn" onclick={() => openUrl(`${REPO}/issues`)}>Report a problem</button>
          <button class="btn" onclick={() => app.checkForUpdate(true)}>Check for updates</button>
        </span></div>
    </div>
  </section>

  <section class="sec">
    <header>
      <h2>Diagnostics</h2>
      <p>For checking stuck or missing input, or attaching to a bug report.</p>
    </header>
    <div class="card rows">
      <div class="row"><div><span>Record a session</span><small>Recordings contain every key pressed on the keyboard. Review before sharing.</small></div>
        <button class="btn" class:primary={app.recording} onclick={() => app.toggleRecording()}>{app.recording ? "Stop recording" : "Start recording"}</button></div>
      <div class="row"><div><span>Logs and recordings</span><small class="path" title={app.info?.log_dir}>{app.info?.log_dir}</small></div>
        <button class="btn" onclick={() => api.openFolder("logs")}>Open folder</button></div>
    </div>
  </section>

  <section class="sec">
    <header>
      <h2>Licences</h2>
      <p>Depthpad is free software under the GNU AGPL-3.0.</p>
    </header>
    <div class="card rows">
      <dl>
        <dt>HallJoy</dt><dd>Stick behaviour, key-blocking rules, keyboard session handling and AULA / AK820 support are ported from it. AGPL-3.0, PashOK7.</dd>
        <dt>AnalogSense</dt><dd>Protocols for Wooting, Razer, NuPhy, DrunkDeer, Keychron, Lemokey, MadLions and Redragon boards are ported from its JavaScript SDK. MIT, Calamity, Inc.</dd>
        <dt>ViGEmBus</dt><dd>Virtual controller driver by Nefarius, used through the vigem-client crate. BSD-3-Clause / MIT.</dd>
        <dt>Tauri, Svelte</dt><dd>Application framework and interface. MIT / Apache-2.0.</dd>
        <dt>Lucide</dt><dd>Interface icons. ISC.</dd>
      </dl>
      <p class="tm">AJAZZ, Xbox and Windows are trademarks of their owners. Depthpad is not affiliated with them.</p>
    </div>
  </section>
</div>

<style>
  .page { max-width: 1000px; margin: 0 auto; padding: 8px 0 24px; display: flex; flex-direction: column; gap: 28px; }
  .sec { display: grid; grid-template-columns: 220px minmax(0, 1fr); gap: 32px; }
  .sec > header h2 { margin: 6px 0 4px; font-size: 14px; font-weight: 600; }
  .sec > header p { margin: 0; color: var(--ink-3); font-size: var(--fs-caption); line-height: 1.5; }
  .rows { padding: 0 18px; }
  .row { display: flex; align-items: center; justify-content: space-between; gap: 32px; padding: 13px 0; min-height: 56px; }
  .row + .row { border-top: 1px solid var(--stroke-soft); }
  .row > div:first-child { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  small { color: var(--ink-3); font-size: var(--fs-caption); line-height: 1.45; }
  .path { font-family: "Cascadia Mono", Consolas, monospace; font-size: 11.5px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .btn { height: 30px; padding: 0 12px; font-size: var(--fs-label); border-radius: 7px; flex: none; }
  .id img { border-radius: 9px; display: block; margin-bottom: 10px; }
  .links { display: flex; gap: 6px; flex: none; }
  dl { display: grid; grid-template-columns: 110px 1fr; gap: 10px 16px; margin: 0; padding: 16px 0 12px; }
  dt { font-weight: 600; }
  dd { margin: 0; color: var(--ink-2); }
  .tm { margin: 0; padding: 12px 0 14px; border-top: 1px solid var(--stroke-soft); color: var(--ink-3); font-size: var(--fs-caption); }
</style>
