<script lang="ts">
  import { slide } from "svelte/transition";
  import { t } from "../lib/i18n.svelte";
  import { describe, kindOf, MEDIA, type KeyAction, type KeyKind, type KeyNames, type Macro } from "../lib/keys";

  interface Props {
    /** Printed label of the key. */
    label: string;
    action: KeyAction;
    factory: KeyAction;
    names: KeyNames;
    macros: Macro[];
    /** The keyboard can store macros. */
    macrosSupported: boolean;
    /** `null` puts the key back to its factory function. */
    onchange: (action: KeyAction | null) => void;
    onnewmacro: () => void;
    oneditmacro: (index: number) => void;
  }

  let { label, action, factory, names, macros, macrosSupported, onchange, onnewmacro, oneditmacro }: Props =
    $props();

  const kind = $derived(kindOf(action, factory));
  /** A choice opened but not set yet (e.g. "another key" before pressing one). */
  let opened = $state<KeyKind | null>(null);
  const shown = $derived(opened ?? kind);
  let listening = $state(false);

  const CHOICES: KeyKind[] = ["default", "key", "media", "macro", "disabled"];

  function choose(choice: KeyKind) {
    listening = false;
    if (choice === "default") {
      opened = null;
      onchange(null);
    } else if (choice === "disabled") {
      opened = null;
      onchange({ kind: "disabled" });
    } else {
      opened = choice;
      if (choice === "key") listening = true;
    }
  }

  function set(next: KeyAction) {
    listening = false;
    opened = null;
    onchange(next);
  }

  // While listening, the next physical key press becomes the key's new function.
  function onKey(e: KeyboardEvent) {
    if (!listening) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    const code = names.codeOfEvent(e.code);
    if (code !== undefined) set({ kind: "key", code });
  }
</script>

<svelte:window onkeydowncapture={onKey} />

{#if factory.kind === "fn"}
  <div class="title"><span class="key">Fn</span></div>
  <p class="hint left">{t("keys.fn_locked")}</p>
{:else}
  <div class="title">
    <span class="key">{label}</span>
    {#if kind !== "default"}
      <span class="arrow">→</span>
      <span class="now {kind}">{describe(action, names, macros)}</span>
    {/if}
  </div>

  <div class="list">
    {#each CHOICES as choice (choice)}
      {#if choice !== "macro" || macrosSupported}
        <button class="row" class:active={shown === choice} onclick={() => choose(choice)}>
          <span class="radio"></span>
          <span class="text">
            <span class="name">{t(`keys.choice_${choice}`)}</span>
            {#if choice === "default"}
              <span class="sub">{t("keys.default_hint", { key: describe(factory, names, macros) })}</span>
            {/if}
          </span>
        </button>

        {#if shown === choice && choice === "key"}
          <div class="detail" transition:slide={{ duration: 180 }}>
            <button class="capture" class:listening onclick={() => (listening = !listening)}>
              {#if listening}
                <span class="pulse"></span>{t("keys.press_key")}
              {:else if action.kind === "key" && kind === "key"}
                {names.label(action.code)} · {t("keys.change")}
              {:else}
                {t("keys.press_to_choose")}
              {/if}
            </button>
            <select
              class="popup"
              value={action.kind === "key" ? action.code : ""}
              onchange={(e) => set({ kind: "key", code: Number((e.target as HTMLSelectElement).value) })}
            >
              <option value="" disabled>{t("keys.pick_from_list")}</option>
              {#each names.choices() as c (c.code)}
                <option value={c.code}>{c.label}</option>
              {/each}
            </select>
          </div>
        {:else if shown === choice && choice === "media"}
          <div class="detail grid" transition:slide={{ duration: 180 }}>
            {#each MEDIA as m (m.id)}
              <button
                class="chip"
                class:active={action.kind === "media" && action.code === m.code}
                onclick={() => set({ kind: "media", code: m.code })}
              >
                {t(`media.${m.id}.name`)}
              </button>
            {/each}
          </div>
        {:else if shown === choice && choice === "macro"}
          <div class="detail" transition:slide={{ duration: 180 }}>
            {#each macros as m, i (i)}
              <div class="macro" class:active={action.kind === "macro" && action.index === i}>
                <button class="pick" onclick={() => set({ kind: "macro", index: i })}>{m.name}</button>
                <button class="edit" onclick={() => oneditmacro(i)}>{t("keys.edit")}</button>
              </div>
            {/each}
            <button class="btn" onclick={onnewmacro}>{t("macros.new_short")}</button>
          </div>
        {/if}
      {/if}
    {/each}
  </div>
{/if}

<style>
  .title {
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
  }

  .key {
    font-size: 20px;
    font-weight: 700;
  }

  .arrow {
    color: var(--text-3);
  }

  .now {
    font-size: 15px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .now.key {
    color: var(--blue-text);
  }

  .now.media {
    color: var(--orange-text);
  }

  .now.macro {
    color: var(--purple-text);
  }

  .now.disabled {
    color: var(--text-2);
  }

  .list {
    display: flex;
    flex-direction: column;
    border-radius: 12px;
    background: var(--fill-subtle);
    overflow: hidden;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    text-align: left;
    transition: background 0.2s var(--ease);
  }

  .row + .row,
  .detail + .row {
    border-top: 0.5px solid var(--hairline);
  }

  .row:hover {
    background: var(--fill-subtle);
  }

  .radio {
    flex: none;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    box-shadow: inset 0 0 0 1.5px var(--fill-strong);
    transition: box-shadow 0.2s var(--ease);
  }

  .row.active .radio {
    box-shadow:
      inset 0 0 0 5px var(--accent),
      inset 0 0 0 8px white;
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .name {
    font-weight: 500;
  }

  .sub {
    color: var(--text-2);
    font-size: 11.5px;
  }

  .detail {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 2px 12px 12px 38px;
  }

  .detail.grid {
    flex-direction: row;
    flex-wrap: wrap;
    gap: 6px;
  }

  .capture {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    height: 34px;
    border-radius: 9px;
    border: 1px dashed var(--fill-strong);
    color: var(--text-2);
    font-weight: 500;
    transition:
      border-color 0.2s var(--ease),
      color 0.2s var(--ease);
  }

  .capture.listening {
    border: 1px solid var(--accent);
    color: var(--text);
    box-shadow: 0 0 0 3px rgba(10, 132, 255, 0.25);
  }

  .pulse {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--accent);
    animation: pulse 1.2s ease-in-out infinite;
  }

  @keyframes pulse {
    50% {
      opacity: 0.3;
    }
  }

  .popup {
    height: 30px;
    padding: 0 10px;
    border-radius: 8px;
    border: 0.5px solid var(--hairline-strong);
    background: var(--fill);
    color: var(--text);
    font: inherit;
    font-size: 12px;
    outline: none;
  }

  .popup option {
    background: var(--option-bg);
  }

  .macro {
    display: flex;
    align-items: center;
    border-radius: 9px;
    background: var(--fill-subtle);
    transition: background 0.2s var(--ease);
  }

  .macro.active {
    background: rgba(191, 90, 242, 0.22);
    box-shadow: inset 0 0 0 1px rgba(191, 90, 242, 0.6);
  }

  .pick {
    flex: 1;
    padding: 8px 10px;
    text-align: left;
    font-weight: 500;
  }

  .edit {
    padding: 8px 10px;
    color: var(--text-2);
    font-size: 12px;
  }

  .edit:hover {
    color: var(--text);
  }
</style>
