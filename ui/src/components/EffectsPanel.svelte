<script lang="ts">
  import { fade } from "svelte/transition";
  import type { EffectInfo, EffectState, Rgb } from "../lib/api";
  import { css } from "../lib/color";
  import { effectDescription, effectName } from "../lib/effects";
  import { t } from "../lib/i18n.svelte";
  import ColorPicker from "./ColorPicker.svelte";

  interface Props {
    /** Effects the connected keyboard offers (empty when disconnected). */
    effects: EffectInfo[];
    value: EffectState;
    onchange: (next: EffectState) => void;
  }

  let { effects, value, onchange }: Props = $props();

  const info = $derived(effects.find((e) => e.id === value.id));
  // Until an effect is verified every control is shown, so it can be tested on the keyboard.
  const speedLevels = $derived(info ? info.speed_levels || (info.verified ? 0 : 4) : 0);
  const showDirection = $derived(!!info && (!info.verified || info.directions.length > 0));
  const showColor = $derived(!!info && (!info.verified || info.color !== "none"));
  const showRandom = $derived(!!info && (!info.verified || info.color === "single_or_random"));

  const RAINBOW = "linear-gradient(120deg, #ff453a, #ff9f0a, #ffd60a, #30d158, #64d2ff, #0a84ff, #bf5af2)";

  function swatch(fx: EffectInfo): string {
    if (fx.color === "none" || (fx.id === value.id && value.random)) return RAINBOW;
    const c = fx.id === value.id ? value.color : ([255, 255, 255] as Rgb);
    return `radial-gradient(circle at 30% 40%, ${css(c, 1)}, ${css(c, 0.25)} 70%)`;
  }

  const update = (patch: Partial<EffectState>) => onchange({ ...value, ...patch });
</script>

{#if effects.length === 0}
  <p class="hint left">{t("effects.connect")}</p>
{:else}
  <div class="grid">
    {#each effects as fx (fx.id)}
      <button
        class="card"
        class:active={fx.id === value.id}
        title={fx.verified ? fx.name : t("effects.to_verify", { name: fx.name })}
        onclick={() => update({ id: fx.id })}
      >
        <span class="swatch" style:background={swatch(fx)}></span>
        <span class="name">{effectName(fx.id, fx.name)}</span>
        {#if !fx.verified}<span class="unverified" aria-label={t("effects.to_verify_short")}></span>{/if}
      </button>
    {/each}
  </div>

  {#if info}
    {#if effectDescription(info.id)}
      <p class="description">{effectDescription(info.id)}</p>
    {/if}
    {#if !info.verified}
      <p class="hint left" transition:fade={{ duration: 150 }}>
        {t("effects.unverified")}
      </p>
    {/if}

    {#if speedLevels > 0}
      <div class="param">
        <div class="caption-row">
          <span class="caption">{t("effects.speed")}</span>
          <span class="count">{t("effects.speed_value", { n: value.speed + 1, total: speedLevels })}</span>
        </div>
        <input
          type="range"
          class="slider"
          min="0"
          max={speedLevels - 1}
          step="1"
          value={Math.min(value.speed, speedLevels - 1)}
          onchange={(e) => update({ speed: Number((e.target as HTMLInputElement).value) })}
          style:--fill="{(Math.min(value.speed, speedLevels - 1) / (speedLevels - 1)) * 100}%"
        />
      </div>
    {/if}

    {#if showDirection}
      <div class="param">
        <span class="caption">{t("effects.direction")}</span>
        <div class="segmented" style:--index={value.direction === "left" ? 0 : 1}>
          <div class="indicator"></div>
          <button class:active={value.direction === "left"} onclick={() => update({ direction: "left" })}>
            {t("effects.left")}
          </button>
          <button class:active={value.direction === "right"} onclick={() => update({ direction: "right" })}>
            {t("effects.right")}
          </button>
        </div>
      </div>
    {/if}

    {#if showColor}
      <div class="param">
        <span class="caption">{t("effects.color")}</span>
        {#if showRandom}
          <label class="toggle-row">
            <span>{t("effects.random")}</span>
            <input
              type="checkbox"
              class="toggle"
              checked={value.random}
              onchange={(e) => update({ random: (e.target as HTMLInputElement).checked })}
            />
          </label>
        {/if}
        <div class:muted={value.random && showRandom}>
          <ColorPicker value={value.color} onpick={(color) => update({ color, random: false })} />
        </div>
      </div>
    {/if}
  {/if}
{/if}

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 6px;
  }

  .card {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 7px;
    padding: 8px;
    border-radius: 11px;
    background: var(--fill-subtle);
    border: 0.5px solid var(--hairline);
    text-align: left;
    transition:
      background 0.2s var(--ease),
      border-color 0.2s var(--ease),
      transform 0.15s var(--ease);
  }

  .card:hover {
    background: var(--fill);
  }

  .card:active {
    transform: scale(0.97);
  }

  .card.active {
    background: rgba(10, 132, 255, 0.18);
    border-color: rgba(10, 132, 255, 0.7);
  }

  .swatch {
    width: 100%;
    height: 22px;
    border-radius: 6px;
    box-shadow: inset 0 0 0 0.5px var(--fill-hover);
    opacity: 0.85;
  }

  .card.active .swatch {
    opacity: 1;
  }

  .name {
    font-size: 11.5px;
    font-weight: 500;
    line-height: 1.2;
    color: var(--text-2);
  }

  .card.active .name {
    color: var(--text);
  }

  .unverified {
    position: absolute;
    top: 5px;
    right: 5px;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #ff9f0a;
    box-shadow: 0 0 6px rgba(255, 159, 10, 0.7);
  }

  .description {
    margin: 0;
    color: var(--text-2);
    font-size: 12px;
    line-height: 1.45;
  }

  .param {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .muted {
    opacity: 0.4;
    transition: opacity 0.2s var(--ease);
  }
</style>
