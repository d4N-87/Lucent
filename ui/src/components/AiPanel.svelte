<script lang="ts">
  import { onMount } from "svelte";
  import { scale } from "svelte/transition";
  import { mcpSetEnabled, mcpStatus, type McpStatus } from "../lib/api";
  import { t } from "../lib/i18n.svelte";

  let open = $state(false);
  let status = $state<McpStatus | null>(null);
  let busy = $state(false);
  let copied = $state<string | null>(null);

  onMount(async () => {
    status = await mcpStatus();
  });

  async function toggle() {
    if (!status) return;
    busy = true;
    try {
      status = await mcpSetEnabled(!status.enabled);
    } finally {
      busy = false;
    }
  }

  const claudeCommand = $derived(`claude mcp add --transport http lucent ${status?.url ?? ""}`);
  const jsonConfig = $derived(
    JSON.stringify({ mcpServers: { "lucent": { type: "http", url: status?.url ?? "" } } }, null, 2),
  );

  async function copy(text: string, what: string) {
    try {
      await navigator.clipboard.writeText(text);
      copied = what;
      setTimeout(() => (copied = null), 1200);
    } catch {
      // Clipboard unavailable: the text is selectable on screen.
    }
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && (open = false)} />

<div class="wrap">
  <button class="pill" class:on={status?.running} onclick={() => (open = !open)} title={t("ai.button")}>
    <svg viewBox="0 0 16 16"><path d="M8 1.5l1.6 3.9 3.9 1.6-3.9 1.6L8 12.5 6.4 8.6 2.5 7l3.9-1.6z" /></svg>
    AI
  </button>

  {#if open}
    <div class="backdrop" role="presentation" onclick={() => (open = false)}></div>
    <div class="panel" transition:scale={{ start: 0.96, duration: 180 }}>
      <div class="row">
        <div>
          <div class="title">{t("ai.title")}</div>
          <div class="sub">{t("ai.subtitle")}</div>
        </div>
        <input type="checkbox" class="toggle" checked={status?.enabled} disabled={busy} onchange={toggle} />
      </div>

      {#if status?.error}
        <p class="error">{status.error}</p>
      {:else if status?.running}
        <p class="ok"><span class="dot"></span>{t("ai.running")} <code>{status.url}</code></p>
      {:else}
        <p class="muted">{t("ai.off")}</p>
      {/if}

      {#if status?.running}
        <div class="block">
          <div class="label">
            Claude Code
            <button class="copy" onclick={() => copy(claudeCommand, "cmd")}>{copied === "cmd" ? t("common.copied") : t("common.copy")}</button>
          </div>
          <pre>{claudeCommand}</pre>
        </div>
        <div class="block">
          <div class="label">
            {t("ai.other_clients")}
            <button class="copy" onclick={() => copy(jsonConfig, "json")}>{copied === "json" ? t("common.copied") : t("common.copy")}</button>
          </div>
          <pre>{jsonConfig}</pre>
        </div>
        <p class="muted">
          {t("ai.note")}
        </p>
      {/if}
    </div>
  {/if}
</div>

<style>
  .wrap {
    position: relative;
  }

  .pill {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    padding: 0 12px;
    border-radius: 15px;
    background: var(--surface);
    border: 0.5px solid var(--hairline);
    backdrop-filter: blur(20px);
    color: var(--text-2);
    font-weight: 600;
    transition:
      background 0.2s var(--ease),
      color 0.2s var(--ease);
  }

  .pill:hover {
    background: var(--surface-strong);
  }

  .pill svg {
    width: 14px;
    height: 14px;
    fill: currentColor;
  }

  .pill.on {
    color: white;
    background: linear-gradient(135deg, rgba(191, 90, 242, 0.55), rgba(10, 132, 255, 0.55));
    border-color: rgba(191, 90, 242, 0.6);
    box-shadow: 0 0 16px rgba(191, 90, 242, 0.35);
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
    width: 420px;
    padding: 16px;
    border-radius: 16px;
    background: var(--panel);
    border: 0.5px solid var(--hairline-strong);
    backdrop-filter: blur(40px);
    box-shadow: 0 24px 60px var(--shadow-strong);
    transform-origin: top right;
    display: flex;
    flex-direction: column;
    gap: 12px;
    user-select: text;
  }

  .row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
  }

  .title {
    font-weight: 700;
    font-size: 14px;
  }

  .sub,
  .muted {
    color: var(--text-2);
    font-size: 12px;
    line-height: 1.4;
  }

  p {
    margin: 0;
  }

  .ok {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--green);
    box-shadow: 0 0 8px var(--green);
  }

  .error {
    color: var(--error-text);
    font-size: 12px;
  }

  code,
  pre {
    font-family: "SF Mono", "Cascadia Mono", Consolas, monospace;
    font-size: 11.5px;
  }

  .block {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .label {
    display: flex;
    justify-content: space-between;
    align-items: center;
    color: var(--text-2);
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .copy {
    padding: 2px 8px;
    border-radius: 6px;
    background: var(--fill);
    font-size: 11px;
    text-transform: none;
    letter-spacing: 0;
    color: var(--text);
  }

  pre {
    margin: 0;
    padding: 10px;
    border-radius: 10px;
    background: var(--well);
    white-space: pre-wrap;
    word-break: break-all;
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
