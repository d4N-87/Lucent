<script lang="ts">
  import { fade, scale } from "svelte/transition";
  import type { Layout, Rgb } from "../lib/api";
  import { css } from "../lib/color";
  import { clampCrop, cropRect, DEFAULT_CROP, sampleKeys, type Crop } from "../lib/image";
  import { t } from "../lib/i18n.svelte";

  interface Props {
    layout: Layout;
    image: ImageBitmap;
    /** File name, shown as the subtitle. */
    name: string;
    onapply: (colors: Map<number, Rgb>) => void;
    oncancel: () => void;
  }

  let { layout, image, name, onapply, oncancel }: Props = $props();

  /** Canvas width in CSS pixels; the height follows the keyboard's proportions. */
  const VIEW_W = 600;
  const aspect = $derived(layout.width / layout.height);
  const viewH = $derived(Math.round(VIEW_W / aspect));
  const unit = $derived(VIEW_W / layout.width);

  let crop = $state<Crop>({ ...DEFAULT_CROP });
  let vividness = $state(0.3);
  let source = $state<HTMLCanvasElement | undefined>();
  let result = $state<HTMLCanvasElement | undefined>();

  const colors = $derived(sampleKeys(image, layout, crop, vividness));

  function setCrop(next: Crop) {
    crop = clampCrop(image.width, image.height, aspect, next);
  }

  // Draws the cropped picture with the keys outlined, and the per-key result below.
  $effect(() => {
    const dpr = devicePixelRatio;
    for (const canvas of [source, result]) {
      if (!canvas) return;
      canvas.width = VIEW_W * dpr;
      canvas.height = viewH * dpr;
    }
    const s = source!.getContext("2d")!;
    const r = result!.getContext("2d")!;
    s.setTransform(dpr, 0, 0, dpr, 0, 0);
    r.setTransform(dpr, 0, 0, dpr, 0, 0);

    const { sx, sy, sw, sh } = cropRect(image.width, image.height, aspect, crop);
    s.imageSmoothingQuality = "high";
    s.drawImage(image, sx, sy, sw, sh, 0, 0, VIEW_W, viewH);
    r.fillStyle = "#111115";
    r.fillRect(0, 0, VIEW_W, viewH);

    s.strokeStyle = "rgba(255, 255, 255, 0.45)";
    s.lineWidth = 1;
    for (const k of layout.keys) {
      const x = k.x * unit + 1.5;
      const y = k.y * unit + 1.5;
      const w = k.w * unit - 3;
      const h = k.h * unit - 3;
      s.beginPath();
      s.roundRect(x, y, w, h, unit * 0.12);
      s.stroke();
      r.beginPath();
      r.roundRect(x, y, w, h, unit * 0.12);
      r.fillStyle = css(colors.get(k.slot) ?? [0, 0, 0]);
      r.fill();
    }
  });

  // Dragging pans the picture under the keyboard; the wheel zooms.
  function drag(e: PointerEvent) {
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    let lastX = e.clientX;
    let lastY = e.clientY;
    el.onpointermove = (ev) => {
      const { sw, sh } = cropRect(image.width, image.height, aspect, crop);
      const dx = ((ev.clientX - lastX) / VIEW_W) * sw;
      const dy = ((ev.clientY - lastY) / viewH) * sh;
      lastX = ev.clientX;
      lastY = ev.clientY;
      setCrop({ ...crop, cx: crop.cx - dx / image.width, cy: crop.cy - dy / image.height });
    };
    el.onpointerup = () => (el.onpointermove = null);
  }

  function wheel(e: WheelEvent) {
    e.preventDefault();
    setCrop({ ...crop, zoom: crop.zoom * Math.pow(1.1, -e.deltaY / 100) });
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && oncancel()} />

<div class="scrim" transition:fade={{ duration: 150 }}>
  <div class="dialog" role="dialog" aria-modal="true" transition:scale={{ start: 0.96, duration: 200 }}>
    <div class="head">
      <div>
        <h2>{t("image.title")}</h2>
        <p class="sub">{name}</p>
      </div>
      <button class="btn" onclick={() => setCrop({ ...DEFAULT_CROP })}>{t("image.fit")}</button>
    </div>

    <div class="caption">{t("image.framing")}</div>
    <canvas
      bind:this={source}
      class="view source"
      style:width="{VIEW_W}px"
      style:height="{viewH}px"
      onpointerdown={drag}
      onwheel={wheel}
    ></canvas>
    <p class="hint left">{t("image.framing_hint")}</p>

    <div class="caption">{t("image.result")}</div>
    <canvas bind:this={result} class="view" style:width="{VIEW_W}px" style:height="{viewH}px"></canvas>

    <div class="controls">
      <label class="control">
        <span class="caption-row">
          <span class="caption">{t("image.zoom")}</span>
          <span class="count">{crop.zoom.toFixed(1)}×</span>
        </span>
        <input
          type="range"
          class="slider"
          min="1"
          max="8"
          step="0.05"
          value={crop.zoom}
          oninput={(e) => setCrop({ ...crop, zoom: Number((e.target as HTMLInputElement).value) })}
          style:--fill="{((crop.zoom - 1) / 7) * 100}%"
        />
      </label>
      <label class="control">
        <span class="caption-row">
          <span class="caption">{t("image.vividness")}</span>
          <span class="count">{Math.round(vividness * 100)}%</span>
        </span>
        <input
          type="range"
          class="slider"
          min="0"
          max="1"
          step="0.01"
          bind:value={vividness}
          style:--fill="{vividness * 100}%"
        />
      </label>
    </div>

    <div class="actions">
      <button class="btn" onclick={oncancel}>{t("common.cancel")}</button>
      <button class="btn primary" onclick={() => onapply(colors)}>{t("image.apply")}</button>
    </div>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 30;
    display: grid;
    place-items: center;
    background: var(--shadow-strong);
    backdrop-filter: blur(8px);
  }

  .dialog {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 22px;
    border-radius: 18px;
    background: var(--panel);
    border: 0.5px solid var(--hairline-strong);
    backdrop-filter: blur(40px);
    box-shadow: 0 30px 80px var(--shadow-strong);
  }

  .head {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    margin-bottom: 6px;
  }

  h2 {
    margin: 0;
    font-size: 15px;
    font-weight: 700;
  }

  .sub {
    margin: 2px 0 0;
    max-width: 480px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-2);
    font-size: 12px;
  }

  .view {
    display: block;
    border-radius: 12px;
    box-shadow: inset 0 0 0 0.5px var(--fill-hover);
    background: #111115;
  }

  .source {
    cursor: grab;
    touch-action: none;
  }

  .source:active {
    cursor: grabbing;
  }

  .controls {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 20px;
    margin-top: 6px;
  }

  .control {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 8px;
  }
</style>
