<script lang="ts">
  import { untrack } from "svelte";
  import { fade, scale } from "svelte/transition";
  import { plural, t } from "../lib/i18n.svelte";
  import type { KeymapSupport, KeyNames, Macro, MacroStep } from "../lib/keys";

  interface Props {
    /** The macro to edit, or `null` for a new one. */
    macro: Macro | null;
    names: KeyNames;
    support: KeymapSupport;
    /** Labels of the keys playing this macro. */
    usedBy: string[];
    onsave: (macro: Macro) => void;
    ondelete?: () => void;
    oncancel: () => void;
  }

  let { macro, names, support, usedBy, onsave, ondelete, oncancel }: Props = $props();

  const initial = untrack(() => macro);
  let name = $state(initial?.name ?? t("macros.new_default"));
  let repeat = $state(initial?.repeat ?? 1);
  let steps = $state<MacroStep[]>(initial ? initial.steps.map((s) => ({ ...s })) : []);
  let recording = $state(false);
  let recordPauses = $state(true);
  let text = $state("");
  let error = $state<string | null>(null);
  let confirmDelete = $state(false);
  let lastTime = 0;

  const step = $derived(support.delay_step_ms);
  /** Pause used when pauses are not recorded and for typed text. */
  const defaultDelay = $derived(step * 5);
  const full = $derived(steps.length >= support.max_macro_steps);
  const canSave = $derived(name.trim().length > 0 && steps.length > 0 && steps.length <= support.max_macro_steps);

  const clampDelay = (ms: number) =>
    Math.min(support.max_delay_ms, Math.max(0, Math.round(ms / step) * step));

  function toggleRecording() {
    recording = !recording;
    error = null;
    lastTime = 0;
  }

  // While recording, every key goes into the macro instead of the app.
  function record(e: KeyboardEvent, pressed: boolean) {
    if (!recording) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    if (e.repeat) return;
    const code = names.codeOfEvent(e.code);
    if (code === undefined) return;
    if (full) {
      recording = false;
      error = t("macros.full", { max: support.max_macro_steps });
      return;
    }
    const now = performance.now();
    const last = steps.at(-1);
    if (last && lastTime) last.delay_ms = recordPauses ? clampDelay(now - lastTime) : defaultDelay;
    steps.push({ code, pressed, delay_ms: 0 });
    lastTime = now;
  }

  function addText() {
    const added = names.textSteps(text, defaultDelay);
    if (!Array.isArray(added)) {
      error = t("macros.text_unsupported", { char: added.unsupported, layout: names.layoutName });
      return;
    }
    if (steps.length + added.length > support.max_macro_steps) {
      error = t("macros.full", { max: support.max_macro_steps });
      return;
    }
    const last = steps.at(-1);
    if (last && last.delay_ms === 0) last.delay_ms = defaultDelay;
    steps.push(...added);
    steps[steps.length - 1].delay_ms = 0;
    text = "";
    error = null;
  }

  function save() {
    if (!canSave) return;
    onsave({ name: name.trim(), repeat: Math.min(99, Math.max(1, Math.round(repeat))), steps: $state.snapshot(steps) });
  }
</script>

<svelte:window
  onkeydowncapture={(e) => record(e, true)}
  onkeyupcapture={(e) => record(e, false)}
  onkeydown={(e) => !recording && e.key === "Escape" && oncancel()}
/>

