<script lang="ts">
  import { fly } from "svelte/transition";
  import { app } from "./state.svelte";
  import Icon from "./Icon.svelte";
</script>

<div class="toasts" aria-live="polite">
  {#each app.toasts as t (t.id)}
    <div class="toast" class:error={t.error} transition:fly={{ y: 12, duration: 180 }}>
      <span class="dot"><Icon name="info" size={16} /></span>
      <span class="msg">{t.message}</span>
      {#if t.action}
        <button class="btn quiet act" onclick={() => { t.action!.run(); app.dismiss(t.id); }}>{t.action.label}</button>
      {/if}
    </div>
  {/each}
</div>

<style>
  .toasts { position: fixed; right: 24px; bottom: 24px; display: flex; flex-direction: column; gap: 10px; z-index: 50; }
  .toast { display: flex; align-items: center; gap: 10px; min-width: 300px; max-width: 460px; padding: 10px 10px 10px 14px;
           background: var(--raised); border: 1px solid var(--stroke); border-radius: 10px;
           box-shadow: 0 12px 32px rgba(0, 0, 0, 0.45); }
  .dot { color: var(--accent-ink); display: flex; }
  .error .dot { color: var(--error); }
  .msg { flex: 1; }
  .act { height: 30px; color: var(--accent-ink); font-weight: 600; }
</style>
