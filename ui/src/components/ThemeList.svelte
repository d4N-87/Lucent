<script lang="ts">
  import { slide } from "svelte/transition";
  import type { Layout, Lighting, Theme } from "../lib/api";
  import Thumb from "./Thumb.svelte";
  import { t } from "../lib/i18n.svelte";

  interface Props {
    layout: Layout;
    presets: { id: string; name: string; lighting: Lighting }[];
    themes: Theme[];
    activeId: string | null;
    /** The active profile's colors differ from the active theme. */
    dirty: boolean;
    onselect: (id: string) => void;
    /** Asks for a name and saves the current lighting as a new theme. */
    onnew: () => void;
    onrename: (id: string, name: string) => void;
    onduplicate: (id: string) => void;
    ondelete: (id: string) => void;
  }

  let { layout, presets, themes, activeId, dirty, onselect, onnew, onrename, onduplicate, ondelete }: Props =
    $props();

  let renamingId = $state<string | null>(null);
  let renameValue = $state("");
  let confirmDeleteId = $state<string | null>(null);

  function startRename(p: Theme) {
    renamingId = p.id;
    renameValue = p.name;
  }

  function commitRename() {
    const name = renameValue.trim();
    if (renamingId && name) onrename(renamingId, name);
    renamingId = null;
  }

  function onRenameKey(e: KeyboardEvent) {
    if (e.key === "Enter") commitRename();
    if (e.key === "Escape") renamingId = null;
  }

  function autofocus(node: HTMLInputElement) {
    node.focus();
    node.select();
  }

  /** Runs `fn` without also selecting the theme row underneath. */
  const only = (fn: () => void) => (e: Event) => {
    e.stopPropagation();
    fn();
  };
</script>

{#snippet row(id: string, name: string, lighting: Lighting, theme: Theme | null)}
  <div
    class="item"
    class:active={id === activeId}
    role="button"
    tabindex="0"
    onclick={() => onselect(id)}
    onkeydown={(e) => e.key === "Enter" && onselect(id)}
    transition:slide={{ duration: 200 }}
  >
    <Thumb {layout} {lighting} />
    <div class="meta">
      {#if theme && renamingId === id}
        <input
          class="rename"
          bind:value={renameValue}
          onclick={(e) => e.stopPropagation()}
          onkeydown={onRenameKey}
          onblur={commitRename}
          use:autofocus
        />
      {:else}
        <span class="name" role="presentation" ondblclick={() => theme && startRename(theme)}>
          {#if id === activeId && dirty}<span class="unsaved" title={t("themes.unsaved")}></span>{/if}
          {name}
        </span>
      {/if}

      {#if theme}
        {#if confirmDeleteId === id}
          <div class="confirm">
            <button class="danger" onclick={only(() => ((confirmDeleteId = null), ondelete(id)))}>{t("themes.delete")}</button>
            <button onclick={only(() => (confirmDeleteId = null))}>{t("common.cancel")}</button>
          </div>
        {:else}
          <div class="tools">
            <button class="icon" title={t("themes.rename")} onclick={only(() => startRename(theme))}>
              <svg viewBox="0 0 16 16"><path d="M10.5 3.5l2 2L6 12H4v-2z" /></svg>
            </button>
            <button class="icon" title={t("themes.duplicate")} onclick={only(() => onduplicate(id))}>
              <svg viewBox="0 0 16 16"
                ><rect x="5.5" y="5.5" width="7" height="7" rx="1.5" /><path d="M3.5 10V4.5a1 1 0 011-1H10" /></svg
              >
            </button>
            <button class="icon" title={t("themes.delete")} onclick={only(() => (confirmDeleteId = id))}>
              <svg viewBox="0 0 16 16"><path d="M4 5h8M6.5 5V3.5h3V5M5 5l.6 7.5h4.8L11 5" /></svg>
            </button>
          </div>
        {/if}
      {/if}
    </div>
  </div>
{/snippet}

<aside class="themes">
  <div class="head">
    <span class="caption">{t("themes.mine")}</span>
    <button class="icon" title={t("themes.new")} onclick={onnew}>
      <svg viewBox="0 0 16 16"><path d="M8 3v10M3 8h10" /></svg>
    </button>
  </div>

  {#each themes as p (p.id)}
    {@render row(p.id, p.name, p, p)}
  {:else}
    <p class="empty">{t("themes.empty")}</p>
  {/each}

  <div class="head">
    <span class="caption">{t("themes.presets")}</span>
  </div>

  {#each presets as p (p.id)}
    {@render row(p.id, p.name, p.lighting, null)}
  {/each}
</aside>

<style>
  .themes {
    display: flex;
    flex-direction: column;
    gap: 6px;
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

  .head:not(:first-child) {
    margin-top: 10px;
  }

  .item {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px;
    border-radius: 12px;
    cursor: pointer;
    outline: none;
    transition: background 0.2s var(--ease);
  }

  .item:hover,
  .item:focus-visible {
    background: var(--fill-subtle);
  }

  .item.active {
    background: rgba(10, 132, 255, 0.18);
    box-shadow: inset 0 0 0 1px rgba(10, 132, 255, 0.55);
  }

  .meta {
    display: flex;
    align-items: center;
    justify-content: space-between;
    min-height: 22px;
    gap: 6px;
  }

  .name {
    display: flex;
    align-items: center;
    gap: 6px;
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .unsaved {
    flex: none;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #ff9f0a;
    box-shadow: 0 0 6px #ff9f0a;
  }

  .tools {
    display: flex;
    gap: 2px;
    opacity: 0;
    transition: opacity 0.2s var(--ease);
  }

  .item:hover .tools,
  .item.active .tools {
    opacity: 1;
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

  .icon:hover {
    background: var(--fill-hover);
    color: var(--text);
  }

  .icon svg {
    width: 14px;
    height: 14px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.4;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .rename {
    width: 100%;
    height: 26px;
    padding: 0 8px;
    border-radius: 7px;
    border: 0.5px solid var(--accent);
    background: var(--fill);
    box-shadow: 0 0 0 3px rgba(10, 132, 255, 0.25);
    outline: none;
  }

  .confirm {
    display: flex;
    gap: 4px;
  }

  .confirm button {
    height: 22px;
    padding: 0 8px;
    border-radius: 6px;
    font-size: 11px;
    font-weight: 600;
    background: var(--fill);
  }

  .confirm .danger {
    background: var(--red);
    color: white;
  }

  .empty {
    margin: 0;
    padding: 4px 8px 8px;
    color: var(--text-3);
    font-size: 12px;
    line-height: 1.4;
  }
</style>
