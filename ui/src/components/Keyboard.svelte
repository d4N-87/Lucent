<script lang="ts">
  import type { KeyDef, Layout, Rgb } from "../lib/api";
  import { css, isOff, toHex } from "../lib/color";
  import { t } from "../lib/i18n.svelte";
  import type { SvelteSet } from "svelte/reactivity";
  import { isLight, look } from "../lib/look.svelte";

  interface Props {
    layout: Layout;
    colors: Rgb[];
    selected: SvelteSet<number>;
    /** 0..1 */
    intensity: number;
    /** Alt+click: pick the color of a key. */
    oneyedrop: (slot: number) => void;
    /** When set (live effect preview), clicking a key presses it instead of selecting it. */
    onpress?: (slot: number) => void;
    /** Text shown instead of the printed label, by slot (key functions). */
    legends?: Record<number, string>;
    /** A small dot on the keycap, by slot: "key", "media", "macro" or "disabled". */
    marks?: Record<number, string>;
  }

  let { layout, colors, selected, intensity, oneyedrop, onpress, legends, marks }: Props = $props();

  let frameWidth = $state(0);
  let frameHeight = $state(0);

  // Padding around the keys, in key units.
  const PAD = 0.5;
  const unit = $derived(
    Math.max(
      24,
      Math.min(frameWidth / (layout.width + 2 * PAD), frameHeight / (layout.height + 2 * PAD)),
    ),
  );

  // Drag-to-select: pressing a key starts a selection, entering others extends it.
  let dragging = false;

  function onKeyDown(e: PointerEvent, key: KeyDef) {
    e.stopPropagation();
    if (onpress) {
      onpress(key.slot);
      return;
    }
    if (e.altKey) {
      oneyedrop(key.slot);
      return;
    }
    const additive = e.ctrlKey || e.shiftKey || e.metaKey;
    if (additive) {
      if (selected.has(key.slot)) selected.delete(key.slot);
      else selected.add(key.slot);
    } else {
      selected.clear();
      selected.add(key.slot);
    }
    dragging = true;
  }

  function onKeyEnter(key: KeyDef) {
    if (dragging) selected.add(key.slot);
  }

  function onBoardDown() {
    selected.clear();
  }

  // The glow follows the intensity so the preview roughly matches the keyboard.
  const glow = $derived(intensity === 0 ? 0 : 0.3 + intensity * 0.7);
</script>

<svelte:window onpointerup={() => (dragging = false)} />

