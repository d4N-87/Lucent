<script lang="ts">
  import { untrack } from "svelte";
  import type { Rgb } from "../lib/api";
  import { fromHex, hsvToRgb, rgbToHsv, toHex, type Hsv } from "../lib/color";
  import { SWATCHES } from "../lib/groups";
  import { t } from "../lib/i18n.svelte";

  interface Props {
    value: Rgb;
    /** Called on every change, including while dragging. */
    onpick: (rgb: Rgb) => void;
  }

  let { value, onpick }: Props = $props();

  let hsv = $state<Hsv>(rgbToHsv(untrack(() => value)));

  // Follow outside changes of `value` (e.g. switching target), but keep the current hue
  // when the color is the same: gray and black have no hue of their own.
  $effect(() => {
    const next = value;
    untrack(() => {
      if (toHex(hsvToRgb(hsv)) !== toHex(next)) hsv = rgbToHsv(next);
    });
  });
  const rgb = $derived(hsvToRgb(hsv));
  const hex = $derived(toHex(rgb));
  const hueColor = $derived(toHex(hsvToRgb({ h: hsv.h, s: 1, v: 1 })));

  function set(next: Hsv) {
    hsv = next;
    onpick(hsvToRgb(next));
  }

  /** Hues of the pure LED colors (red, yellow, green, cyan, blue, magenta). */
  const PURE_HUES = [0, 60, 120, 180, 240, 300, 360];
  const HUE_SNAP = 5;
  const EDGE_SNAP = 0.03;

  // Near a pure hue or an edge of the square, snap to it: a red dragged to 3° comes out
  // as #FF0E00, which looks orange on the keyboard.
  function snapHue(h: number): number {
    const pure = PURE_HUES.find((p) => Math.abs(h - p) <= HUE_SNAP);
    return pure === undefined ? h : pure % 360;
  }

  const snapEdge = (x: number) => (x <= EDGE_SNAP ? 0 : x >= 1 - EDGE_SNAP ? 1 : x);

  // Pointer capture keeps the drag alive when the cursor leaves the control.
  function track(e: PointerEvent, apply: (fx: number, fy: number) => void) {
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    const move = (ev: PointerEvent) => {
      const r = el.getBoundingClientRect();
      const fx = Math.min(1, Math.max(0, (ev.clientX - r.left) / r.width));
      const fy = Math.min(1, Math.max(0, (ev.clientY - r.top) / r.height));
      apply(fx, fy);
    };
    move(e);
    el.onpointermove = move;
    el.onpointerup = () => (el.onpointermove = null);
  }

  function onHex(e: Event) {
    const parsed = fromHex((e.target as HTMLInputElement).value);
    if (parsed) set(rgbToHsv(parsed));
  }
</script>

<div class="picker">
  <div
    class="sv"
    role="slider"
    aria-label={t("picker.saturation")}
    aria-valuenow={Math.round(hsv.s * 100)}
    tabindex="0"
    style:background-color={hueColor}
    onpointerdown={(e) => track(e, (fx, fy) => set({ ...hsv, s: snapEdge(fx), v: snapEdge(1 - fy) }))}
  >
    <div class="thumb" style:left="{hsv.s * 100}%" style:top="{(1 - hsv.v) * 100}%" style:background={hex}></div>
  </div>

  <div
    class="hue"
    role="slider"
    aria-label={t("picker.hue")}
    aria-valuenow={Math.round(hsv.h)}
    tabindex="0"
    onpointerdown={(e) => track(e, (fx) => set({ ...hsv, h: snapHue(fx * 360) }))}
  >
    <div class="thumb round" style:left="{(hsv.h / 360) * 100}%" style:background={hueColor}></div>
  </div>

  <div class="row">
    <div class="preview" style:background={hex} style:box-shadow="0 0 18px {hex}66"></div>
    <input class="hex" value={hex} onchange={onHex} spellcheck="false" maxlength="7" />
  </div>

  <div class="swatches">
    {#each SWATCHES as swatch}
      <button
        class="swatch"
        class:active={toHex(swatch) === hex}
        style:background={toHex(swatch)}
        aria-label={toHex(swatch)}
        onclick={() => set(rgbToHsv(swatch))}
      ></button>
    {/each}
  </div>
</div>

<style>
  .picker {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .sv {
    position: relative;
    height: 130px;
    border-radius: 12px;
    background-image:
      linear-gradient(to top, #000, transparent),
      linear-gradient(to right, #fff, transparent);
    cursor: crosshair;
    box-shadow: inset 0 0 0 0.5px var(--fill-hover);
    touch-action: none;
  }

  .hue {
    position: relative;
    height: 14px;
    border-radius: 7px;
    background: linear-gradient(to right, #f00, #ff0, #0f0, #0ff, #00f, #f0f, #f00);
    cursor: pointer;
    touch-action: none;
  }

  .thumb {
    position: absolute;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    transform: translate(-50%, -50%);
    border: 2.5px solid white;
    box-shadow: 0 1px 6px var(--shadow-strong);
    pointer-events: none;
  }

  .thumb.round {
    top: 50%;
  }

  .row {
    display: flex;
    gap: 10px;
    align-items: center;
  }

  .preview {
    width: 30px;
    height: 30px;
    border-radius: 8px;
    box-shadow: inset 0 0 0 0.5px var(--fill-strong);
    transition: box-shadow 0.3s;
  }

  .hex {
    flex: 1;
    height: 30px;
    padding: 0 10px;
    border-radius: 8px;
    border: 0.5px solid var(--hairline);
    background: var(--fill-subtle);
    font-family: "SF Mono", "Cascadia Mono", Consolas, monospace;
    letter-spacing: 0.04em;
    outline: none;
  }

  .hex:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px rgba(10, 132, 255, 0.25);
  }

  .swatches {
    display: grid;
    grid-template-columns: repeat(10, 1fr);
    gap: 6px;
  }

  .swatch {
    aspect-ratio: 1;
    border-radius: 50%;
    box-shadow: inset 0 0 0 0.5px var(--fill-strong);
    transition: transform 0.2s var(--ease);
  }

  .swatch:hover {
    transform: scale(1.15);
  }

  .swatch.active {
    box-shadow:
      0 0 0 2px var(--bg),
      0 0 0 3.5px white;
  }
</style>
