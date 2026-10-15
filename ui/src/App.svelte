<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import { api } from "./lib/api";
  import Icon from "./lib/Icon.svelte";
  import ProfileMenu from "./lib/ProfileMenu.svelte";
  import { app } from "./lib/state.svelte";
  import KeyboardChooser from "./lib/KeyboardChooser.svelte";
  import Toasts from "./lib/Toasts.svelte";
  import About from "./pages/About.svelte";
  import Live from "./pages/Live.svelte";
  import Mapping from "./pages/Mapping.svelte";
  import Settings from "./pages/Settings.svelte";

  const NAV = [
    { id: "live", label: "Live", icon: "activity" },
    { id: "mapping", label: "Mapping", icon: "keyboard" },
    { id: "settings", label: "Settings", icon: "sliders-horizontal" },
    { id: "about", label: "About", icon: "info" },
  ] as const;

  onMount(() => { app.init(); });

  // Alt or F10 on its own puts a Windows window into menu mode, which pauses its event loop:
  // live updates stop until the next key. There is no menu, so keep them for the page.
  function noMenuKey(e: KeyboardEvent) {
    if (e.key === "Alt" || e.key === "F10") e.preventDefault();
  }

  let s = $derived(app.snapshot);
  let status = $derived.by(() => {
    if (!s) return { text: "Starting", detail: "", tone: "warn" };
    if (s.state === "searching") return { text: "Keyboard not found", detail: s.error ?? "", tone: "error" };
    if (s.state !== "streaming") return { text: "Connecting", detail: "Waiting for the keyboard", tone: "warn" };
    if (s.paused) return { text: "Paused", detail: "Controller neutral, keys type normally", tone: "warn" };
    if (!s.pad) return { text: "No controller", detail: s.pad_error ?? "", tone: "error" };
    return { text: "Active", detail: `${Math.round(s.rate)} depth updates/s${s.blocking ? " · keys blocked" : ""}`, tone: "ok" };
  });
</script>

<svelte:window onkeydown={noMenuKey} onkeyup={noMenuKey} />

