<!-- Real-time line chart of two series over the last 5 s, one y-axis. Dataviz method: 2 px lines,
     solid hairline grid, legend for two series, end dots with a surface ring, neutral text, and a
     crosshair with values on hover. -->
<script lang="ts">
  import { app } from "./state.svelte";

  let { title, series, lo, hi }: { title: string; series: { name: string; color: string; index: number }[]; lo: number; hi: number } = $props();

  let canvas: HTMLCanvasElement;
  let hoverX = $state<number | null>(null);
  const css = (v: string) => getComputedStyle(document.documentElement).getPropertyValue(v).trim();
  const SECONDS = 5;

  function draw() {
    if (!canvas) return;
    const dpr = window.devicePixelRatio || 1;
    const w = canvas.clientWidth, h = canvas.clientHeight;
    if (canvas.width !== Math.round(w * dpr) || canvas.height !== Math.round(h * dpr)) {
      canvas.width = Math.round(w * dpr);
      canvas.height = Math.round(h * dpr);
    }
    const g = canvas.getContext("2d")!;
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
    g.clearRect(0, 0, w, h);
    const area = { l: 34, t: 6, r: w - 6, b: h - 6 };
    const toY = (v: number) => area.b - ((v - lo) / (hi - lo)) * (area.b - area.t);
    g.font = `500 11px ${css("--font")}`;
    g.textAlign = "right";
    g.textBaseline = "middle";
    for (const v of [lo, (lo + hi) / 2, hi]) {
      const y = Math.round(toY(v)) + 0.5;
      g.strokeStyle = lo < 0 && v === 0 ? "#3a3a3a" : css("--stroke-soft");
      g.lineWidth = 1;
      g.beginPath(); g.moveTo(area.l, y); g.lineTo(area.r, y); g.stroke();
      g.fillStyle = css("--ink-3");
      g.fillText(lo < 0 ? (v === 0 ? "0" : (v > 0 ? "+" : "") + v.toFixed(0)) : `${Math.round(v * 100)}%`, area.l - 8, y);
    }
    const data = app.history;
    if (data.length < 2) return;
    const tEnd = data[data.length - 1][0];
    const toX = (t: number) => area.r - ((tEnd - t) / SECONDS) * (area.r - area.l);
    for (const s of series) {
      g.strokeStyle = s.color;
      g.lineWidth = 2;
      g.lineJoin = "round";
      g.lineCap = "round";
      g.beginPath();
      data.forEach((d, i) => (i ? g.lineTo(toX(d[0]), toY(d[s.index])) : g.moveTo(toX(d[0]), toY(d[s.index]))));
      g.stroke();
      const last = data[data.length - 1];
      g.beginPath();
      g.arc(toX(last[0]), toY(last[s.index]), 4, 0, Math.PI * 2);
      g.fillStyle = s.color;
      g.strokeStyle = css("--surface");
      g.lineWidth = 2;
      g.fill(); g.stroke();
    }
    if (hoverX !== null && hoverX >= area.l && hoverX <= area.r) {
      const t = tEnd - ((area.r - hoverX) / (area.r - area.l)) * SECONDS;
      const sample = data.reduce((a, b) => (Math.abs(b[0] - t) < Math.abs(a[0] - t) ? b : a));
      const x = Math.round(toX(sample[0])) + 0.5;
      g.strokeStyle = css("--ink-3");
      g.lineWidth = 1;
      g.beginPath(); g.moveTo(x, area.t); g.lineTo(x, area.b); g.stroke();
      const lines = [`${(tEnd - sample[0]).toFixed(2)} s ago`, ...series.map((s) =>
        `${s.name}  ${lo < 0 ? (sample[s.index] >= 0 ? "+" : "") + sample[s.index].toFixed(2) : Math.round(sample[s.index] * 100) + "%"}`)];
      g.font = `500 12px ${css("--font")}`;
      const bw = Math.max(...lines.map((l) => g.measureText(l).width)) + 34;
      const bh = lines.length * 18 + 10;
      const bx = x + 10 + bw < area.r ? x + 10 : x - 10 - bw;
      g.fillStyle = css("--raised");
      g.strokeStyle = css("--stroke");
      g.beginPath(); g.roundRect(bx, area.t + 4, bw, bh, 8); g.fill(); g.stroke();
      g.textAlign = "left";
      lines.forEach((l, i) => {
        const y = area.t + 4 + 5 + i * 18 + 9;
        if (i) {
          g.strokeStyle = series[i - 1].color;
          g.lineWidth = 2;
          g.beginPath(); g.moveTo(bx + 10, y); g.lineTo(bx + 18, y); g.stroke();
        }
        g.fillStyle = i ? css("--ink") : css("--ink-3");
        g.fillText(l, bx + 24, y);
      });
    }
  }

  $effect(() => {
    app.historyTick;
    hoverX;
    draw();
  });
</script>

<figure>
  <figcaption>
    <span class="title">{title}</span>
    <span class="legend">
      {#each series as s}<span class="item"><i style:background={s.color}></i>{s.name}</span>{/each}
    </span>
  </figcaption>
  <canvas bind:this={canvas} onmousemove={(e) => (hoverX = e.offsetX)} onmouseleave={() => (hoverX = null)}
          aria-label="{title} over the last 5 seconds"></canvas>
</figure>

<style>
  figure { margin: 0; display: flex; flex-direction: column; gap: 8px; min-width: 0; }
  figcaption { display: flex; justify-content: space-between; align-items: center; }
  .title { font-size: var(--fs-label); font-weight: 600; }
  .legend { display: flex; gap: 14px; color: var(--ink-2); font-size: var(--fs-caption); }
  .item { display: flex; align-items: center; gap: 6px; }
  i { width: 10px; height: 2px; border-radius: 1px; display: inline-block; }
  canvas { width: 100%; height: 170px; display: block; cursor: crosshair; }
</style>
