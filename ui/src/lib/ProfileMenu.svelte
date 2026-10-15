<!-- The active profile as a pill; its dropdown switches profiles and holds the profile actions. -->
<script lang="ts">
  import { ask, open as openFile, save as saveFile } from "@tauri-apps/plugin-dialog";
  import { fly } from "svelte/transition";
  import { api } from "./api";
  import Icon from "./Icon.svelte";
  import { app } from "./state.svelte";

  let open = $state(false);
  let renaming = $state(false);
  let renameValue = $state("");

  async function choose(name: string) {
    open = false;
    if (name !== app.profile?.name) await app.run(() => api.activateProfile(name));
  }

  async function act(what: string) {
    open = false;
    const p = app.profile!;
    if (what === "new") return app.run(() => api.newProfile(false));
    if (what === "duplicate") return app.run(() => api.newProfile(true));
    if (what === "rename") { renameValue = p.name; renaming = true; return; }
    if (what === "import") {
      const path = await openFile({ filters: [{ name: "Depthpad profile", extensions: ["json"] }] });
      if (path) await app.run(() => api.importProfile(path as string));
    }
    if (what === "export") {
      const path = await saveFile({ defaultPath: `${p.name}.json`, filters: [{ name: "Depthpad profile", extensions: ["json"] }] });
      if (path) await api.exportProfile(path).catch((e) => app.toast(String(e), { error: true }));
    }
    if (what === "delete" && (await ask(`Delete the profile “${p.name}”?`, { title: "Depthpad", kind: "warning" })))
      await app.run(() => api.deleteProfile());
  }

  async function commit() {
    renaming = false;
    if (renameValue.trim() && renameValue !== app.profile?.name) await app.run(() => api.renameProfile(renameValue));
  }
</script>

<svelte:window onclick={() => (open = false)} onkeydown={(e) => e.key === "Escape" && (open = false)} />

<div class="wrap">
  {#if renaming}
    <!-- svelte-ignore a11y_autofocus -->
    <input class="pill rename" bind:value={renameValue} autofocus
           onkeydown={(e) => { if (e.key === "Enter") commit(); if (e.key === "Escape") renaming = false; }} onblur={commit} />
  {:else}
    <button class="pill" aria-haspopup="menu" aria-expanded={open} onclick={(e) => { e.stopPropagation(); open = !open; }}>
      <span class="cap">Profile</span><span class="name">{app.profile?.name}</span>
      <span class="chev" class:up={open}><Icon name="chevron-down" size={16} /></span>
    </button>
  {/if}

  {#if open}
    <div class="menu" role="menu" tabindex="-1" transition:fly={{ y: -6, duration: 140 }} onclick={(e) => e.stopPropagation()}
         onkeydown={() => {}}>
      <div class="section">Profiles</div>
      {#each app.profiles as name}
        <button role="menuitemradio" aria-checked={name === app.profile?.name} class:current={name === app.profile?.name}
                onclick={() => choose(name)}>
          <span class="grow">{name}</span>
          {#if name === app.profile?.name}<Icon name="check" size={15} />{/if}
        </button>
      {/each}
      <hr />
      <button role="menuitem" onclick={() => act("new")}><Icon name="plus" size={15} />New profile</button>
      <button role="menuitem" onclick={() => act("duplicate")}><Icon name="copy" size={15} />Duplicate</button>
      <button role="menuitem" onclick={() => act("rename")}><Icon name="pencil" size={15} />Rename</button>
      <button role="menuitem" onclick={() => act("import")}><Icon name="upload" size={15} />Import…</button>
      <button role="menuitem" onclick={() => act("export")}><Icon name="download" size={15} />Export…</button>
      <hr />
      <button role="menuitem" class="danger" onclick={() => act("delete")}><Icon name="trash-2" size={15} />Delete profile</button>
    </div>
  {/if}
</div>

<style>
  .wrap { position: relative; }
  .pill { height: 30px; display: flex; align-items: center; gap: 8px; padding: 0 8px 0 10px; border-radius: 7px;
          background: var(--raised); border: 1px solid var(--stroke); cursor: pointer; font-size: var(--fs-label);
          transition: background 120ms var(--ease); }
  .pill:hover { background: var(--hover); }
  .rename { width: 200px; color: var(--ink); font-weight: 600; padding: 0 10px; outline: none; border-color: var(--accent); }
  .cap { color: var(--ink-3); }
  .name { font-weight: 600; max-width: 160px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .chev { color: var(--ink-3); display: flex; transition: transform 160ms var(--ease); }
  .chev.up { transform: rotate(180deg); }
  .menu { position: absolute; top: calc(100% + 6px); right: 0; z-index: 60; width: 230px; padding: 4px;
          background: var(--raised); border: 1px solid var(--stroke); border-radius: 9px; box-shadow: 0 12px 32px rgba(0, 0, 0, 0.5); }
  .section { padding: 7px 10px 4px; font-size: var(--fs-caption); color: var(--ink-3); }
  .menu button { width: 100%; display: flex; align-items: center; gap: 9px; padding: 7px 10px; border: 0; font-size: var(--fs-label);
                 background: none; border-radius: 6px; text-align: left; cursor: pointer; color: var(--ink); }
  .menu button:hover { background: var(--hover); }
  .menu button :global(svg) { color: var(--ink-2); }
  .current :global(svg) { color: var(--accent-ink) !important; }
  .grow { flex: 1; }
  .danger, .danger :global(svg) { color: var(--error) !important; }
  hr { border: 0; height: 1px; background: var(--stroke-soft); margin: 4px 6px; }
</style>
