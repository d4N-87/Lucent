<script lang="ts">
  import { tick } from "svelte";
  import { slide, scale } from "svelte/transition";
  import type { Layout, UserProfile } from "../lib/api";
  import Thumb from "./Thumb.svelte";
  import { t } from "../lib/i18n.svelte";

  interface Props {
    layout: Layout;
    profiles: UserProfile[];
    activeId: string | null;
    /** Second line of each profile: its theme and how many keys it changes. */
    details: Record<string, string>;
    onselect: (id: string) => void;
    /** A new profile: a copy of `from`, or factory keys with the current colors when `null`. */
    oncreate: (name: string, from: string | null) => void;
    onrename: (id: string, name: string) => void;
    ondelete: (id: string) => void;
    /** Saves the profile as a file to share or keep. */
    onexport: (id: string) => void;
    /** Adds the profile in a file picked by the user, without making it active. */
    onimport: (file: File) => void;
  }

  let { layout, profiles, activeId, details, onselect, oncreate, onrename, ondelete, onexport, onimport }: Props =
    $props();
  let fileInput = $state<HTMLInputElement | undefined>();

  let open = $state(false);
  /** Name field at the bottom, for a new profile (`from` null) or a duplicate. */
  let creating = $state<{ from: string | null } | null>(null);
  let newName = $state("");
  let nameInput = $state<HTMLInputElement | undefined>();
  let renamingId = $state<string | null>(null);
  let renameValue = $state("");
  let confirmDeleteId = $state<string | null>(null);

  const active = $derived(profiles.find((p) => p.id === activeId) ?? null);

  function close() {
    open = false;
    creating = null;
    renamingId = null;
    confirmDeleteId = null;
  }

  function choose(id: string) {
    if (renamingId || confirmDeleteId) return;
    onselect(id);
    close();
  }

  async function startCreating(from: string | null) {
    const source = profiles.find((p) => p.id === from);
    newName = source
      ? t("profiles.duplicate_name", { name: source.name })
      : t("profiles.new_default", { n: profiles.length + 1 });
    creating = { from };
    await tick();
    nameInput?.focus();
    nameInput?.select();
  }

  function commitCreating() {
    const name = newName.trim();
    if (!creating || !name) return;
    oncreate(name, creating.from);
    close();
  }

  function startRename(p: UserProfile) {
    confirmDeleteId = null;
    renamingId = p.id;
    renameValue = p.name;
  }

  function commitRename() {
    const name = renameValue.trim();
    if (renamingId && name) onrename(renamingId, name);
    renamingId = null;
  }

  function autofocus(node: HTMLInputElement) {
    node.focus();
    node.select();
  }

  /** Runs `fn` without also choosing the profile row underneath. */
  const only = (fn: () => void) => (e: Event) => {
    e.stopPropagation();
    fn();
  };
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && open && close()} />

