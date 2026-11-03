<script lang="ts">
  import { slide } from "svelte/transition";
  import type { LayoutSummary, Support, UnknownKeyboard } from "../lib/api";
  import { t } from "../lib/i18n.svelte";

  interface Props {
    support: Support;
    /** The keyboard without a model, when `support` is "unconfigured". */
    unknown: UnknownKeyboard | null;
    /** Name of the model that is not verified. */
    modelName: string;
    /** Set when a verified model is drawn in a shape it wasn't verified in (e.g. ANSI). */
    shape: { verified: string; standard: string } | null;
    /** Formats that can draw a keyboard without a model. */
    formats: LayoutSummary[];
    formatId: string | null;
    onformat: (id: string) => void;
  }

  let { support, unknown, modelName, shape, formats, formatId, onformat }: Props = $props();

  const unconfigured = $derived(support === "unconfigured");
  const name = $derived(unknown ? [unknown.manufacturer, unknown.product].filter(Boolean).join(" ") : modelName);
</script>

<div class="banner" class:warn={unconfigured} role="status" transition:slide={{ duration: 220 }}>
  <svg class="icon" viewBox="0 0 20 20" aria-hidden="true">
    <circle cx="10" cy="10" r="8.25" />
    <path d="M10 5.75v5.5M10 14.2v.05" />
  </svg>
  <div class="text">
    <div class="title">
      {unconfigured ? t("support.unconfigured_title") : t("support.unverified_title")}
      {#if unknown}<span class="ids">USB {unknown.vid}:{unknown.pid}</span>{/if}
    </div>
    <p>
      {unconfigured
        ? t("support.unconfigured_body", { name })
        : shape
          ? t("support.unverified_shape_body", { name, ...shape })
          : t("support.unverified_body", { name })}
    </p>
    {#if unconfigured}
      <p class="guide">{t("support.guide")}</p>
    {/if}
  </div>
  {#if unconfigured && formats.length}
    <label class="format">
      <span>{t("support.format")}</span>
      <select class="popup" value={formatId} onchange={(e) => onformat((e.target as HTMLSelectElement).value)}>
        {#each formats as f (f.id)}
          <option value={f.id}>{f.name}</option>
        {/each}
      </select>
    </label>
  {/if}
</div>

<style>
  .banner {
    display: flex;
    align-items: center;
    gap: 14px;
    margin: 4px 0 10px;
    padding: 12px 16px;
    border-radius: 14px;
    background: var(--surface);
    border: 0.5px solid var(--hairline);
    backdrop-filter: blur(30px) saturate(1.6);
  }

  .icon {
    flex: none;
    width: 22px;
    height: 22px;
    fill: none;
    stroke: var(--blue-text);
    stroke-width: 1.5;
    stroke-linecap: round;
  }

  .warn .icon {
    stroke: var(--orange-text);
  }

  .text {
    flex: 1;
    min-width: 0;
  }

  .title {
    display: flex;
    align-items: baseline;
    gap: 8px;
    font-weight: 600;
  }

  .ids {
    color: var(--text-3);
    font-size: 11px;
    font-weight: 500;
    font-variant-numeric: tabular-nums;
  }

  p {
    margin: 3px 0 0;
    color: var(--text-2);
    font-size: 12px;
    line-height: 1.45;
  }

  .guide {
    color: var(--text-3);
  }

  .format {
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 4px;
    color: var(--text-2);
    font-size: 11px;
    font-weight: 600;
  }

  .popup {
    height: 28px;
    padding: 0 28px 0 11px;
    border-radius: 8px;
    border: 0.5px solid var(--hairline-strong);
    background:
      url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 10 14'%3E%3Cpath d='M2 5.5l3-3 3 3M2 8.5l3 3 3-3' fill='none' stroke='%238e8e93' stroke-width='1.4' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E")
        no-repeat right 9px center / 9px 13px,
      var(--fill);
    appearance: none;
    color: var(--text);
    font: inherit;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    outline: none;
    transition: background-color 0.2s var(--ease);
  }

  .popup:hover {
    background-color: var(--fill-hover);
  }

  .popup:focus-visible {
    box-shadow: 0 0 0 3px rgba(10, 132, 255, 0.35);
  }

  .popup option {
    background: var(--option-bg);
    color: var(--text);
  }
</style>
