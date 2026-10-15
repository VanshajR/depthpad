<!-- How well support for a keyboard model has been verified. -->
<script lang="ts">
  let { status }: { status: "tested" | "community" | "untested" } = $props();
  const LABEL = { tested: "Tested", community: "Community tested", untested: "Untested" };
  const TITLE = {
    tested: "Tested on this keyboard by the Depthpad maintainers.",
    community: "Reported working by owners of this keyboard.",
    untested: "Supported from another project's work; nobody has tried it with Depthpad yet. Reports welcome.",
  };
</script>

<span class="badge {status}" title={TITLE[status]}>{LABEL[status]}</span>

<style>
  .badge { flex: none; padding: 1px 7px; border-radius: 4px; font-size: 11px; font-weight: 500; white-space: nowrap;
           color: var(--ink-2); background: var(--raised); border: 1px solid var(--stroke-soft); }
  .untested { color: var(--warn); border-color: rgba(240, 163, 58, 0.3); background: rgba(240, 163, 58, 0.06); }
</style>