<div class="scrim" transition:fade={{ duration: 150 }}>
  <div class="dialog" role="dialog" aria-modal="true" transition:scale={{ start: 0.96, duration: 200 }}>
    <div class="head">
      <input class="title" bind:value={name} maxlength="32" placeholder={t("macros.name_placeholder")} />
      <label class="repeat">
        <span>{t("macros.repeat")}</span>
        <input type="number" min="1" max="99" bind:value={repeat} />
      </label>
    </div>

    <div class="recorder">
      <button class="record" class:on={recording} onclick={toggleRecording}>
        <span class="dot"></span>
        {recording ? t("macros.stop") : steps.length ? t("macros.record_more") : t("macros.record")}
      </button>
      <label class="toggle-row small">
        <span>{t("macros.record_pauses")}</span>
        <input type="checkbox" class="toggle" bind:checked={recordPauses} disabled={recording} />
      </label>
    </div>
    <p class="hint left">{recording ? t("macros.recording_hint") : t("macros.record_hint")}</p>

    <div class="steps" class:empty={steps.length === 0}>
      {#each steps as s, i (i)}
        <div class="step">
          <span class="dir" class:down={s.pressed}>{s.pressed ? "↓" : "↑"}</span>
          <span class="label">{names.label(s.code)}</span>
          <span class="state">{s.pressed ? t("macros.pressed") : t("macros.released")}</span>
          {#if i < steps.length - 1}
            <label class="delay" title={t("macros.pause_hint")}>
              <input
                type="number"
                min="0"
                max={support.max_delay_ms}
                step={step}
                value={s.delay_ms}
                onchange={(e) => (s.delay_ms = clampDelay(Number((e.target as HTMLInputElement).value)))}
              />
              ms
            </label>
          {:else}
            <span class="delay end"></span>
          {/if}
          <button class="remove" aria-label={t("macros.remove_step")} onclick={() => steps.splice(i, 1)}>×</button>
        </div>
      {:else}
        <p class="placeholder">{t("macros.no_steps")}</p>
      {/each}
    </div>

    <div class="row">
      <input
        class="text"
        bind:value={text}
        placeholder={t("macros.text_placeholder")}
        disabled={recording}
        onkeydown={(e) => e.key === "Enter" && text && addText()}
      />
      <button class="btn" disabled={!text || recording} onclick={addText}>{t("macros.add_text")}</button>
      {#if steps.length}
        <button class="btn" disabled={recording} onclick={() => (steps = [])}>{t("macros.clear")}</button>
      {/if}
    </div>

    <p class="hint left">
      {plural("macros.steps", steps.length)} · {t("macros.max_steps", { max: support.max_macro_steps })}
      {#if usedBy.length}· {t("macros.on_keys", { keys: usedBy.join(", ") })}{/if}
    </p>
    {#if !support.verified}
      <p class="hint left warn">{t("macros.unverified")}</p>
    {/if}
    {#if error}
      <p class="error">{error}</p>
    {/if}

    <div class="actions">
      {#if ondelete}
        {#if confirmDelete}
          <button class="btn danger" onclick={ondelete}>{t("macros.delete_confirm")}</button>
        {:else}
          <button class="btn" onclick={() => (confirmDelete = true)}>{t("macros.delete")}</button>
        {/if}
      {/if}
      <span class="spacer"></span>
      <button class="btn" onclick={oncancel}>{t("common.cancel")}</button>
      <button class="btn primary" disabled={!canSave || recording} onclick={save}>{t("common.save")}</button>
    </div>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
    background: var(--scrim);
    backdrop-filter: blur(6px);
    z-index: 10;
  }

  .dialog {
    display: flex;
    flex-direction: column;
    gap: 12px;
    width: 520px;
    max-height: calc(100vh - 60px);
    padding: 22px;
    border-radius: 18px;
    background: var(--panel);
    border: 0.5px solid var(--hairline-strong);
    backdrop-filter: blur(40px);
    box-shadow: 0 30px 80px var(--shadow-strong);
  }

  .head {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .title {
    flex: 1;
    min-width: 0;
    height: 34px;
    padding: 0 10px;
    border-radius: 9px;
    border: 0.5px solid transparent;
    background: transparent;
    font-size: 17px;
    font-weight: 700;
    outline: none;
    transition:
      background 0.2s var(--ease),
      border-color 0.2s var(--ease);
  }

  .title:hover,
  .title:focus {
    background: var(--fill-subtle);
    border-color: var(--hairline-strong);
  }

  .repeat {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-2);
    font-size: 12px;
  }

  input[type="number"] {
    width: 58px;
    height: 28px;
    padding: 0 6px;
    border-radius: 7px;
    border: 0.5px solid var(--hairline-strong);
    background: var(--fill);
    text-align: right;
    outline: none;
  }

  .recorder {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .record {
    display: flex;
    align-items: center;
    gap: 9px;
    height: 38px;
    padding: 0 18px;
    border-radius: 19px;
    background: var(--fill);
    font-weight: 600;
    transition: background 0.2s var(--ease);
  }

  .record:hover {
    background: var(--fill-hover);
  }

  .record .dot {
    width: 11px;
    height: 11px;
    border-radius: 50%;
    background: var(--red);
  }

  .record.on {
    background: rgba(255, 69, 58, 0.22);
    box-shadow: inset 0 0 0 1px rgba(255, 69, 58, 0.6);
  }

  .record.on .dot {
    border-radius: 3px;
    animation: blink 1s ease-in-out infinite;
  }

  @keyframes blink {
    50% {
      opacity: 0.35;
    }
  }

  .toggle-row.small {
    gap: 10px;
    font-size: 12px;
    color: var(--text-2);
  }

  .steps {
    display: flex;
    flex-direction: column;
    min-height: 120px;
    max-height: 300px;
    overflow-y: auto;
    border-radius: 12px;
    background: var(--well);
    border: 0.5px solid var(--hairline);
  }

  .steps.empty {
    display: grid;
    place-items: center;
  }

  .placeholder {
    margin: 0;
    color: var(--text-3);
  }

  .step {
    display: grid;
    grid-template-columns: 22px 1fr auto 92px 24px;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
  }

  .step + .step {
    border-top: 0.5px solid var(--hairline);
  }

  .dir {
    font-weight: 700;
    color: var(--text-3);
  }

  .dir.down {
    color: var(--purple-text);
  }

  .label {
    font-weight: 600;
  }

  .state {
    color: var(--text-2);
    font-size: 11.5px;
  }

  .delay {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 5px;
    color: var(--text-3);
    font-size: 11px;
  }

  .remove {
    width: 22px;
    height: 22px;
    border-radius: 6px;
    color: var(--text-3);
    font-size: 15px;
  }

  .remove:hover {
    background: var(--fill);
    color: var(--text);
  }

  .row {
    display: flex;
    gap: 8px;
  }

  .text {
    flex: 1;
    min-width: 0;
    height: 30px;
    padding: 0 10px;
    border-radius: 8px;
    border: 0.5px solid var(--hairline-strong);
    background: var(--fill-subtle);
    outline: none;
  }

  .text:focus {
    border-color: var(--accent);
  }

  .warn {
    color: var(--orange-text);
  }

  .error {
    margin: 0;
    color: var(--error-text);
    font-size: 12px;
  }

  .actions {
    display: flex;
    gap: 8px;
    margin-top: 4px;
  }

  .spacer {
    flex: 1;
  }

  .btn.danger {
    background: rgba(255, 69, 58, 0.85);
    color: white;
  }
</style>
