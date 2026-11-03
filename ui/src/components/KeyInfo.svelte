<script lang="ts">
  import type { Rgb } from "../lib/api";
  import { css, rgbToHsv, toHex } from "../lib/color";
  import { t } from "../lib/i18n.svelte";

  interface Props {
    label: string;
    color: Rgb;
    /** True when the key has its own color, false when it shows the background. */
    custom: boolean;
    /** 0..100 */
    intensity: number;
    onuse: () => void;
  }

  let { label, color, custom, intensity, onuse }: Props = $props();

  const hex = $derived(toHex(color));
  const value = $derived(Math.round(rgbToHsv(color).v * 100));
  let copied = $state(false);

  async function copy() {
    try {
      await navigator.clipboard.writeText(hex);
      copied = true;
      setTimeout(() => (copied = false), 1200);
    } catch {
      // Clipboard can be unavailable; the value is still visible on screen.
    }
  }
</script>

<div class="card">
  <div class="swatch" style:background={hex} style:--glow={css(color, 0.5)}></div>
  <div class="info">
    <div class="title">
      <span class="key">{label || t("common.space")}</span>
      <span class="origin" class:custom>{custom ? t("key.custom") : t("key.background")}</span>
    </div>
    <button class="hex" title={t("common.copy")} onclick={copy}>{copied ? t("common.copied") : hex}</button>
    <div class="values">
      <span>R {color[0]}</span><span>G {color[1]}</span><span>B {color[2]}</span>
    </div>
    <div class="values">
      <span title={t("key.color_value_hint")}>{t("key.color_value", { n: value })}</span>
      <span title={t("key.on_keyboard_hint")}>{t("key.on_keyboard", { n: Math.round((value * intensity) / 100) })}</span>
    </div>
  </div>
</div>
<button class="btn wide" onclick={onuse}>
  <svg viewBox="0 0 16 16"
    ><path d="M10.5 2.5l3 3-1.5 1.5-3-3zM9 4l3 3-6 6H3v-3z" /></svg
  >
  {t("key.use")}
</button>

<style>
  .card {
    display: flex;
    gap: 12px;
    align-items: stretch;
  }

  .swatch {
    flex: none;
    width: 64px;
    border-radius: 14px;
    box-shadow:
      inset 0 0 0 0.5px var(--fill-strong),
      0 8px 22px -6px var(--glow);
  }

  .info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .title {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .key {
    font-size: 15px;
    font-weight: 700;
  }

  .origin {
    padding: 1px 7px;
    border-radius: 6px;
    background: var(--fill);
    color: var(--text-2);
    font-size: 11px;
    font-weight: 600;
  }

  .origin.custom {
    background: rgba(10, 132, 255, 0.22);
    color: #64b5ff;
  }

  .hex {
    align-self: flex-start;
    padding: 0;
    font-family: "SF Mono", "Cascadia Mono", Consolas, monospace;
    font-size: 14px;
    letter-spacing: 0.04em;
    color: var(--text);
    border-bottom: 1px dashed var(--text-3);
  }

  .hex:hover {
    color: var(--accent);
    border-color: var(--accent);
  }

  .values {
    display: flex;
    gap: 10px;
    color: var(--text-2);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
  }

  .wide {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    width: 100%;
  }

  .wide svg {
    width: 14px;
    height: 14px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.4;
    stroke-linejoin: round;
  }
</style>