<div class="shell">
  <header class="bar">
    <div class="brand"><img src="/icon.png" alt="" width="20" height="20" /><span>Depthpad</span></div>

    <nav class="tabs" aria-label="Pages">
      {#each NAV as n}
        <button class:on={app.page === n.id} aria-current={app.page === n.id ? "page" : undefined}
                onmousedown={(e) => e.preventDefault()} onclick={() => (app.page = n.id)}>{n.label}</button>
      {/each}
    </nav>

    <div class="right">
      {#if app.recording}
        <button class="rec" title="Stop recording and save the session log" onclick={() => app.toggleRecording()}>
          <span class="rec-dot"></span>Recording<span class="rec-stop">Stop</span>
        </button>
      {/if}
      <ProfileMenu />
      <span class="divider"></span>
      <div class="status {status.tone}" title={status.detail}>
        <span class="dot"></span><span class="main">{status.text}</span>
        {#if status.detail}<span class="detail">{status.detail}</span>{/if}
      </div>
      <button class="pause" class:paused={s?.paused} aria-label={s?.paused ? "Resume" : "Pause"}
              title={s?.paused ? "Resume the controller" : "Pause the controller"} onmousedown={(e) => e.preventDefault()} onclick={() => api.setPaused(!s?.paused)}>
        <Icon name={s?.paused ? "play" : "pause"} size={15} /><span>{s?.paused ? "Resume" : "Pause"}</span>
      </button>
    </div>
  </header>

  {#if app.update}
    <div class="banner update">
      <Icon name="download" size={16} /><span>Depthpad {app.update.version} is available.</span>
      <button class="btn quiet" onclick={() => openUrl(`https://github.com/VanshajR/depthpad/releases/tag/v${app.update?.version}`)}>What's new</button>
      <button class="btn primary" disabled={app.updating} onclick={() => app.installUpdate()}>{app.updating ? "Updating…" : "Update and restart"}</button>
    </div>
  {/if}
  {#if !app.vigembus}
    <div class="banner error">
      <Icon name="info" size={16} /><span>The ViGEmBus driver isn't installed, so games can't see the controller. Install it, then restart Depthpad.</span>
      <button class="btn primary" onclick={() => api.installVigembus().catch(() => openUrl("https://github.com/nefarius/ViGEmBus/releases/latest"))}>Install ViGEmBus</button>
    </div>
  {/if}
  {#if s?.contention}
    <div class="banner" transition:fade>
      <Icon name="info" size={16} /><span>Another app keeps switching the keyboard's analog mode off. Close the keyboard's own configurator (web or app) while using Depthpad.</span>
    </div>
  {/if}

  <main class="content">
    {#key app.page}
      <div class="pagewrap" in:fade={{ duration: 160 }}>
        {#if app.page === "live"}<Live />
        {:else if app.page === "mapping"}<Mapping />
        {:else if app.page === "settings"}<Settings />
        {:else}<About />{/if}
      </div>
    {/key}
  </main>
</div>
{#if app.keyboards?.needs_choice}<KeyboardChooser />{/if}
<Toasts />

<style>
  .shell { height: 100%; display: flex; flex-direction: column; background: var(--bg); }
  .bar { display: flex; align-items: center; gap: 28px; height: 52px; padding: 0 20px; flex: none;
         border-bottom: 1px solid var(--stroke-soft); background: var(--rail); }
  .brand { display: flex; align-items: center; gap: 9px; font: 650 14px/1 var(--font-display); letter-spacing: -0.01em; }
  .brand img { border-radius: 5px; }

  .tabs { display: flex; align-self: stretch; gap: 4px; }
  .tabs button { position: relative; padding: 0 12px; border: 0; background: none; color: var(--ink-2);
                 font-weight: 500; cursor: pointer; transition: color 120ms; }
  .tabs button:hover { color: var(--ink); }
  .tabs button.on { color: var(--ink); font-weight: 600; }
  .tabs button.on::after { content: ""; position: absolute; left: 12px; right: 12px; bottom: -1px; height: 2px;
                           border-radius: 2px 2px 0 0; background: var(--accent); }
  .tabs button:focus-visible { outline: 2px solid var(--accent); outline-offset: -6px; border-radius: 8px; }

  .right { display: flex; align-items: center; gap: 14px; margin-left: auto; min-width: 0; }
  .rec { display: inline-flex; align-items: center; gap: 8px; height: 30px; padding: 0 4px 0 10px; border-radius: 7px;
         border: 1px solid rgba(239, 83, 80, 0.35); background: rgba(239, 83, 80, 0.08); color: var(--ink);
         font-size: var(--fs-label); font-weight: 600; cursor: pointer; }
  .rec-dot { width: 7px; height: 7px; border-radius: 50%; background: var(--error); animation: blink 1.4s ease-in-out infinite; }
  .rec-stop { height: 22px; padding: 0 8px; display: grid; place-items: center; border-radius: 5px; background: rgba(239, 83, 80, 0.18); }
  .rec:hover .rec-stop { background: rgba(239, 83, 80, 0.3); }
  @keyframes blink { 50% { opacity: 0.35; } }
  .divider { width: 1px; height: 20px; background: var(--stroke); }
  .status { display: flex; align-items: center; gap: 8px; min-width: 0; font-size: var(--fs-label); }
  .dot { width: 7px; height: 7px; border-radius: 50%; flex: none; }
  .ok .dot { background: var(--ok); }
  .warn .dot { background: var(--warn); }
  .error .dot { background: var(--error); }
  .main { font-weight: 600; white-space: nowrap; }
  .detail { color: var(--ink-3); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 260px;
            font-variant-numeric: tabular-nums; }
  .pause { display: inline-flex; align-items: center; gap: 7px; height: 30px; padding: 0 12px; border-radius: 7px; flex: none;
           cursor: pointer; background: var(--raised); border: 1px solid var(--stroke); color: var(--ink);
           font-size: var(--fs-label); font-weight: 600; transition: background 120ms var(--ease); }
  .pause:hover { background: var(--hover); }
  .pause.paused { background: var(--accent); border-color: var(--accent); color: #fff; }

  .banner { margin: 12px 20px 0; display: flex; align-items: center; gap: 10px; padding: 8px 8px 8px 12px; border-radius: 8px;
            background: rgba(240, 163, 58, 0.08); border: 1px solid rgba(240, 163, 58, 0.22); }
  .banner :global(svg) { color: var(--warn); flex: none; }
  .banner span { flex: 1; }
  .banner.error { background: rgba(239, 83, 80, 0.08); border-color: rgba(239, 83, 80, 0.22); }
  .banner.error :global(svg) { color: var(--error); }
  .banner.update { background: var(--accent-wash); border-color: color-mix(in srgb, var(--accent) 35%, transparent); }
  .banner.update :global(svg) { color: var(--accent); }
  .content { flex: 1; overflow-y: auto; padding: 20px; }
  .pagewrap { height: 100%; }
</style>