<div class="frame" bind:clientWidth={frameWidth} bind:clientHeight={frameHeight}>
  <div
    class="board"
    class:live={!!onpress}
    class:light-caps={isLight(look.keys)}
    role="presentation"
    style:--case={look.case}
    style:--keycap={look.keys}
    style:width="{(layout.width + 2 * PAD) * unit}px"
    style:height="{(layout.height + 2 * PAD) * unit}px"
    style:--u="{unit}px"
    onpointerdown={onBoardDown}
  >
    {#each layout.keys as key (key.slot)}
      {@const color = colors[key.slot]}
      {@const off = isOff(color) || glow === 0}
      {@const legend = legends?.[key.slot] ?? key.label}
      {@const mark = marks?.[key.slot]}
      <button
        class="key"
        class:off
        class:selected={selected.has(key.slot)}
        style:left="{(key.x + PAD) * unit}px"
        style:top="{(key.y + PAD) * unit}px"
        style:width="{key.w * unit}px"
        style:height="{key.h * unit}px"
        style:--led={css(color)}
        style:--led-soft={css(color, 0.55 * glow)}
        style:--led-faint={css(color, 0.18 * glow)}
        onpointerdown={(e) => onKeyDown(e, key)}
        onpointerenter={() => onKeyEnter(key)}
        title={legends
          ? `${key.label || t("common.space")}${legend !== key.label ? ` → ${legend}` : ""}`
          : `${key.label || t("common.space")} · ${toHex(color)}`}
      >
        <span class="cap">
          <span class="legend" class:small={legend.length > 5}>{legend}</span>
          {#if mark}<span class="mark {mark}"></span>{/if}
        </span>
      </button>
    {/each}
  </div>
</div>

<style>
  .frame {
    position: relative;
    width: 100%;
    height: 100%;
    display: grid;
    place-items: center;
  }

  .board {
    position: relative;
    border-radius: calc(var(--u) * 0.45);
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.05), rgba(255, 255, 255, 0) 30%),
      linear-gradient(180deg, var(--case), color-mix(in srgb, var(--case), black 38%));
    box-shadow:
      0 0 0 0.5px rgba(255, 255, 255, 0.1),
      inset 0 1px 0 rgba(255, 255, 255, 0.08),
      0 40px 80px -20px rgba(0, 0, 0, 0.8),
      0 18px 36px -18px rgba(0, 0, 0, 0.9);
  }

  .key {
    position: absolute;
    padding: calc(var(--u) * 0.045);
    border-radius: calc(var(--u) * 0.16);
    /* Underglow: light leaking around the keycap */
    background: var(--led-faint);
    box-shadow: 0 0 calc(var(--u) * 0.32) var(--led-soft);
    transition:
      box-shadow 0.35s var(--ease),
      background 0.35s var(--ease),
      transform 0.18s var(--ease);
  }

  /* The live preview changes every frame: easing would smear it. */
  .board.live .key,
  .board.live .legend {
    transition: transform 0.18s var(--ease);
  }

  .key.off {
    background: transparent;
    box-shadow: none;
  }

  .cap {
    display: grid;
    place-items: center;
    width: 100%;
    height: 100%;
    border-radius: calc(var(--u) * 0.13);
    /* Keycap color from the preview look (lib/look.svelte.ts), lit from above. */
    background: linear-gradient(
      180deg,
      color-mix(in srgb, var(--keycap), white 9%) 0%,
      var(--keycap) 55%,
      color-mix(in srgb, var(--keycap), black 22%) 100%
    );
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.09),
      inset 0 calc(var(--u) * -0.06) 0 rgba(0, 0, 0, 0.35),
      0 1px 2px rgba(0, 0, 0, 0.6);
    transition: filter 0.2s var(--ease);
  }

  .legend {
    font-size: calc(var(--u) * 0.24);
    font-weight: 600;
    color: var(--led);
    text-shadow: 0 0 calc(var(--u) * 0.12) var(--led-soft);
    transition: color 0.35s var(--ease);
    pointer-events: none;
  }

  .legend.small {
    font-size: calc(var(--u) * 0.17);
  }

  .key.off .legend {
    color: rgba(255, 255, 255, 0.28);
    text-shadow: none;
  }

  .cap {
    position: relative;
  }

  .mark {
    position: absolute;
    top: calc(var(--u) * 0.08);
    right: calc(var(--u) * 0.08);
    width: calc(var(--u) * 0.1);
    height: calc(var(--u) * 0.1);
    border-radius: 50%;
    background: var(--led);
    box-shadow: 0 0 calc(var(--u) * 0.1) var(--led);
  }

  .mark.disabled {
    background: rgba(255, 255, 255, 0.35);
    box-shadow: none;
  }

  /* Backlit legends on light keycaps: an outline keeps them readable. */
  .board.light-caps .legend {
    text-shadow:
      0 0 1px rgba(0, 0, 0, 0.6),
      0 0 calc(var(--u) * 0.12) var(--led-soft);
  }

  .board.light-caps .key.off .legend {
    color: rgba(0, 0, 0, 0.35);
    text-shadow: none;
  }

  .key:hover .cap {
    filter: brightness(1.25);
  }

  .key:active {
    transform: scale(0.96);
  }

  .key.selected {
    transform: translateY(calc(var(--u) * -0.04));
  }

  .key.selected .cap {
    box-shadow:
      0 0 0 2px var(--accent),
      0 0 0 4px rgba(10, 132, 255, 0.25),
      inset 0 1px 0 rgba(255, 255, 255, 0.12);
  }
</style>
