<script lang="ts">
  import { slide } from "svelte/transition";
  import { plural, t } from "../lib/i18n.svelte";
  import type { Macro } from "../lib/keys";

  interface Props {
    macros: Macro[];
    /** Labels of the keys playing each macro, by macro index. */
    usedBy: string[][];
    /** Most macros the keyboard holds. */
    max: number;
    onnew: () => void;
    onedit: (index: number) => void;
  }

  let { macros, usedBy, max, onnew, onedit }: Props = $props();
</script>

<aside class="macros">
  <div class="head">
    <span class="caption">{t("macros.mine")}</span>
    <button class="icon" title={t("macros.new")} disabled={macros.length >= max} onclick={onnew}>
      <svg viewBox="0 0 16 16"><path d="M8 3v10M3 8h10" /></svg>
    </button>
  </div>

  {#each macros as m, i (i)}
    <div
      class="item"
      role="button"
      tabindex="0"
      onclick={() => onedit(i)}
      onkeydown={(e) => e.key === "Enter" && onedit(i)}
      transition:slide={{ duration: 200 }}
    >
      <span class="glyph">
        <svg viewBox="0 0 16 16"><path d="M3 4.5h10M3 8h6.5M3 11.5h4M11 9.5l2.5 2-2.5 2" /></svg>
      </span>
      <div class="meta">
        <span class="name">{m.name}</span>
        <span class="sub">
          {plural("macros.steps", m.steps.length)}
          {#if usedBy[i]?.length}
            · {t("macros.on_keys", { keys: usedBy[i].join(", ") })}
          {:else}
            · {t("macros.unassigned")}
          {/if}
        </span>
      </div>
    </div>
  {:else}
    <p class="empty">{t("macros.empty")}</p>
  {/each}

  <p class="foot">{t("macros.capacity", { n: macros.length, max })}</p>
</aside>

<style>
  .macros {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 14px 12px;
    overflow-y: auto;
    border-radius: 20px;
    background: var(--surface);
    border: 0.5px solid var(--hairline);
    backdrop-filter: blur(40px) saturate(1.6);
    box-shadow: 0 20px 50px -20px var(--shadow-strong);
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 6px 4px;
  }

  .item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 8px;
    border-radius: 12px;
    cursor: pointer;
    outline: none;
    transition: background 0.2s var(--ease);
  }

  .item:hover,
  .item:focus-visible {
    background: var(--fill-subtle);
  }

  .glyph {
    flex: none;
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: 9px;
    background: rgba(191, 90, 242, 0.18);
    color: var(--purple-text);
  }

  .glyph svg,
  .icon svg {
    width: 15px;
    height: 15px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.4;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .meta {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .name {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sub {
    color: var(--text-2);
    font-size: 11.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .icon {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 7px;
    color: var(--text-2);
    transition:
      background 0.2s var(--ease),
      color 0.2s var(--ease);
  }

  .icon:hover:not(:disabled) {
    background: var(--fill-hover);
    color: var(--text);
  }

  .icon:disabled {
    opacity: 0.3;
  }

  .empty {
    margin: 4px 6px;
    color: var(--text-2);
    font-size: 12px;
    line-height: 1.45;
  }

  .foot {
    margin: auto 6px 0;
    padding-top: 10px;
    color: var(--text-3);
    font-size: 11px;
  }
</style>
