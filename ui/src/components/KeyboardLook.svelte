<script lang="ts">
  import { scale } from "svelte/transition";
  import { fromHex, toHex } from "../lib/color";
  import { t } from "../lib/i18n.svelte";
  import { CASE_PRESETS, KEY_PRESETS, look, setLook, type KeyboardLook } from "../lib/look.svelte";
  import ColorPicker from "./ColorPicker.svelte";

  /** Which color the popover edits. */
  let open = $state<keyof KeyboardLook | null>(null);

  const presets = $derived(open === "case" ? CASE_PRESETS : KEY_PRESETS);
  const value = $derived(open ? look[open] : "#000000");
  const custom = $derived(!presets.some((p) => p.hex.toLowerCase() === value.toLowerCase()));

  const toggle = (part: keyof KeyboardLook) => (open = open === part ? null : part);
  const choose = (hex: string) => open && setLook({ [open]: hex });
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && (open = null)} />

<div class="look">
  {#each ["case", "keys"] as const as part (part)}
    <button class="pill" class:active={open === part} onclick={() => toggle(part)}>
      <span class="dot" style:background={look[part]}></span>
      {t(`look.${part}`)}
    </button>
  {/each}

  {#if open}
    <div class="backdrop" role="presentation" onclick={() => (open = null)}></div>
    <div class="popover" transition:scale={{ start: 0.96, duration: 180 }}>
      <div class="title">{t(`look.${open}_title`)}</div>
      <div class="presets">
        {#each presets as p (p.id)}
          <button class="preset" class:active={p.hex.toLowerCase() === value.toLowerCase()} onclick={() => choose(p.hex)}>
            <span class="swatch" style:background={p.hex}></span>
            {t(`look.${p.id}`)}
          </button>
        {/each}
        <span class="preset" class:active={custom}>
          <span class="swatch rainbow"></span>
          {t("look.custom")}
        </span>
      </div>
      <ColorPicker value={fromHex(value) ?? [0, 0, 0]} onpick={(rgb) => choose(toHex(rgb))} />
      <p class="hint left">{t("look.hint")}</p>
    </div>
  {/if}
</div>

<style>
  .look {
    position: absolute;
    left: 6px;
    bottom: 8px;
    display: flex;
    gap: 6px;
    z-index: 5;
  }

  .pill {
    display: flex;
    align-items: center;
    gap: 7px;
    height: 28px;
    padding: 0 12px 0 9px;
    border-radius: 14px;
    background: var(--surface);
    border: 0.5px solid var(--hairline);
    backdrop-filter: blur(20px);
    color: var(--text-2);
    font-size: 12px;
    font-weight: 500;
    transition:
      background 0.2s var(--ease),
      color 0.2s var(--ease);
  }

  .pill:hover,
  .pill.active {
    background: var(--surface-strong);
    color: var(--text);
  }

  .dot {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    box-shadow: inset 0 0 0 0.5px var(--fill-strong);
  }

  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 19;
  }

  .popover {
    position: absolute;
    left: 0;
    bottom: 38px;
    z-index: 20;
    display: flex;
    flex-direction: column;
    gap: 12px;
    width: 280px;
    padding: 16px;
    border-radius: 16px;
    background: var(--panel);
    border: 0.5px solid var(--hairline-strong);
    backdrop-filter: blur(40px);
    box-shadow: 0 24px 60px var(--shadow-strong);
    transform-origin: bottom left;
  }

  .title {
    font-weight: 700;
    font-size: 14px;
  }

  .presets {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 6px;
  }

  .preset {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 8px 4px;
    border-radius: 10px;
    color: var(--text-2);
    font-size: 11px;
    transition: background 0.2s var(--ease);
  }

  button.preset:hover {
    background: var(--fill-subtle);
  }

  .preset.active {
    background: var(--fill);
    color: var(--text);
  }

  .swatch {
    width: 26px;
    height: 26px;
    border-radius: 50%;
    box-shadow:
      inset 0 0 0 0.5px var(--fill-strong),
      0 2px 6px var(--shadow-strong);
  }

  .preset.active .swatch {
    box-shadow:
      0 0 0 2px var(--panel),
      0 0 0 4px var(--accent);
  }

  .rainbow {
    background: conic-gradient(#ff453a, #ffd60a, #30d158, #64d2ff, #5e5ce6, #bf5af2, #ff453a);
  }
</style>
