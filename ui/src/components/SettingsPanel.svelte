<script lang="ts">
  import { scale } from "svelte/transition";
  import {
    deviceInfo,
    keyboardLayout,
    logStatus,
    reportRate,
    setReportRate,
    setKeyboardLayout,
    openLogDir,
    setStartupOptions,
    setVerboseLog,
    startupOptions,
    type KeyboardLayoutState,
    type LogStatus,
    type StartupOptions,
  } from "../lib/api";
  import { APPEARANCES, appearance, setAppearance, type Appearance } from "../lib/appearance.svelte";
  import { language, loadLanguage, openLangDir, setLanguage, t } from "../lib/i18n.svelte";

  interface Props {
    /** The shape or the Windows layout changed: the app redraws the keyboard. */
    onkeyboardlayout: (state: KeyboardLayoutState) => void;
  }

  let { onkeyboardlayout }: Props = $props();

  const FORMATS = ["auto", "iso", "ansi"] as const;

  let open = $state(false);
  let keyboard = $state<KeyboardLayoutState | null>(null);
  let log = $state<LogStatus | null>(null);
  let startup = $state<StartupOptions | null>(null);
  /** Report rates the keyboard accepts (empty when disconnected or not supported). */
  let rates = $state<number[]>([]);
  let rate = $state<number | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);

  /** Language the "Automatic" choice resolves to, shown next to it. */
  const autoName = $derived(language.setting === "auto" ? language.name : null);

  async function toggleOpen() {
    open = !open;
    if (open) {
      error = null;
      // Picks up language files added while the app was running.
      loadLanguage().catch((e) => (error = String(e)));
      startupOptions()
        .then((s) => (startup = s))
        .catch((e) => (error = String(e)));
      loadRate();
      keyboardLayout()
        .then((k) => (keyboard = k))
        .catch((e) => (error = String(e)));
      log = await logStatus().catch((e) => ((error = String(e)), null));
    }
  }

  async function loadRate() {
    const device = await deviceInfo().catch(() => null);
    rates = device?.capabilities.report_rates ?? [];
    rate = rates.length ? await reportRate().catch(() => null) : null;
  }

  async function changeRate(hz: number) {
    if (hz === rate) return;
    busy = true;
    try {
      await setReportRate(hz);
      rate = hz;
    } catch (e) {
      error = String(e);
      await loadRate();
    } finally {
      busy = false;
    }
  }

  async function changeKeyboard(change: { format?: string; layout?: string }) {
    busy = true;
    try {
      keyboard = await setKeyboardLayout(change);
      onkeyboardlayout(keyboard);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function changeStartup(options: { autostart?: boolean; closeToTray?: boolean }) {
    busy = true;
    try {
      startup = await setStartupOptions(options);
    } catch (e) {
      error = String(e);
      // Puts the toggle back where the system says it is.
      startup = await startupOptions().catch(() => startup);
    } finally {
      busy = false;
    }
  }

  async function chooseLanguage(code: string) {
    try {
      await setLanguage(code);
    } catch (e) {
      error = String(e);
    }
  }

  async function chooseAppearance(value: Appearance) {
    try {
      await setAppearance(value);
    } catch (e) {
      error = String(e);
    }
  }

  async function showLanguages() {
    try {
      await openLangDir();
    } catch (e) {
      error = String(e);
    }
  }

  async function toggleVerbose() {
    if (!log) return;
    busy = true;
    try {
      log = await setVerboseLog(!log.verbose);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function showLogs() {
    try {
      await openLogDir();
    } catch (e) {
      error = String(e);
    }
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && (open = false)} />

<div class="wrap">
  <button class="pill" class:active={open} onclick={toggleOpen} title={t("settings.title")} aria-label={t("settings.title")}>
    <svg viewBox="0 0 16 16">
      <path
        d="M6.9 1.5h2.2l.35 1.75c.4.15.77.36 1.1.62l1.7-.57 1.1 1.9-1.35 1.18a4.6 4.6 0 0 1 0 1.24l1.35 1.18-1.1 1.9-1.7-.57c-.33.26-.7.47-1.1.62L9.1 13.5H6.9l-.35-1.75a4.6 4.6 0 0 1-1.1-.62l-1.7.57-1.1-1.9 1.35-1.18a4.6 4.6 0 0 1 0-1.24L2.65 6.2l1.1-1.9 1.7.57c.33-.26.7-.47 1.1-.62zM8 5.6a1.9 1.9 0 1 0 0 3.8 1.9 1.9 0 0 0 0-3.8z"
      />
    </svg>
  </button>

  {#if open}
    <div class="backdrop" role="presentation" onclick={() => (open = false)}></div>
    <div class="panel" transition:scale={{ start: 0.96, duration: 180 }}>
      <div class="title">{t("settings.title")}</div>

      <div class="label">{t("settings.general")}</div>
      <div class="group">
        <div class="row">
          <div>
            <div class="name">{t("settings.language")}</div>
            <div class="sub">{t("settings.language_hint")}</div>
          </div>
          <select
            class="popup"
            value={language.setting}
            onchange={(e) => chooseLanguage((e.target as HTMLSelectElement).value)}
          >
            <option value="auto">{t("settings.language_auto", { name: autoName ?? "…" })}</option>
            {#each language.languages as l (l.code)}
              <option value={l.code}>{l.name}</option>
            {/each}
          </select>
        </div>
        <div class="sep"></div>
        <div class="row column">
          <div>
            <div class="name">{t("settings.appearance")}</div>
            <div class="sub">{t("settings.appearance_hint")}</div>
          </div>
          <div class="segmented" style:--n={3} style:--index={APPEARANCES.indexOf(appearance.setting)}>
            <div class="indicator"></div>
            {#each APPEARANCES as value (value)}
              <button class:active={appearance.setting === value} onclick={() => chooseAppearance(value)}>
                {t(`settings.appearance_${value}`)}
              </button>
            {/each}
          </div>
        </div>
        <div class="sep"></div>
        <div class="row">
          <div class="grow">
            <div class="name">{t("settings.add_language")}</div>
            <div class="sub">{t("settings.add_language_hint")}</div>
          </div>
          <button class="action" onclick={showLanguages}>{t("common.show")}</button>
        </div>
      </div>

      <div class="label">{t("settings.keyboard")}</div>
      <div class="group">
        {#if keyboard}
          <div class="row column">
            <div>
              <div class="name">{t("settings.key_format")}</div>
              <div class="sub">
                {t("settings.key_format_hint")}
                {#if keyboard.format === "auto"}{t("settings.key_format_now", { standard: keyboard.auto_standard.toUpperCase() })}{/if}
              </div>
            </div>
            <div class="segmented" style:--n={3} style:--index={Math.max(0, FORMATS.indexOf(keyboard.format as (typeof FORMATS)[number]))}>
              <div class="indicator"></div>
              {#each FORMATS as value (value)}
                <button class:active={keyboard.format === value} disabled={busy} onclick={() => changeKeyboard({ format: value })}>
                  {value === "auto" ? t("settings.key_format_auto") : value.toUpperCase()}
                </button>
              {/each}
            </div>
          </div>
          <div class="sep"></div>
          <div class="row">
            <div>
              <div class="name">{t("settings.key_layout")}</div>
              <div class="sub">{t("settings.key_layout_hint")}</div>
            </div>
            <select
              class="popup"
              value={keyboard.layout}
              disabled={busy}
              onchange={(e) => changeKeyboard({ layout: (e.target as HTMLSelectElement).value })}
            >
              <option value="auto">{t("settings.key_layout_auto", { name: keyboard.layout === "auto" ? (keyboard.layout_name ?? "…") : "Windows" })}</option>
              {#each keyboard.installed as l (l.id)}
                <option value={l.id}>{l.name}</option>
              {/each}
            </select>
          </div>
          <div class="sep"></div>
        {/if}
        <div class="row column">
          <div>
            <div class="name">{t("settings.report_rate")}</div>
            <div class="sub">{rate === null ? t("settings.report_rate_offline") : t("settings.report_rate_hint")}</div>
          </div>
          {#if rate !== null}
            <div class="segmented" style:--n={rates.length} style:--index={Math.max(0, rates.indexOf(rate))}>
              <div class="indicator"></div>
              {#each rates as hz (hz)}
                <button class:active={hz === rate} disabled={busy} onclick={() => changeRate(hz)}>{hz} Hz</button>
              {/each}
            </div>
          {/if}
        </div>
      </div>

      <div class="label">{t("settings.startup")}</div>
      <div class="group">
        <div class="row">
          <div>
            <div class="name">{t("settings.autostart")}</div>
            <div class="sub">
              {startup && !startup.autostart_supported ? t("settings.autostart_unsupported") : t("settings.autostart_hint")}
            </div>
          </div>
          <input
            type="checkbox"
            class="toggle"
            checked={startup?.autostart}
            disabled={!startup?.autostart_supported || busy}
            onchange={(e) => changeStartup({ autostart: (e.target as HTMLInputElement).checked })}
          />
        </div>
        <div class="sep"></div>
        <div class="row">
          <div>
            <div class="name">{t("settings.close_to_tray")}</div>
            <div class="sub">{t("settings.close_to_tray_hint")}</div>
          </div>
          <input
            type="checkbox"
            class="toggle"
            checked={startup?.close_to_tray}
            disabled={!startup || busy}
            onchange={(e) => changeStartup({ closeToTray: (e.target as HTMLInputElement).checked })}
          />
        </div>
      </div>

      <div class="label">{t("settings.diagnostics")}</div>
      <div class="group">
        <div class="row">
          <div>
            <div class="name">{t("settings.verbose")}</div>
            <div class="sub">{t("settings.verbose_hint")}</div>
          </div>
          <input type="checkbox" class="toggle" checked={log?.verbose} disabled={!log || busy} onchange={toggleVerbose} />
        </div>
        <div class="sep"></div>
        <div class="row">
          <div class="grow">
            <div class="name">{t("settings.activity_log")}</div>
            <div class="sub path">{log?.path ?? "…"}</div>
          </div>
          <button class="action" onclick={showLogs}>{t("common.show")}</button>
        </div>
      </div>

      {#if error}
        <p class="error">{error}</p>
      {/if}
    </div>
  {/if}
</div>

<style>
  .wrap {
    position: relative;
  }

  .pill {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: 15px;
    background: var(--surface);
    border: 0.5px solid var(--hairline);
    backdrop-filter: blur(20px);
    color: var(--text-2);
    transition:
      background 0.2s var(--ease),
      color 0.2s var(--ease);
  }

  .pill:hover,
  .pill.active {
    background: var(--surface-strong);
    color: var(--text);
  }

  .pill svg {
    width: 15px;
    height: 15px;
    fill: currentColor;
    fill-rule: evenodd;
  }

  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 19;
  }

  .panel {
    position: absolute;
    top: 40px;
    right: 0;
    z-index: 20;
    width: 380px;
    max-height: calc(100vh - 70px);
    overflow-y: auto;
    padding: 16px;
    border-radius: 16px;
    background: var(--panel);
    border: 0.5px solid var(--hairline-strong);
    backdrop-filter: blur(40px);
    box-shadow: 0 24px 60px var(--shadow-strong);
    transform-origin: top right;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .title {
    font-weight: 700;
    font-size: 14px;
  }

  .label {
    margin-top: 4px;
    color: var(--text-2);
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .group {
    border-radius: 12px;
    background: var(--fill-subtle);
    padding: 4px 12px;
  }

  .row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    padding: 10px 0;
  }

  .row.column {
    flex-direction: column;
    align-items: stretch;
  }

  .grow {
    min-width: 0;
  }

  .sep {
    height: 0.5px;
    background: var(--hairline);
  }

  .name {
    font-size: 13px;
    font-weight: 600;
  }

  .sub {
    margin-top: 2px;
    color: var(--text-2);
    font-size: 12px;
    line-height: 1.4;
  }

  .path {
    font-family: "SF Mono", "Cascadia Mono", Consolas, monospace;
    font-size: 11px;
    word-break: break-all;
    user-select: text;
  }

  .action {
    flex: none;
    padding: 5px 12px;
    border-radius: 8px;
    background: var(--fill);
    font-size: 12px;
    font-weight: 600;
    color: var(--text);
    transition: background 0.2s var(--ease);
  }

  .action:hover {
    background: var(--fill-hover);
  }

  .popup {
    flex: none;
    max-width: 170px;
    height: 28px;
    padding: 0 28px 0 11px;
    border-radius: 8px;
    border: 0.5px solid var(--hairline-strong);
    background:
      url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 10 14'%3E%3Cpath d='M2 5.5l3-3 3 3M2 8.5l3 3 3-3' fill='none' stroke='%23ebebf5' stroke-opacity='.6' stroke-width='1.4' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E")
        no-repeat right 9px center / 9px 13px,
      var(--fill);
    appearance: none;
    color: var(--text);
    font: inherit;
    font-size: 12px;
    font-weight: 600;
    text-overflow: ellipsis;
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

  .error {
    margin: 0;
    color: var(--error-text);
    font-size: 12px;
  }

  .toggle {
    appearance: none;
    position: relative;
    flex: none;
    width: 42px;
    height: 25px;
    margin: 0;
    border-radius: 13px;
    background: var(--toggle-off);
    cursor: pointer;
    transition: background 0.25s var(--ease);
  }

  .toggle::before {
    content: "";
    position: absolute;
    top: 2px;
    left: 2px;
    width: 21px;
    height: 21px;
    border-radius: 50%;
    background: white;
    box-shadow: 0 2px 5px rgba(0, 0, 0, 0.3);
    transition: transform 0.25s var(--ease);
  }

  .toggle:checked {
    background: var(--green);
  }

  .toggle:checked::before {
    transform: translateX(17px);
  }
</style>