<div class="wrap">
  <input
    bind:this={fileInput}
    type="file"
    accept=".json,application/json"
    hidden
    onchange={(e) => {
      const input = e.target as HTMLInputElement;
      const file = input.files?.[0];
      input.value = "";
      if (file) {
        onimport(file);
        close();
      }
    }}
  />
  <button class="trigger" class:active={open} onclick={() => (open ? close() : (open = true))} title={t("profiles.title")}>
    <svg class="stack" viewBox="0 0 16 16" aria-hidden="true">
      <path d="M8 2.5l5.5 2.75L8 8 2.5 5.25zM2.5 8L8 10.75 13.5 8M2.5 10.75L8 13.5l5.5-2.75" />
    </svg>
    <span class="current" title={active?.name}>{active?.name ?? t("profiles.title")}</span>
    <svg class="chevron" viewBox="0 0 16 16" aria-hidden="true"><path d="M4.5 6.5L8 10l3.5-3.5" /></svg>
  </button>

  {#if open}
    <div class="backdrop" role="presentation" onclick={close}></div>
    <div class="panel" role="menu" transition:scale={{ start: 0.96, duration: 180 }}>
      <div class="caption">{t("profiles.title")}</div>

      <div class="list">
        {#each profiles as p (p.id)}
          <div
            class="item"
            class:active={p.id === activeId}
            role="menuitemradio"
            aria-checked={p.id === activeId}
            tabindex="0"
            onclick={() => choose(p.id)}
            onkeydown={(e) => e.key === "Enter" && choose(p.id)}
            transition:slide={{ duration: 200 }}
          >
            <div class="thumb"><Thumb {layout} lighting={p.lighting} /></div>
            <div class="meta">
              {#if renamingId === p.id}
                <input
                  class="field"
                  bind:value={renameValue}
                  onclick={(e) => e.stopPropagation()}
                  onkeydown={(e) => {
                    e.stopPropagation();
                    if (e.key === "Enter") commitRename();
                    if (e.key === "Escape") renamingId = null;
                  }}
                  onblur={commitRename}
                  use:autofocus
                />
              {:else}
                <span class="name">{p.name}</span>
                {#if confirmDeleteId === p.id}
                  <div class="confirm">
                    <span>{t("profiles.delete_question")}</span>
                    <button class="danger" onclick={only(() => ((confirmDeleteId = null), ondelete(p.id)))}>
                      {t("profiles.delete")}
                    </button>
                    <button onclick={only(() => (confirmDeleteId = null))}>{t("common.cancel")}</button>
                  </div>
                {:else}
                  <span class="sub">{details[p.id] ?? ""}</span>
                {/if}
              {/if}
            </div>
            {#if renamingId !== p.id && confirmDeleteId !== p.id}
              <div class="tools">
                <button class="icon" title={t("profiles.rename")} onclick={only(() => startRename(p))}>
                  <svg viewBox="0 0 16 16"><path d="M10.5 3.5l2 2L6 12H4v-2z" /></svg>
                </button>
                <button class="icon" title={t("profiles.export")} onclick={only(() => (onexport(p.id), close()))}>
                  <svg viewBox="0 0 16 16"><path d="M8 2.5v7M5.5 5L8 2.5 10.5 5M4 8.5v4h8v-4" /></svg>
                </button>
                {#if profiles.length > 1}
                  <button
                    class="icon"
                    title={t("profiles.delete")}
                    onclick={only(() => ((renamingId = null), (confirmDeleteId = p.id)))}
                  >
                    <svg viewBox="0 0 16 16"><path d="M4 5h8M6.5 5V3.5h3V5M5 5l.6 7.5h4.8L11 5" /></svg>
                  </button>
                {/if}
              </div>
              <svg class="check" viewBox="0 0 16 16" aria-hidden="true"><path d="M3.5 8.5l3 3 6-7" /></svg>
            {/if}
          </div>
        {/each}
      </div>

      <div class="sep"></div>

      {#if creating}
        <div class="create" transition:slide={{ duration: 180 }}>
          <input
            class="field"
            bind:this={nameInput}
            bind:value={newName}
            placeholder={t("profiles.name_placeholder")}
            onkeydown={(e) => {
              e.stopPropagation();
              if (e.key === "Enter") commitCreating();
              if (e.key === "Escape") creating = null;
            }}
          />
          <p class="hint">{creating.from ? t("profiles.duplicate_hint") : t("profiles.new_hint")}</p>
          <div class="buttons">
            <button class="btn" onclick={() => (creating = null)}>{t("common.cancel")}</button>
            <button class="btn primary" disabled={!newName.trim()} onclick={commitCreating}>{t("profiles.create")}</button>
          </div>
        </div>
      {:else}
        <button class="command" onclick={() => startCreating(null)}>
          <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M8 3v10M3 8h10" /></svg>
          {t("profiles.new")}
        </button>
        {#if active}
          <button class="command" onclick={() => startCreating(active.id)}>
            <svg viewBox="0 0 16 16" aria-hidden="true"
              ><rect x="5.5" y="5.5" width="7" height="7" rx="1.5" /><path d="M3.5 10V4.5a1 1 0 011-1H10" /></svg
            >
            {t("profiles.duplicate", { name: active.name })}
          </button>
        {/if}
        <button class="command" onclick={() => fileInput?.click()}>
          <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M8 9.5v-7M5.5 7L8 9.5 10.5 7M4 8.5v4h8v-4" /></svg>
          {t("profiles.import")}
        </button>
      {/if}
    </div>
  {/if}
</div>

<style>
  .wrap {
    position: relative;
    /* Shrinks (the name gets an ellipsis) before the header runs into the section switch. */
    display: flex;
    min-width: 0;
  }

  .trigger {
    display: flex;
    align-items: center;
    gap: 7px;
    height: 30px;
    min-width: 0;
    max-width: 240px;
    padding: 0 10px 0 11px;
    border-radius: 15px;
    background: var(--surface);
    border: 0.5px solid var(--hairline);
    backdrop-filter: blur(20px);
    color: var(--text);
    font-weight: 600;
    transition: background 0.2s var(--ease);
  }

  .trigger:hover,
  .trigger.active {
    background: var(--surface-strong);
  }

  .trigger svg {
    flex: none;
    fill: none;
    stroke: currentColor;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .stack {
    width: 15px;
    height: 15px;
    stroke-width: 1.3;
    color: var(--accent);
  }

  .chevron {
    width: 12px;
    height: 12px;
    stroke-width: 1.6;
    color: var(--text-2);
    transition: transform 0.25s var(--ease);
  }

  .trigger.active .chevron {
    transform: rotate(180deg);
  }

  .current {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 19;
  }

  .panel {
    position: absolute;
    top: 40px;
    left: 0;
    z-index: 20;
    width: 340px;
    max-height: calc(100vh - 80px);
    overflow-y: auto;
    padding: 8px;
    border-radius: 16px;
    background: var(--panel);
    border: 0.5px solid var(--hairline-strong);
    backdrop-filter: blur(40px);
    box-shadow: 0 24px 60px var(--shadow-strong);
    transform-origin: top left;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .caption {
    padding: 6px 8px 4px;
    color: var(--text-2);
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 8px;
    border-radius: 10px;
    cursor: pointer;
    outline: none;
    transition: background 0.15s var(--ease);
  }

  .item:hover,
  .item:focus-visible {
    background: var(--fill);
  }

  .thumb {
    flex: none;
    width: 64px;
  }

  .meta {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .name {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sub {
    color: var(--text-2);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .check {
    flex: none;
    width: 14px;
    height: 14px;
    fill: none;
    stroke: var(--accent);
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
    visibility: hidden;
  }

  .item.active .check {
    visibility: visible;
  }

  .tools {
    display: flex;
    gap: 2px;
    opacity: 0;
    transition: opacity 0.15s var(--ease);
  }

  .item:hover .tools,
  .item:focus-within .tools {
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

  .icon svg,
  .command svg {
    width: 14px;
    height: 14px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.4;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .confirm {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    color: var(--text-2);
  }

  .confirm span {
    margin-right: 2px;
  }

  .confirm button {
    height: 22px;
    padding: 0 8px;
    border-radius: 6px;
    font-size: 11px;
    font-weight: 600;
    background: var(--fill);
    color: var(--text);
  }

  .confirm .danger {
    background: var(--red);
    color: white;
  }

  .sep {
    height: 0.5px;
    margin: 6px 8px;
    background: var(--hairline);
  }

  .command {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 32px;
    padding: 0 10px;
    border-radius: 9px;
    text-align: left;
    color: var(--text);
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    transition: background 0.15s var(--ease);
  }

  .command:hover {
    background: var(--fill);
  }

  .command svg {
    flex: none;
    color: var(--accent);
  }

  .create {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 4px 6px 6px;
  }

  .field {
    width: 100%;
    height: 28px;
    padding: 0 9px;
    border-radius: 8px;
    border: 0.5px solid var(--accent);
    background: var(--fill);
    box-shadow: 0 0 0 3px rgba(10, 132, 255, 0.25);
    outline: none;
  }

  .hint {
    margin: 0;
    color: var(--text-2);
    font-size: 12px;
    line-height: 1.4;
  }

  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
  }
</style>
