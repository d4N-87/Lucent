<script lang="ts">
  import { onMount, tick, untrack } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { SvelteMap, SvelteSet } from "svelte/reactivity";
  import { fade, fly } from "svelte/transition";
  import AiPanel from "./components/AiPanel.svelte";
  import SettingsPanel from "./components/SettingsPanel.svelte";
  import ColorPicker from "./components/ColorPicker.svelte";
  import EffectsPanel from "./components/EffectsPanel.svelte";
  import ImageDialog from "./components/ImageDialog.svelte";
  import KeyboardLook from "./components/KeyboardLook.svelte";
  import KeyFunctionPanel from "./components/KeyFunctionPanel.svelte";
  import KeyInfo from "./components/KeyInfo.svelte";
  import MacroEditor from "./components/MacroEditor.svelte";
  import MacroList from "./components/MacroList.svelte";
  import Keyboard from "./components/Keyboard.svelte";
  import ProfileMenu from "./components/ProfileMenu.svelte";
  import SupportBanner from "./components/SupportBanner.svelte";
  import ThemeList from "./components/ThemeList.svelte";
  import {
    checkKeyboard,
    connect,
    deviceInfo,
    getLayout,
    keyboardLayout,
    listLayouts,
    setPreviewFormat,
    setKeyboardLayout,
    type KeyboardLayoutState,
    deleteUserProfile,
    loadThemes,
    loadUserProfiles,
    saveExport,
    saveThemes,
    saveUserProfile,
    setActiveProfile,
    sendColors,
    reportRate,
    setEffect,
    setReportRate,
    setTrayMenu,
    startupOptions,
    type DeviceDescription,
    type EffectInfo,
    type EffectState,
    type Layout,
    type LayoutSummary,
    type Support,
    type UnknownKeyboard,
    type Lighting,
    type Rgb,
    type ThemeStore,
    type UserProfile,
  } from "./lib/api";
  import { css, fromHex, toHex } from "./lib/color";
  import { DEFAULT_EFFECT, effectDescriptionEn, effectName, effectNameEn } from "./lib/effects";
  import { GROUPS, groupSlots } from "./lib/groups";
  import { APPEARANCES, appearance, setAppearance, type Appearance } from "./lib/appearance.svelte";
  import { CASE_PRESETS, KEY_PRESETS, look, setLook } from "./lib/look.svelte";
  import { language, loadLanguage, plural, setLanguage, t, tEn } from "./lib/i18n.svelte";
  import { serveMcp, type ToolHandler } from "./lib/mcp";
  import { glowColors } from "./lib/glow";
  import logoUrl from "./assets/lucent-logo.png";
  import { buildPalette, HARMONIES, samplePalette } from "./lib/palette";
  import { EffectSimulator, keyIdFromCode } from "./lib/simulate";
  import { clampCrop, DEFAULT_CROP, keyTemplate, sampleKeys } from "./lib/image";
  import { PRESETS, presetKey } from "./lib/presets";
  import {
    canonical,
    cloneConfig,
    configDiff,
    describe,
    kindOf,
    KeyNames,
    MEDIA,
    mediaId,
    readKeys,
    sameAction,
    sameConfig,
    writeKeys,
    type KeyAction,
    type KeyConfig,
    type KeyKind,
    type Macro,
  } from "./lib/keys";

  /** Wait after the last edit before uploading, so dragging the picker doesn't flood the keyboard. */
  const AUTO_SEND_DELAY = 300;
  /** Edits closer together than this form a single undo step (e.g. dragging the picker). */
  const UNDO_GROUP_GAP = 400;
  const UNDO_LIMIT = 200;
  const DEFAULT_PRESET = PRESETS[0].id;
  /** Hardware brightness is only 0..4: keep it at max and dim the colors in software instead. */
  const HW_BRIGHTNESS_MAX = 4;

  type Target = "keys" | "background";
  type Mode = "per_key" | "effect";
  /** The effect preview redraws at most this often. */
  const PREVIEW_FPS = 30;

  let layout = $state<Layout | null>(null);
  /** Color of every key without its own color. */
  let background = $state<Rgb>([255, 255, 255]);
  /** Per-key colors painted over the background, by slot. */
  const overrides = new SvelteMap<number, Rgb>();
  const selected = new SvelteSet<number>();
  let brush = $state<Rgb>([255, 0, 0]);
  /** Color the harmony palette is built from; picking a palette tone must not rebuild it. */
  let paletteBase = $state<Rgb>([255, 0, 0]);
  let target = $state<Target>("keys");
  let harmonyId = $state(HARMONIES[0].id);
  let intensity = $state(100);
  /** Per-key colors, or a hardware effect running on the keyboard. */
  let mode = $state<Mode>("per_key");
  let effectSetup = $state<EffectState>({ ...DEFAULT_EFFECT });
  /** Model and capabilities of the connected keyboard. */
  let device = $state<DeviceDescription | null>(null);
  let autoSend = $state(true);
  let connected = $state(false);
  /** How far the app trusts the connected keyboard (see `Support` in lib/api.ts). */
  let support = $state<Support>("none");
  let unknownKeyboard = $state<UnknownKeyboard | null>(null);
  /** Formats that can draw a keyboard without a model. */
  let formats = $state<LayoutSummary[]>([]);
  /** The user allowed key writes on a model that is not verified, for this session. */
  let keysUnlocked = $state(false);
  let status = $state(t("app.connecting"));
  let lastSendMs = $state<number | null>(null);
  let sending = $state(false);

  let themeStore = $state<ThemeStore>({ themes: [] });
  /** Lighting of the active theme; edits are compared against it. `null` without a theme. */
  let baseline = $state.raw<Lighting | null>(null);

  /** Profiles: lighting, key functions and macros. The active one saves itself on every edit. */
  let profiles = $state<UserProfile[]>([]);
  let activeProfileId = $state<string | null>(null);
  let profileTimer: ReturnType<typeof setTimeout> | undefined;
  /** Wait after the last edit before saving the profile file. */
  const PROFILE_SAVE_DELAY = 400;

  // Undo history: snapshots taken before each group of edits.
  let undoStack = $state.raw<Lighting[]>([]);
  let redoStack = $state.raw<Lighting[]>([]);
  /** State at the end of the last finished edit group. */
  let committed: Lighting | null = null;
  let groupTimer: ReturnType<typeof setTimeout> | undefined;

  /** Name field shown in the toolbar while saving the colors as a new theme. */
  let naming = $state(false);
  let newName = $state("");
  let nameInput = $state<HTMLInputElement | undefined>();

  // ---- Key functions state ----
  /** What the app edits: lighting, or what each key does. */
  let section = $state<"lighting" | "keys">("lighting");
  /** Key functions and macros of the active profile; written to the keyboard on change. */
  let keyConfig = $state<KeyConfig | null>(null);
  /** Factory function of every key, by slot. */
  let keyDefaults = $state.raw<Record<string, KeyAction>>({});
  /** What the keyboard had when last read or written. */
  let keysOnBoard = $state.raw<KeyConfig | null>(null);
  let keyUndo = $state.raw<KeyConfig[]>([]);
  let keyRedo = $state.raw<KeyConfig[]>([]);
  let keysWriting = $state(false);
  /** An edit is waiting for its write: the keyboard is behind on purpose, not changed elsewhere. */
  let keysPending = $state(false);
  let keysError = $state<string | null>(null);
  let keysTimer: ReturnType<typeof setTimeout> | undefined;
  let keysRewrite = false;
  /** Macro open in the editor: an index, or `null` for a new one. */
  let editingMacro = $state<{ index: number | null; forSlot?: number } | null>(null);
  let confirmResetKeys = $state(false);
  /** Wait after the last key edit before writing, so quick changes make one write. */
  const KEYS_WRITE_DELAY = 500;
  /** Key colors in the Keys section, by kind of function. */
  const KIND_COLORS: Record<KeyKind, Rgb> = {
    default: [120, 120, 132],
    key: [100, 210, 255],
    media: [255, 179, 64],
    macro: [209, 140, 255],
    disabled: [0, 0, 0],
  };

  let sendTimer: ReturnType<typeof setTimeout> | undefined;
  // Set when an edit arrives during an upload, so the latest colors are sent right after.
  let resend = false;

  const effects: EffectInfo[] = $derived(device?.capabilities.effects ?? []);
  const effectInfo = $derived(effects.find((e) => e.id === effectSetup.id));
  /** Picture being framed in the image dialog. */
  let pendingImage = $state.raw<{ bitmap: ImageBitmap; name: string } | null>(null);
  let imageInput = $state<HTMLInputElement | undefined>();

  /** Imitation of the running effect, redrawn while in effect mode. */
  let effectFrame = $state.raw<Rgb[] | null>(null);
  const simulator = $derived(layout ? new EffectSimulator(layout) : null);
  const colors: Rgb[] = $derived(
    layout ? Array.from({ length: layout.led_count }, (_, slot) => overrides.get(slot) ?? background) : [],
  );
  /** What the keyboard shows. */
  const previewColors: Rgb[] = $derived(mode === "effect" && effectFrame ? effectFrame : colors);
  /** Glow behind the logo: the two main colors of the keyboard as shown, off with the lights. */
  const glow = $derived(
    layout && intensity > 0 ? glowColors(layout.keys.map((k) => previewColors[k.slot] ?? [0, 0, 0])) : null,
  );
  /** A format is drawn instead of a real keyboard: nothing is saved or sent. */
  const previewOnly = $derived(layout?.kind === "format");
  /** Key functions may be written: always, except on a model that is not verified until the user agrees. */
  const keysAllowed = $derived(support !== "unverified" || keysUnlocked);
  const allSlots = $derived(layout ? layout.keys.map((k) => k.slot) : []);
  const harmony = $derived(HARMONIES.find((h) => h.id === harmonyId) ?? HARMONIES[0]);
  const palette = $derived(buildPalette(paletteBase, harmony));
  const pickerValue = $derived(target === "background" ? background : brush);
  const builtPresets = $derived(
    layout ? PRESETS.map((p) => ({ id: p.id, name: t(presetKey(p.id)), lighting: p.build(layout!) })) : [],
  );
  const profile = $derived(profiles.find((p) => p.id === activeProfileId) ?? null);
  /** Theme the active profile's colors come from: a user theme or a preset id. */
  const themeId = $derived(profile?.theme ?? null);
  const activeTheme = $derived(themeStore.themes.find((p) => p.id === themeId) ?? null);
  const activePreset = $derived(builtPresets.find((p) => p.id === themeId) ?? null);
  const activeName = $derived(activeTheme?.name ?? activePreset?.name ?? t("themes.custom"));
  /** Second line of each profile in the menu: its theme and how much of the keyboard it changes. */
  const profileDetails = $derived(Object.fromEntries(profiles.map((p) => [p.id, describeProfile(p)])));
  const dirty = $derived(baseline !== null && !sameLighting(baseline, currentLighting()));
  const inspectedKey = $derived(
    layout && selected.size === 1 ? layout.keys.find((k) => selected.has(k.slot)) : undefined,
  );

  const keymapSupport = $derived(device?.capabilities.keymap ?? null);
  const keyNames = $derived(layout && Object.keys(keyDefaults).length ? new KeyNames(layout, keyDefaults) : null);
  const keysMismatch = $derived(
    !keysPending && !keysWriting && !!keyConfig && !!keysOnBoard && !sameConfig(keyConfig, keysOnBoard),
  );
  const keyKinds = $derived(
    Object.fromEntries(
      (layout?.keys ?? []).map((k) => [k.slot, kindOf(keyConfig?.keys[k.slot], keyDefaults[k.slot])]),
    ) as Record<number, KeyKind>,
  );
  const keyColors: Rgb[] = $derived(
    layout
      ? Array.from({ length: layout.led_count }, (_, slot) => KIND_COLORS[keyKinds[slot] ?? "default"])
      : [],
  );
  const keyLegends = $derived.by(() => {
    const out: Record<number, string> = {};
    if (!keyConfig || !keyNames) return out;
    for (const [slot, kind] of Object.entries(keyKinds)) {
      if (kind !== "default") out[Number(slot)] = describe(keyConfig.keys[slot], keyNames, keyConfig.macros);
    }
    return out;
  });
  const keyMarks = $derived(
    Object.fromEntries(Object.entries(keyKinds).filter(([, k]) => k !== "default")) as Record<number, string>,
  );
  const changedKeys = $derived(Object.values(keyKinds).filter((k) => k !== "default").length);
  /** Labels of the keys playing each macro. */
  const macroUsers = $derived(
    (keyConfig?.macros ?? []).map((_, i) =>
      (layout?.keys ?? [])
        .filter((k) => {
          const a = keyConfig?.keys[k.slot];
          return a?.kind === "macro" && a.index === i;
        })
        .map((k) => k.label || t("common.space")),
    ),
  );

  onMount(() => {
    const unlisten = serveMcp(mcpTools, (tool) => (status = `AI · ${tool}`));
    const win = getCurrentWindow();
    // Closing hides the window when the app stays in the tray. Nothing can be lost: the
    // profile saves itself, so only a save still waiting is finished first.
    const unlistenClose = win.onCloseRequested(async (e) => {
      e.preventDefault();
      if ((await startupOptions().catch(() => null))?.close_to_tray) {
        await win.hide();
        return;
      }
      await flushProfile();
      await win.destroy();
    });
    const unlistenQuit = listen("tray-quit", async () => {
      await flushProfile();
      await win.destroy();
    });
    // The menu ticks the clicked item by itself: redraw it from the real state.
    const unlistenTrayProfile = listen<string>("tray-profile", async ({ payload }) => {
      await selectProfile(payload);
      updateTrayMenu();
    });
    const unlistenTrayTheme = listen<string>("tray-theme", ({ payload }) => {
      applyTheme(payload);
      updateTrayMenu();
    });
    // The user may have switched the Windows keyboard layout while away.
    const unlistenFocus = win.onFocusChanged(({ payload }) => payload && refreshLegends());
    init();
    const watch = setInterval(watchKeyboard, WATCH_INTERVAL);
    return () => {
      clearInterval(watch);
      for (const stop of [unlisten, unlistenClose, unlistenQuit, unlistenTrayProfile, unlistenTrayTheme, unlistenFocus]) {
        stop.then((f) => f());
      }
    };
  });

  // Animates the effect preview. With "reduce motion" on, it shows a single still frame.
  $effect(() => {
    const sim = simulator;
    if (mode !== "effect" || !sim) {
      effectFrame = null;
      return;
    }
    const fx = $state.snapshot(effectSetup) as EffectState;
    const draw = () => (effectFrame = sim.frame(fx, performance.now() / 1000));
    if (matchMedia("(prefers-reduced-motion: reduce)").matches) {
      draw();
      return;
    }
    let raf = 0;
    let last = 0;
    const loop = (now: number) => {
      if (now - last >= 1000 / PREVIEW_FPS) {
        last = now;
        draw();
      }
      raf = requestAnimationFrame(loop);
    };
    raf = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(raf);
  });

  // The tray menu follows the profiles, the themes, the active ones and the language.
  $effect(() => {
    updateTrayMenu();
  });

  // Named keys (Enter, Shift…) are drawn in the interface language.
  $effect(() => {
    void language.active;
    untrack(refreshLegends);
  });

  /** Legends come from the Windows keyboard layout and the interface language: takes them again,
   *  replacing the layout only when a legend, or the shape guessed from the layout, changed. */
  async function refreshLegends() {
    const current = layout;
    if (!current) return;
    const fresh = await getLayout().catch(() => null);
    if (!fresh || fresh.id !== current.id) return;
    if (fresh.standard !== current.standard) {
      support = (await keyboardLayout()).support;
      selected.clear();
      layout = fresh;
    } else if (fresh.keys.some((k, i) => k.label !== current.keys[i]?.label)) {
      layout = fresh;
    }
  }

  /** The user chose another shape (ISO/ANSI) or Windows layout: redraw. Nothing goes to the keyboard. */
  async function keyboardLayoutChanged(state: KeyboardLayoutState) {
    support = state.support;
    const fresh = await getLayout();
    if (fresh.standard !== layout?.standard) selected.clear();
    layout = fresh;
  }

  function updateTrayMenu() {
    const item = (p: { id: string; name: string }) => ({ id: p.id, name: p.name });
    setTrayMenu({
      show: t("tray.show"),
      quit: t("tray.quit"),
      profiles: profiles.map(item),
      active_profile: activeProfileId,
      themes_title: t("tray.themes"),
      themes: themeStore.themes.map(item),
      presets: builtPresets.map(item),
      active_theme: dirty ? null : themeId,
    }).catch((e) => console.warn(`Menu della tray non aggiornato: ${e}`));
  }

  async function showWindow() {
    const win = getCurrentWindow();
    await win.unminimize();
    await win.show();
    await win.setFocus();
  }

  /** A key press on the real keyboard or a click in the preview, for the reactive effects. */
  function previewPress(slot: number) {
    simulator?.press(slot, performance.now() / 1000);
  }

  async function init() {
    layout = await getLayout();
    try {
      themeStore = await loadThemes();
    } catch (e) {
      status = String(e);
    }
    let active: string | null = null;
    try {
      const disk = await loadUserProfiles();
      profiles = disk.profiles;
      active = disk.active;
    } catch (e) {
      status = String(e);
    }
    if (profiles.length === 0) await createFirstProfile();
    const first = profiles.find((p) => p.id === active) ?? profiles[0];
    activeProfileId = first.id;
    keyConfig = first.keys ? cloneConfig(first.keys) : null;
    await reconnect();
    activate(first, true);
  }

  /** The first time: the default theme, keys as the keyboard has them. */
  async function createFirstProfile() {
    const first: UserProfile = {
      id: crypto.randomUUID(),
      name: t("profiles.first_name"),
      created: Date.now(),
      lighting: $state.snapshot(lightingOf(DEFAULT_PRESET)!) as Lighting,
      theme: DEFAULT_PRESET,
      keys: null,
    };
    profiles.push(first);
    await saveUserProfile(first).catch((e) => (status = String(e)));
  }

  // ---- Lighting snapshots ----

  function currentLighting(): Lighting {
    return {
      mode,
      background: [...background] as Rgb,
      overrides: Object.fromEntries([...overrides].map(([slot, c]) => [String(slot), [...c] as Rgb])),
      intensity,
      effect: { ...effectSetup, color: [...effectSetup.color] as Rgb },
    };
  }

  function applyLighting(l: Lighting) {
    overrides.clear();
    for (const [slot, c] of Object.entries(l.overrides)) overrides.set(Number(slot), [...c] as Rgb);
    background = [...l.background] as Rgb;
    intensity = l.intensity;
    mode = l.mode ?? "per_key";
    const e = l.effect ?? DEFAULT_EFFECT;
    effectSetup = { ...e, color: [...e.color] as Rgb };
  }

  /** Same result on the keyboard: compares the color each key ends up with, not how it was set. */
  function sameLighting(a: Lighting, b: Lighting): boolean {
    if (a.intensity !== b.intensity || (a.mode ?? "per_key") !== (b.mode ?? "per_key")) return false;
    if (canonical(a.effect ?? DEFAULT_EFFECT) !== canonical(b.effect ?? DEFAULT_EFFECT)) return false;
    return allSlots.every((slot) => {
      const ca = a.overrides[slot] ?? a.background;
      const cb = b.overrides[slot] ?? b.background;
      return ca[0] === cb[0] && ca[1] === cb[1] && ca[2] === cb[2];
    });
  }

  // ---- Undo / redo ----

  function recordEdit() {
    if (groupTimer === undefined) {
      if (committed) undoStack = [...undoStack, committed].slice(-UNDO_LIMIT);
      redoStack = [];
    }
    clearTimeout(groupTimer);
    groupTimer = setTimeout(endEditGroup, UNDO_GROUP_GAP);
  }

  function endEditGroup() {
    clearTimeout(groupTimer);
    groupTimer = undefined;
    committed = currentLighting();
  }

  function resetHistory() {
    clearTimeout(groupTimer);
    groupTimer = undefined;
    undoStack = [];
    redoStack = [];
    committed = currentLighting();
  }

  function restore(l: Lighting) {
    applyLighting(l);
    committed = l;
    scheduleSend();
    profileChanged();
  }

  function undo() {
    if (groupTimer !== undefined) endEditGroup();
    const prev = undoStack.at(-1);
    if (!prev) return;
    undoStack = undoStack.slice(0, -1);
    redoStack = [...redoStack, currentLighting()];
    restore(prev);
  }

  function redo() {
    if (groupTimer !== undefined) endEditGroup();
    const next = redoStack.at(-1);
    if (!next) return;
    redoStack = redoStack.slice(0, -1);
    undoStack = [...undoStack, currentLighting()];
    restore(next);
  }

  // ---- Themes ----
  // A theme is a saved lighting setup. Choosing one copies its colors into the active profile;
  // the profile remembers which theme they came from, to show when they have been changed.

  function lightingOf(id: string): Lighting | undefined {
    return themeStore.themes.find((p) => p.id === id) ?? builtPresets.find((p) => p.id === id)?.lighting;
  }

  function themeName(id: string | null): string | null {
    if (!id) return null;
    return themeStore.themes.find((p) => p.id === id)?.name ?? builtPresets.find((p) => p.id === id)?.name ?? null;
  }

  /** Puts a theme's colors on the active profile, as one undo step. */
  function applyTheme(id: string) {
    const lighting = lightingOf(id);
    if (!lighting || !profile || (id === themeId && !dirty)) return;
    if (groupTimer !== undefined) endEditGroup();
    undoStack = [...undoStack, currentLighting()].slice(-UNDO_LIMIT);
    redoStack = [];
    applyLighting($state.snapshot(lighting) as Lighting);
    committed = currentLighting();
    profile.theme = id;
    baseline = currentLighting();
    profileChanged();
    clearTimeout(sendTimer);
    if (autoSend && connected) send();
  }

  /** Saves the colors into the active user theme, or asks for a name when a preset is active. */
  function save() {
    if (!dirty) return;
    if (!activeTheme) {
      startNaming(t("themes.copy_of", { name: activeName }));
      return;
    }
    Object.assign(activeTheme, currentLighting());
    baseline = currentLighting();
    status = t("status.theme_saved", { name: activeTheme.name });
    return persistThemes();
  }

  /** Puts the theme's colors back. Undo brings the edits back. */
  function revert() {
    if (!baseline || !dirty) return;
    if (groupTimer !== undefined) endEditGroup();
    undoStack = [...undoStack, currentLighting()].slice(-UNDO_LIMIT);
    redoStack = [];
    restore(baseline);
    status = t("status.reverted", { name: activeName });
  }

  async function startNaming(defaultName: string) {
    naming = true;
    newName = defaultName;
    await tick();
    nameInput?.focus();
    nameInput?.select();
  }

  function commitNaming() {
    const name = newName.trim();
    naming = false;
    if (name) createTheme(name);
  }

  /** Saves the current colors as a new theme, which the active profile then follows. */
  function createTheme(name: string) {
    const id = crypto.randomUUID();
    themeStore.themes.push({ id, name, ...currentLighting() });
    if (profile) {
      profile.theme = id;
      profileChanged();
    }
    baseline = currentLighting();
    persistThemes();
    status = t("status.theme_saved", { name });
    return id;
  }

  async function persistThemes() {
    try {
      await saveThemes($state.snapshot(themeStore));
    } catch (e) {
      status = String(e);
    }
  }

  function renameTheme(id: string, name: string) {
    const p = themeStore.themes.find((p) => p.id === id);
    if (!p) return;
    p.name = name;
    persistThemes();
  }

  function duplicateTheme(id: string) {
    const i = themeStore.themes.findIndex((p) => p.id === id);
    if (i < 0) return;
    const copy = { ...$state.snapshot(themeStore.themes[i]), id: crypto.randomUUID() };
    copy.name = t("themes.duplicate_name", { name: copy.name });
    themeStore.themes.splice(i + 1, 0, copy);
    persistThemes();
  }

  /** Profiles using the theme keep their colors; they just stop following a theme. */
  function deleteTheme(id: string) {
    const p = themeStore.themes.find((p) => p.id === id);
    themeStore.themes = themeStore.themes.filter((p) => p.id !== id);
    for (const other of profiles.filter((o) => o.theme === id)) {
      other.theme = null;
      if (other.id === activeProfileId) profileChanged();
      else saveUserProfile($state.snapshot(other) as UserProfile).catch((e) => (status = String(e)));
    }
    if (themeId === null) baseline = null;
    persistThemes();
    if (p) status = t("status.theme_deleted", { name: p.name });
  }

  // ---- Profiles ----
  // A profile is what the keyboard does as a whole: lighting, key functions and macros.
  // Only the active one is edited, and it saves itself shortly after every edit.

  /** Saves the active profile soon: edits close together make one write to disk. */
  function profileChanged() {
    if (previewOnly) return;
    clearTimeout(profileTimer);
    profileTimer = setTimeout(flushProfile, PROFILE_SAVE_DELAY);
  }

  /** Copies the current lighting and keys into the active profile and saves it. */
  async function flushProfile() {
    clearTimeout(profileTimer);
    profileTimer = undefined;
    if (!profile || previewOnly) return;
    profile.lighting = currentLighting();
    if (keyConfig) profile.keys = cloneConfig($state.snapshot(keyConfig) as KeyConfig);
    try {
      await saveUserProfile($state.snapshot(profile) as UserProfile);
    } catch (e) {
      status = String(e);
    }
  }

  async function selectProfile(id: string) {
    const next = profiles.find((p) => p.id === id);
    if (!next || id === activeProfileId) return;
    await flushProfile();
    activate(next);
  }

  /**
   * Makes a profile the active one: its colors are sent only if they differ from what is on, and
   * its keys are written only if they differ from the keyboard's (both live in its memory). At
   * start the keys are not written: a difference shows in the Keys section instead.
   */
  function activate(next: UserProfile, starting = false) {
    const before = currentLighting();
    activeProfileId = next.id;
    setActiveProfile(next.id).catch((e) => (status = String(e)));
    applyLighting($state.snapshot(next.lighting) as Lighting);
    const theme = next.theme ? lightingOf(next.theme) : undefined;
    baseline = theme ? ($state.snapshot(theme) as Lighting) : null;
    resetHistory();
    clearTimeout(sendTimer);
    if (autoSend && connected && (starting || !sameLighting(before, next.lighting))) send();

    clearTimeout(keysTimer);
    keysPending = false;
    keyUndo = [];
    keyRedo = [];
    if (next.keys) {
      keyConfig = cloneConfig(next.keys);
      if (!starting && keysOnBoard && !sameConfig(keyConfig, keysOnBoard)) scheduleKeysWrite();
    } else if (keysOnBoard) {
      // A profile made while the keyboard was away takes the keyboard's keys.
      keyConfig = cloneConfig(keysOnBoard);
      profileChanged();
    } else {
      keyConfig = null;
    }
    if (!starting) status = t("status.profile_active", { name: next.name });
  }

  /** Factory keys and no macros; `null` while the keyboard's factory keys are unknown. */
  function factoryKeys(): KeyConfig | null {
    return Object.keys(keyDefaults).length ? { keys: { ...keyDefaults }, macros: [] } : null;
  }

  /** A copy of `from`, or factory keys with the current colors; it becomes the active profile. */
  async function createProfile(name: string, from: string | null) {
    await flushProfile();
    const source = profiles.find((p) => p.id === from);
    const created: UserProfile = source
      ? { ...($state.snapshot(source) as UserProfile), id: crypto.randomUUID(), name, created: Date.now() }
      : {
          id: crypto.randomUUID(),
          name,
          created: Date.now(),
          lighting: currentLighting(),
          theme: dirty ? null : themeId,
          keys: factoryKeys(),
        };
    profiles.push(created);
    await saveUserProfile(created).catch((e) => (status = String(e)));
    activate(profiles.at(-1)!);
    status = t("status.profile_created", { name });
    return created.id;
  }

  function renameProfile(id: string, name: string) {
    const p = profiles.find((p) => p.id === id);
    if (!p) return;
    p.name = name;
    if (id === activeProfileId) profileChanged();
    else saveUserProfile($state.snapshot(p) as UserProfile).catch((e) => (status = String(e)));
  }

  /** The last profile can't go; deleting the active one switches to the one before it. */
  async function deleteProfile(id: string) {
    const i = profiles.findIndex((p) => p.id === id);
    if (i < 0 || profiles.length < 2) return;
    const removed = profiles[i];
    if (id === activeProfileId) {
      clearTimeout(profileTimer);
      profileTimer = undefined;
      activate(profiles[i === 0 ? 1 : i - 1]);
    }
    profiles.splice(i, 1);
    try {
      await deleteUserProfile(id);
      status = t("status.profile_deleted", { name: removed.name });
    } catch (e) {
      status = String(e);
    }
  }

  // ---- Profile files ----
  // A profile can be saved as a file to keep or share, and added back on any PC. The file has the
  // colors, keys and macros; a theme is kept only if it is a preset, which exists everywhere.

  const PROFILE_FORMAT = "lucent-profile";

  /** A file that can't be imported; `key` names the reason in the language files. */
  class ProfileFileError extends Error {
    key: string;
    params: Record<string, string | number>;
    constructor(key: string, params: Record<string, string | number> = {}) {
      super(tEn(key, params));
      this.key = key;
      this.params = params;
    }
  }

  /** Writes the profile into the exports folder; resolves with the file's path. */
  async function exportProfile(id: string, reveal = true): Promise<string> {
    const p = profiles.find((p) => p.id === id);
    if (!p) throw new Error(`No profile ${id}`);
    const active = id === activeProfileId;
    const file = {
      format: PROFILE_FORMAT,
      version: 1,
      layout: device?.info.layout ?? null,
      profile: {
        name: p.name,
        lighting: active ? currentLighting() : $state.snapshot(p.lighting),
        theme: p.theme?.startsWith("preset:") ? p.theme : null,
        keys: active && keyConfig ? $state.snapshot(keyConfig) : $state.snapshot(p.keys),
      },
    };
    const name = `${p.name.replace(/[\\/:*?"<>|]/g, "-")}.lucent-profile.json`;
    const path = await saveExport(name, new TextEncoder().encode(JSON.stringify(file, null, 2)), reveal);
    status = t("status.profile_exported", { path });
    return path;
  }

  /** Adds the profile in a file, under a free name. It does not become active. */
  async function importProfile(text: string): Promise<UserProfile> {
    let file: any;
    try {
      // Notepad and PowerShell may save with a byte order mark, which JSON.parse refuses.
      file = JSON.parse(text.replace(/^﻿/, ""));
    } catch {
      throw new ProfileFileError("profiles.import_invalid");
    }
    const p = file?.profile;
    const rgb = (c: unknown) => Array.isArray(c) && c.length === 3 && c.every((x) => Number.isInteger(x) && x >= 0 && x <= 255);
    const lighting = p?.lighting;
    const valid =
      file?.format === PROFILE_FORMAT &&
      typeof p?.name === "string" &&
      rgb(lighting?.background) &&
      typeof lighting?.overrides === "object" &&
      Object.values(lighting.overrides ?? {}).every(rgb) &&
      Number.isFinite(lighting?.intensity) &&
      (p.keys === null || (typeof p.keys?.keys === "object" && Array.isArray(p.keys?.macros)));
    if (!valid || (p.keys && !validKeys(p.keys))) throw new ProfileFileError("profiles.import_invalid");
    if (file.version !== 1) throw new ProfileFileError("profiles.import_newer");
    const here = device?.info.layout;
    if (file.layout && here && file.layout !== here) {
      throw new ProfileFileError("profiles.import_other_keyboard", { layout: file.layout });
    }
    const max = keymapSupport?.max_macros;
    if (p.keys && max !== undefined && p.keys.macros.length > max) {
      throw new ProfileFileError("profiles.import_too_many_macros", { n: p.keys.macros.length, max });
    }
    const base = p.name.trim() || t("profiles.new_default", { n: profiles.length + 1 });
    let name = base;
    for (let i = 2; profiles.some((o) => o.name.toLowerCase() === name.toLowerCase()); i++) name = `${base} (${i})`;
    const theme = typeof p.theme === "string" && PRESETS.some((x) => x.id === p.theme) ? p.theme : null;
    const created: UserProfile = {
      id: crypto.randomUUID(),
      name,
      created: Date.now(),
      lighting: { ...lighting, intensity: Math.min(100, Math.max(0, Math.round(lighting.intensity))) },
      theme,
      keys: p.keys,
    };
    profiles.push(created);
    await saveUserProfile(created);
    status = t("status.profile_imported", { name });
    return created;
  }

  /**
   * Key functions from a file: only what the app itself can set (keys, media, macros, disabled)
   * and well-formed macros. Raw codes and Fn are what the keyboard had, never a file's to bring:
   * they pass only where they match the factory function.
   */
  function validKeys(keys: any): boolean {
    if (!keys.keys) return false;
    const byte = (x: unknown) => Number.isInteger(x) && (x as number) >= 0 && (x as number) <= 255;
    const macros: unknown[] = keys.macros;
    const macrosOk = macros.every(
      (m: any) =>
        typeof m?.name === "string" &&
        Number.isInteger(m.repeat) &&
        Array.isArray(m.steps) &&
        m.steps.every(
          (s: any) => byte(s?.code) && typeof s.pressed === "boolean" && Number.isFinite(s.delay_ms) && s.delay_ms >= 0,
        ),
    );
    return (
      macrosOk &&
      Object.entries(keys.keys).every(([slot, a]: [string, any]) => {
        switch (a?.kind) {
          case "key":
            return byte(a.code);
          case "media":
            return Number.isInteger(a.code) && a.code >= 0 && a.code <= 0xffff;
          case "macro":
            return Number.isInteger(a.index) && a.index >= 0 && a.index < macros.length;
          case "disabled":
            return true;
          default:
            return keyDefaults[slot] !== undefined && sameAction(a, keyDefaults[slot]);
        }
      })
    );
  }

  async function importProfileFile(f: File) {
    try {
      await importProfile(await f.text());
    } catch (e) {
      status = e instanceof ProfileFileError ? t(e.key, e.params) : String(e);
    }
  }

  /** "Synthwave · 3 keys changed · 1 macro", for the profile menu. */
  function describeProfile(p: UserProfile): string {
    const parts: string[] = [];
    const theme = themeName(p.theme);
    const themeLighting = p.theme ? lightingOf(p.theme) : undefined;
    if (!theme || !themeLighting) parts.push(t("themes.custom"));
    else if (sameLighting(themeLighting, p.lighting)) parts.push(theme);
    else parts.push(t("profiles.theme_edited", { name: theme }));
    if (p.keys && Object.keys(keyDefaults).length) {
      const changed = allSlots.filter((s) => !sameAction(p.keys!.keys[s] ?? keyDefaults[s], keyDefaults[s])).length;
      if (changed) parts.push(plural("profiles.keys", changed));
      if (p.keys.macros.length) parts.push(plural("profiles.macros", p.keys.macros.length));
    }
    return parts.join(" · ");
  }

  // ---- Keyboard ----

  /** How often the USB device list is checked for the keyboard coming or going. */
  const WATCH_INTERVAL = 2000;
  /** After a reconnect that failed with the keyboard plugged in, wait this long before the next. */
  const RETRY_DELAY = 10000;
  let lastAutoConnect = 0;
  /** Lighting was edited while the keyboard was away: send it when it's back. */
  let unsentLighting = false;
  let reconnecting = false;

  /** Follows the keyboard being unplugged and plugged back, without sending it anything. */
  async function watchKeyboard() {
    if (reconnecting) return;
    const change = await checkKeyboard().catch(() => "same" as const);
    if (change === "lost") {
      connected = false;
      device = null;
      support = "none";
      unknownKeyboard = null;
      keysOnBoard = null;
      status = t("status.keyboard_unplugged");
    } else if (change === "available" && Date.now() - lastAutoConnect > RETRY_DELAY) {
      lastAutoConnect = Date.now();
      await reconnect();
      if (connected) lastAutoConnect = 0;
      if (connected && unsentLighting && autoSend) {
        unsentLighting = false;
        send();
      }
    }
  }

  async function reconnect() {
    reconnecting = true;
    try {
      await connectNow();
    } finally {
      reconnecting = false;
    }
  }

  async function connectNow() {
    status = t("app.connecting");
    const s = await connect();
    connected = s.connected;
    status = s.message;
    support = s.support;
    unknownKeyboard = s.unknown;
    layout = await getLayout();
    if (support === "unconfigured" && formats.length === 0) {
      formats = (await listLayouts()).filter((l) => l.kind === "format");
    }
    device = connected ? await deviceInfo().catch(() => null) : null;
    await loadKeys();
  }

  /** Draws a keyboard without a model with another format. Nothing is sent to the keyboard. */
  async function choosePreviewFormat(id: string) {
    const known = formats.find((f) => f.id === id || f.name.toLowerCase() === id.trim().toLowerCase());
    if (!known) throw new Error(`Unknown format "${id}". Call list_models for the formats.`);
    layout = await setPreviewFormat(known.id);
    selected.clear();
  }

  // ---- Key functions ----

  /** Reads the keyboard's keys. A profile without keys yet takes them; otherwise the profile's win. */
  async function loadKeys() {
    if (!connected || !device?.capabilities.keymap) {
      keysOnBoard = null;
      return;
    }
    try {
      const board = await readKeys();
      keyDefaults = board.defaults;
      keysOnBoard = board.config;
      if (!keyConfig) {
        keyConfig = cloneConfig(board.config);
        profileChanged();
      }
    } catch (e) {
      keysError = String(e);
    }
  }

  /** Every key-function edit goes through here: one undo step, saved, then written. */
  function updateKeys(next: KeyConfig) {
    if (keyConfig) keyUndo = [...keyUndo, cloneConfig(keyConfig)].slice(-UNDO_LIMIT);
    keyRedo = [];
    applyKeys(next);
  }

  function applyKeys(next: KeyConfig) {
    keyConfig = next;
    profileChanged();
    scheduleKeysWrite();
  }

  function undoKeys() {
    const prev = keyUndo.at(-1);
    if (!prev || !keyConfig) return;
    keyUndo = keyUndo.slice(0, -1);
    keyRedo = [...keyRedo, cloneConfig(keyConfig)];
    applyKeys(prev);
  }

  function redoKeys() {
    const next = keyRedo.at(-1);
    if (!next || !keyConfig) return;
    keyRedo = keyRedo.slice(0, -1);
    keyUndo = [...keyUndo, cloneConfig(keyConfig)];
    applyKeys(next);
  }

  function scheduleKeysWrite() {
    if (!connected || !keymapSupport || !keysAllowed) return;
    clearTimeout(keysTimer);
    keysPending = true;
    keysTimer = setTimeout(writeKeysNow, KEYS_WRITE_DELAY);
  }

  async function writeKeysNow() {
    if (!keyConfig || !keysAllowed) return;
    if (keysWriting) {
      keysRewrite = true;
      return;
    }
    keysWriting = true;
    keysError = null;
    const config = cloneConfig($state.snapshot(keyConfig) as KeyConfig);
    try {
      await writeKeys(config);
      keysOnBoard = config;
      status = t("status.keys_written");
    } catch (e) {
      keysError = String(e);
      status = String(e);
    } finally {
      keysWriting = false;
      if (keysRewrite) {
        keysRewrite = false;
        writeKeysNow();
      } else {
        keysPending = false;
      }
    }
  }

  /** `null` puts the key back to its factory function. */
  function setKeyAction(slot: number, action: KeyAction | null) {
    if (!keyConfig) return;
    const next = cloneConfig(keyConfig);
    next.keys[slot] = action ?? keyDefaults[slot];
    updateKeys(next);
  }

  function resetKeys(slots: number[]) {
    if (!keyConfig) return;
    const next = cloneConfig(keyConfig);
    for (const slot of slots) if (keyDefaults[slot]) next.keys[slot] = keyDefaults[slot];
    updateKeys(next);
  }

  /** Saves a macro (new when `index` is null); a new one made from a key's panel goes on that key. */
  function saveMacro(index: number | null, macro: Macro, forSlot?: number) {
    if (!keyConfig) return;
    const next = cloneConfig(keyConfig);
    if (index === null) {
      next.macros.push(macro);
      if (forSlot !== undefined) next.keys[forSlot] = { kind: "macro", index: next.macros.length - 1 };
    } else {
      next.macros[index] = macro;
    }
    updateKeys(next);
    status = t("status.macro_saved", { name: macro.name });
  }

  /** Keys playing the macro go back to their factory function; later macros move up by one. */
  function deleteMacro(index: number) {
    if (!keyConfig) return;
    const next = cloneConfig(keyConfig);
    const [removed] = next.macros.splice(index, 1);
    for (const [slot, action] of Object.entries(next.keys)) {
      if (action.kind !== "macro") continue;
      if (action.index === index) next.keys[slot] = keyDefaults[slot];
      else if (action.index > index) next.keys[slot] = { kind: "macro", index: action.index - 1 };
    }
    updateKeys(next);
    if (removed) status = t("status.macro_deleted", { name: removed.name });
  }

  /** Takes what the keyboard has (e.g. set by another program) as the app's keys. */
  function useBoardKeys() {
    if (keysOnBoard) updateKeys(cloneConfig(keysOnBoard));
    clearTimeout(keysTimer);
    keysPending = false;
  }

  async function send() {
    if (sending) {
      resend = true;
      return;
    }
    sending = true;
    try {
      if (mode === "effect") {
        await setEffect({
          effect: effectSetup.id,
          brightness: Math.round(intensity / 25),
          speed: effectSetup.speed,
          direction: effectSetup.direction,
          color: effectSetup.random ? null : effectSetup.color,
        });
        status = t("status.effect_active", { effect: effectName(effectSetup.id, effectInfo?.name) });
      } else {
        const k = intensity / 100;
        const scaled = colors.map((c) => c.map((x) => Math.round(x * k)) as Rgb);
        lastSendMs = await sendColors(scaled, intensity === 0 ? 0 : HW_BRIGHTNESS_MAX);
        status = t("status.colors_sent", { ms: lastSendMs });
      }
    } catch (e) {
      status = String(e);
      connected = false;
      unsentLighting = true;
    } finally {
      sending = false;
      if (resend) {
        resend = false;
        send();
      }
    }
  }

  function scheduleSend() {
    if (!connected) unsentLighting = true;
    if (!autoSend || !connected) return;
    clearTimeout(sendTimer);
    sendTimer = setTimeout(send, AUTO_SEND_DELAY);
  }

  /** Every user edit goes through here, after changing the lighting. */
  function changed() {
    scheduleSend();
    profileChanged();
  }

  /** Starts an edit of the per-key colors, which brings the keyboard back to per-key mode. */
  function recordPerKeyEdit() {
    recordEdit();
    mode = "per_key";
  }

  function setMode(next: Mode) {
    if (next === mode) return;
    recordEdit();
    mode = next;
    changed();
  }

  function updateEffect(next: EffectState) {
    recordEdit();
    mode = "effect";
    effectSetup = next;
    changed();
  }

  function paint(rgb: Rgb, slots: Iterable<number> = selected) {
    const list = [...slots];
    if (list.length === 0) return;
    recordPerKeyEdit();
    for (const slot of list) overrides.set(slot, [...rgb] as Rgb);
    changed();
  }

  /** Puts the selected keys back on the background color. */
  function resetSelection() {
    recordPerKeyEdit();
    for (const slot of selected) overrides.delete(slot);
    changed();
  }

  function setBackground(rgb: Rgb) {
    recordPerKeyEdit();
    background = rgb;
    changed();
  }

  function setIntensity(value: number) {
    recordEdit();
    intensity = value;
    changed();
  }

  /** A color picked in the picker or the palette goes to the active target. */
  function pick(rgb: Rgb, fromPalette = false) {
    if (!fromPalette) paletteBase = rgb;
    if (target === "background") {
      setBackground(rgb);
    } else {
      brush = rgb;
      paint(rgb);
    }
  }

  /** Makes a key's color the active color, without painting anything. */
  function eyedrop(slot: number) {
    const color = [...colors[slot]] as Rgb;
    target = "keys";
    brush = color;
    paletteBase = color;
    status = t("status.color_picked", { hex: toHex(color) });
  }

  function select(slots: number[]) {
    selected.clear();
    for (const s of slots) selected.add(s);
  }

  /** Paints every key from a picture (see lib/image.ts), as one undo step. */
  function applyImageColors(map: Map<number, Rgb>, name: string) {
    recordPerKeyEdit();
    for (const [slot, color] of map) overrides.set(slot, color);
    changed();
    status = t("status.image_applied", { name });
  }

  async function openImage(file: File | undefined) {
    if (!file) return;
    try {
      pendingImage = { bitmap: await createImageBitmap(file), name: file.name };
    } catch {
      status = t("status.not_an_image", { name: file.name });
    }
  }

  async function downloadTemplate() {
    if (!layout) return;
    try {
      const png = new Uint8Array(await (await keyTemplate(layout)).arrayBuffer());
      const path = await saveExport(t("inspector.template_file"), png);
      status = t("status.template_saved", { path });
    } catch (e) {
      status = String(e);
    }
  }

  /** Background, letters, numbers, modifiers and the rest each get one palette color. */
  function applyToGroups() {
    const byId = (id: string) => groupSlots(layout!, id);
    recordPerKeyEdit();
    overrides.clear();
    background = palette[0];
    const layers: [number[], Rgb][] = [
      [byId("letters"), palette[1]],
      [[...byId("numbers"), ...byId("fkeys")], palette[2]],
      [byId("modifiers"), palette[3]],
      [[...byId("arrows"), ...byId("navigation"), ...byId("numpad")], palette[4]],
    ];
    for (const [slots, color] of layers) for (const slot of slots) overrides.set(slot, color);
    changed();
  }

  /** Left-to-right gradient through the palette, on every key. */
  function applyGradient() {
    gradient(palette, allSlots, "horizontal");
  }

  /** Gradient through `stops` over the given keys, following their position. */
  function gradient(stops: Rgb[], slots: number[], direction: "horizontal" | "vertical") {
    if (!layout || slots.length === 0) return;
    const horizontal = direction === "horizontal";
    const size = horizontal ? layout.width : layout.height;
    recordPerKeyEdit();
    for (const key of layout.keys) {
      if (!slots.includes(key.slot)) continue;
      const center = horizontal ? key.x + key.w / 2 : key.y + key.h / 2;
      overrides.set(key.slot, samplePalette(stops, center / size));
    }
    changed();
  }

  // ---- MCP tools ----
  // Called by AI agents through the local MCP server (src-tauri/src/mcp.rs). Every edit goes
  // through the same functions as the UI, as its own undo step. Errors explain how to fix the call.

  const keyIds = $derived(new Map(layout?.keys.map((k) => [k.slot, k.id]) ?? []));

  function parseColor(value: string): Rgb {
    const rgb = fromHex(value);
    if (!rgb) throw new Error(`Invalid color "${value}": use a hex string like "#FF3B30"`);
    return rgb;
  }

  /** Slots for key ids/labels and group ids. Empty selection means no keys. */
  function resolveKeys(sel: { keys?: string[]; groups?: string[] }): number[] {
    if (!layout) throw new Error("Keyboard layout not loaded yet");
    const slots = new Set<number>();
    const unknown: string[] = [];
    for (const name of sel.keys ?? []) {
      const n = name.trim().toLowerCase();
      const byId = layout.keys.find((k) => k.id.toLowerCase() === n);
      const byLabel = layout.keys.filter((k) => k.label.toLowerCase() === n);
      const key = byId ?? (byLabel.length === 1 ? byLabel[0] : undefined);
      if (key) slots.add(key.slot);
      else if (byLabel.length > 1) unknown.push(`${name} (ambiguous: ${byLabel.map((k) => k.id).join(" or ")})`);
      else unknown.push(name);
    }
    for (const name of sel.groups ?? []) {
      const n = name.trim().toLowerCase();
      const group = GROUPS.find(
        (g) => g.id === n || t(`group.${g.id}`).toLowerCase() === n || tEn(`group.${g.id}`).toLowerCase() === n,
      );
      if (n === "all") allSlots.forEach((s) => slots.add(s));
      else if (group) groupSlots(layout, group.id).forEach((s) => slots.add(s));
      else unknown.push(`group:${name}`);
    }
    if (unknown.length) {
      throw new Error(`Unknown keys or groups: ${unknown.join(", ")}. Call list_keys for valid ids.`);
    }
    return [...slots];
  }

  /** Runs one AI edit as a single undo step. */
  function aiEdit(fn: () => void) {
    if (groupTimer !== undefined) endEditGroup();
    fn();
    if (groupTimer !== undefined) endEditGroup();
  }

  function findThemeId(ref: string, userOnly = false): string {
    const r = ref.trim().toLowerCase();
    const all = [...themeStore.themes, ...(userOnly ? [] : builtPresets)];
    const names = (p: { id: string; name: string }) =>
      p.id.startsWith("preset:") ? [p.name, tEn(presetKey(p.id))] : [p.name];
    const match =
      all.find((p) => p.id.toLowerCase() === r) ?? all.find((p) => names(p).some((n) => n.toLowerCase() === r));
    if (!match) {
      const what = userOnly ? "user theme" : "theme or preset";
      throw new Error(`No ${what} named "${ref}". Call list_themes to see them.`);
    }
    return match.id;
  }

  function findProfile(ref: string): UserProfile {
    const r = ref.trim().toLowerCase();
    const match = profiles.find((p) => p.id.toLowerCase() === r) ?? profiles.find((p) => p.name.toLowerCase() === r);
    if (!match) throw new Error(`No profile named "${ref}". Call list_profiles to see them.`);
    return match;
  }

  /** A profile as MCP answers describe it. */
  function mcpProfile(p: UserProfile) {
    const themeLighting = p.theme ? lightingOf(p.theme) : undefined;
    const keys = p.id === activeProfileId ? keyConfig : p.keys;
    const factory = Object.keys(keyDefaults).length > 0;
    return {
      id: p.id,
      name: p.name,
      active: p.id === activeProfileId,
      theme: themeLighting
        ? { id: p.theme, name: themeName(p.theme), edited: !sameLighting(themeLighting, p.id === activeProfileId ? currentLighting() : p.lighting) }
        : null,
      changed_keys:
        keys && factory ? allSlots.filter((s) => !sameAction(keys.keys[s] ?? keyDefaults[s], keyDefaults[s])).length : null,
      macros: keys?.macros.map((m) => m.name) ?? null,
    };
  }

  function stateSummary() {
    return {
      connected,
      profile: profile ? { id: profile.id, name: profile.name } : null,
      theme: themeId
        ? { id: themeId, name: activeName, kind: activeTheme ? "user" : "preset", edited: dirty }
        : null,
      mode,
      effect:
        mode === "effect"
          ? {
              id: effectSetup.id,
              name: effectName(effectSetup.id, effectInfo?.name),
              speed: effectSetup.speed,
              direction: effectSetup.direction,
              color: effectSetup.random ? "random" : toHex(effectSetup.color),
              hardware_brightness: Math.round(intensity / 25),
            }
          : null,
      background: toHex(background),
      intensity,
      custom_keys: Object.fromEntries(
        [...overrides].filter(([slot]) => keyIds.has(slot)).map(([slot, c]) => [keyIds.get(slot), toHex(c)]),
      ),
      auto_send: autoSend,
      ui_language: language.active,
      ui_appearance: { setting: appearance.setting, in_use: appearance.resolved },
      preview_look: { case: look.case, keys: look.keys },
      can_undo: undoStack.length > 0 || groupTimer !== undefined,
      can_redo: redoStack.length > 0,
    };
  }

  /** Key functions as MCP answers describe them. */
  function mcpAction(action: KeyAction | undefined, names: KeyNames, config: KeyConfig) {
    switch (action?.kind) {
      case "key":
        return { type: "key", key: names.id(action.code) ?? action.code, label: names.label(action.code) };
      case "media":
        return { type: "media", media: mediaId(action.code) ?? action.code };
      case "macro":
        return { type: "macro", macro: config.macros[action.index]?.name ?? null };
      case "disabled":
        return { type: "disabled" };
      default:
        return { type: action?.kind ?? "default" };
    }
  }

  /** Checks that key functions can be read, and changed unless `reading`. */
  function keysReady(reading = false) {
    if (!connected) throw new Error("Keyboard not connected");
    if (!keymapSupport) throw new Error("This keyboard's key functions can't be changed");
    if (!keysAllowed && !reading) {
      throw new Error(
        "This keyboard's model is not verified: the user must allow key changes in the app's Keys section first",
      );
    }
    if (!keyConfig || !keyNames) throw new Error("Key functions not loaded yet: retry in a moment, or read get_log");
    return { names: keyNames, config: keyConfig };
  }

  const mcpTools: Record<string, ToolHandler> = {
    get_state: () => stateSummary(),

    list_keys: () => ({
      model: layout?.name,
      layout: layout?.id,
      preview_only: previewOnly,
      keys: layout?.keys.map((k) => ({ id: k.id, label: k.label || "(space)", x: k.x, y: k.y, w: k.w, h: k.h })),
      groups: [
        ...GROUPS.map((g) => ({ id: g.id, keys: g.keys.filter((id) => layout?.keys.some((k) => k.id === id)) })),
        { id: "all", keys: "every key" },
      ],
      notes: `x/y are in key units from the top-left corner; the keyboard is ${layout?.width} x ${layout?.height} units.`,
    }),

    paint_keys: (args: { assignments: { keys?: string[]; groups?: string[]; color: string }[] }) => {
      const plan = args.assignments.map((a) => ({ slots: resolveKeys(a), color: parseColor(a.color) }));
      aiEdit(() => {
        recordPerKeyEdit();
        for (const { slots, color } of plan) for (const s of slots) overrides.set(s, color);
        changed();
      });
      return { painted_keys: new Set(plan.flatMap((p) => p.slots)).size, state: stateSummary() };
    },

    reset_keys: (args: { keys?: string[]; groups?: string[] }) => {
      const slots = resolveKeys(args);
      aiEdit(() => {
        recordPerKeyEdit();
        for (const s of slots) overrides.delete(s);
        changed();
      });
      return { reset_keys: slots.length };
    },

    set_background: (args: { color: string }) => {
      const color = parseColor(args.color);
      aiEdit(() => setBackground(color));
      return stateSummary();
    },

    set_intensity: (args: { percent: number }) => {
      const p = Math.round(Math.min(100, Math.max(0, args.percent)));
      aiEdit(() => setIntensity(p));
      return { intensity: p };
    },

    apply_gradient: (args: { colors: string[]; direction?: "horizontal" | "vertical"; keys?: string[]; groups?: string[] }) => {
      if (args.colors.length < 2) throw new Error("A gradient needs at least two colors");
      const stops = args.colors.map(parseColor);
      const scoped = (args.keys?.length ?? 0) + (args.groups?.length ?? 0) > 0;
      const slots = scoped ? resolveKeys(args) : allSlots;
      aiEdit(() => gradient(stops, slots, args.direction ?? "horizontal"));
      return { painted_keys: slots.length };
    },

    apply_harmony: (args: { base_color: string; harmony: string; mode: "groups" | "gradient" }) => {
      const ids: Record<string, string> = {
        complementary: "complementary",
        analogous: "analogous",
        triadic: "triadic",
        split: "split",
        tetradic: "tetradic",
        mono: "mono",
      };
      const id = ids[args.harmony];
      if (!id) throw new Error(`Unknown harmony "${args.harmony}"`);
      harmonyId = id;
      paletteBase = parseColor(args.base_color);
      aiEdit(() => (args.mode === "gradient" ? applyGradient() : applyToGroups()));
      return { palette: palette.map(toHex), mode: args.mode };
    },

    list_themes: () => ({
      active: themeId,
      edited: dirty,
      user_themes: themeStore.themes.map((p) => ({ id: p.id, name: p.name })),
      presets: builtPresets.map((p) => ({ id: p.id, name: p.name, english_name: tEn(presetKey(p.id)) })),
    }),

    apply_theme: (args: { theme: string }) => {
      if (!profile) throw new Error("No active profile yet: retry in a moment");
      applyTheme(findThemeId(args.theme));
      return stateSummary();
    },

    save_theme: (args: { name?: string }) => {
      const name = args.name?.trim();
      if (name) {
        const id = createTheme(name);
        return { saved: "new theme", id, name };
      }
      if (!activeTheme) {
        throw new Error("The colors don't come from a user theme (presets are read-only): pass a name to save a new theme.");
      }
      if (!dirty) return { saved: false, reason: "the colors already match the theme" };
      save();
      return { saved: activeTheme.name };
    },

    revert_to_theme: () => {
      if (!dirty) return { reverted: false, reason: "the colors already match the theme" };
      revert();
      return stateSummary();
    },

    rename_theme: (args: { theme: string; name: string }) => {
      const id = findThemeId(args.theme, true);
      const name = args.name.trim();
      if (!name) throw new Error("The new name is empty");
      renameTheme(id, name);
      return { renamed: id, name };
    },

    delete_theme: (args: { theme: string }) => {
      const id = findThemeId(args.theme, true);
      deleteTheme(id);
      return { deleted: id };
    },

    list_profiles: () => ({
      active: activeProfileId,
      profiles: profiles.map(mcpProfile),
    }),

    switch_profile: async (args: { profile: string }) => {
      const target = findProfile(args.profile);
      await selectProfile(target.id);
      return { profile: mcpProfile(target), keys_written: keysPending || keysWriting, state: stateSummary() };
    },

    create_profile: async (args: { name: string; copy_of?: string }) => {
      const name = args.name.trim();
      if (!name) throw new Error("The name is empty");
      const from = args.copy_of ? findProfile(args.copy_of).id : null;
      const id = await createProfile(name, from);
      return { created: mcpProfile(profiles.find((p) => p.id === id)!), keys_written: keysPending || keysWriting };
    },

    rename_profile: (args: { profile: string; name: string }) => {
      const target = findProfile(args.profile);
      const name = args.name.trim();
      if (!name) throw new Error("The new name is empty");
      renameProfile(target.id, name);
      return { renamed: target.id, name };
    },

    export_profile: async (args: { profile: string }) => {
      const target = findProfile(args.profile);
      return { exported: target.name, path: await exportProfile(target.id, false) };
    },

    import_profile: async (args: { name: string; text: string }) => {
      const created = await importProfile(args.text);
      return { imported: mcpProfile(created), from: args.name, note: "Not active: use switch_profile to use it." };
    },

    delete_profile: async (args: { profile: string }) => {
      const target = findProfile(args.profile);
      if (profiles.length < 2) throw new Error("The last profile can't be deleted");
      await deleteProfile(target.id);
      return { deleted: target.id, active: mcpProfile(profile!) };
    },

    undo: () => {
      if (undoStack.length === 0 && groupTimer === undefined) throw new Error("Nothing to undo");
      undo();
      return stateSummary();
    },

    redo: () => {
      if (redoStack.length === 0) throw new Error("Nothing to redo");
      redo();
      return stateSummary();
    },

    get_device_info: async () => {
      if (!device) {
        return {
          connected: false,
          support,
          unknown_keyboard: unknownKeyboard,
          drawn_layout: layout ? { id: layout.id, name: layout.name, kind: layout.kind } : null,
          note:
            support === "unconfigured"
              ? "The app has no model for this keyboard: it sends it nothing. See list_models and set_preview_format."
              : "No keyboard connected",
        };
      }
      const rates = device.capabilities.report_rates;
      return {
        connected: true,
        support,
        model: device.info.model,
        driver: device.info.driver,
        layout: device.info.layout,
        per_key_rgb: device.capabilities.per_key_rgb,
        brightness_levels: device.capabilities.brightness_levels,
        per_key_upload_ms: device.capabilities.per_key_upload_ms,
        report_rates_hz: rates,
        report_rate_hz: rates.length ? await reportRate().catch(() => null) : null,
        effects: effects.map((e) => ({
          id: e.id,
          name: effectNameEn(e.id, e.name),
          ui_name: effectName(e.id, e.name),
          manufacturer_name: e.name,
          description: effectDescriptionEn(e.id),
          speed_levels: e.speed_levels,
          directions: e.directions,
          color: e.color,
          verified: e.verified,
        })),
      };
    },

    set_effect: (args: { effect: string; speed?: number; direction?: "left" | "right"; color?: string; random?: boolean }) => {
      const ref = args.effect.trim().toLowerCase();
      const fx = effects.find((e) =>
        [e.id, e.name, effectName(e.id, e.name), effectNameEn(e.id, e.name)].some((n) => n.toLowerCase() === ref),
      );
      if (!fx) {
        if (!device) throw new Error("Keyboard not connected: its effects are unknown");
        throw new Error(`Unknown effect "${args.effect}". Call get_device_info for the list.`);
      }
      if (args.direction && args.direction !== "left" && args.direction !== "right") {
        throw new Error('direction must be "left" or "right"');
      }
      const levels = fx.speed_levels || 4;
      const next: EffectState = { ...effectSetup, color: [...effectSetup.color] as Rgb, id: fx.id };
      if (args.speed !== undefined) next.speed = Math.round(Math.min(levels - 1, Math.max(0, args.speed)));
      if (args.direction) next.direction = args.direction;
      if (args.color) {
        next.color = parseColor(args.color);
        next.random = false;
      }
      if (args.random !== undefined) next.random = args.random;
      aiEdit(() => updateEffect(next));
      return {
        state: stateSummary(),
        note: fx.verified ? undefined : "Parameters of this effect are not verified yet: some may have no visible effect.",
      };
    },

    apply_image: async (args: {
      name: string;
      data: string;
      zoom?: number;
      center_x?: number;
      center_y?: number;
      vividness?: number;
    }) => {
      if (!layout) throw new Error("Keyboard layout not loaded yet");
      const bytes = Uint8Array.from(atob(args.data), (c) => c.charCodeAt(0));
      let bitmap: ImageBitmap;
      try {
        bitmap = await createImageBitmap(new Blob([bytes]));
      } catch {
        throw new Error(`"${args.name}" is not a picture the app can read (use PNG, JPEG, GIF, WebP or BMP)`);
      }
      const crop = clampCrop(bitmap.width, bitmap.height, layout.width / layout.height, {
        cx: args.center_x ?? DEFAULT_CROP.cx,
        cy: args.center_y ?? DEFAULT_CROP.cy,
        zoom: args.zoom ?? DEFAULT_CROP.zoom,
      });
      const map = sampleKeys(bitmap, layout, crop, Math.min(1, Math.max(0, args.vividness ?? 0.3)));
      aiEdit(() => applyImageColors(map, args.name));
      return { painted_keys: map.size, picture: { width: bitmap.width, height: bitmap.height }, crop };
    },

    set_lighting_mode: (args: { mode: Mode }) => {
      if (args.mode !== "per_key" && args.mode !== "effect") throw new Error('mode must be "per_key" or "effect"');
      aiEdit(() => setMode(args.mode));
      return stateSummary();
    },

    list_models: async () => {
      const all = await listLayouts();
      return {
        drawn: layout?.id,
        support,
        models: all.filter((l) => l.kind === "model"),
        formats: all.filter((l) => l.kind === "format"),
        note: "Models drive real keyboards; formats only draw a keyboard the app has no model for.",
      };
    },

    set_preview_format: async (args: { format: string }) => {
      if (support !== "unconfigured") throw new Error("A format can only be chosen while a keyboard without a model is connected");
      await choosePreviewFormat(args.format);
      return { drawn: layout?.id, name: layout?.name, keys: layout?.keys.length };
    },

    get_keyboard_layout: async () => {
      const k = await keyboardLayout();
      return {
        shape: k.format,
        shape_drawn: k.standard,
        layout: k.layout,
        layout_in_use: k.layout_id && { id: k.layout_id, name: k.layout_name },
        installed: k.installed,
        support: k.support,
        verified_as: layout?.verified_as ?? null,
      };
    },

    set_keyboard_layout: async (args: { shape?: string; layout?: string }) => {
      const shape = args.shape?.trim().toLowerCase();
      if (shape !== undefined && !["auto", "iso", "ansi"].includes(shape)) {
        throw new Error(`Unknown shape "${args.shape}": use auto, iso or ansi`);
      }
      let id = args.layout?.trim();
      if (id && id.toLowerCase() !== "auto") {
        const wanted = id.toLowerCase();
        const found = (await keyboardLayout()).installed.find((l) => l.id.toLowerCase() === wanted || l.name.toLowerCase() === wanted);
        if (!found) throw new Error(`Layout "${id}" is not installed in Windows. See installed in get_keyboard_layout.`);
        id = found.id;
      }
      const state = await setKeyboardLayout({ format: shape, layout: id?.toLowerCase() === "auto" ? "auto" : id });
      await keyboardLayoutChanged(state);
      return {
        shape: state.format,
        shape_drawn: state.standard,
        layout: state.layout,
        layout_in_use: state.layout_id && { id: state.layout_id, name: state.layout_name },
        support: state.support,
        note: "Only the drawing and text macros changed; nothing was sent to the keyboard.",
      };
    },

    set_report_rate: async (args: { hz: number }) => {
      if (!device) throw new Error("Keyboard not connected");
      const rates = device.capabilities.report_rates;
      if (!rates.includes(args.hz)) {
        throw new Error(
          rates.length
            ? `Unsupported report rate ${args.hz} Hz. Use one of: ${rates.join(", ")}.`
            : "This keyboard's report rate can't be changed",
        );
      }
      await setReportRate(args.hz);
      status = t("status.report_rate_set", { hz: args.hz });
      return { report_rate_hz: await reportRate() };
    },

    set_keyboard_look: async (args: { case?: string; keys?: string }) => {
      const pick = (value: string | undefined, presets: { id: string; hex: string }[]) => {
        if (value === undefined) return undefined;
        const preset = presets.find((p) => p.id === value.trim().toLowerCase());
        if (preset) return preset.hex;
        const rgb = fromHex(value);
        if (!rgb) throw new Error(`"${value}" is not a hex color or one of: ${presets.map((p) => p.id).join(", ")}`);
        return toHex(rgb);
      };
      const patch = { case: pick(args.case, CASE_PRESETS), keys: pick(args.keys, KEY_PRESETS) };
      await setLook(Object.fromEntries(Object.entries(patch).filter(([, v]) => v)));
      return { case: look.case, keys: look.keys };
    },

    set_appearance: async (args: { appearance: string }) => {
      if (!APPEARANCES.includes(args.appearance as Appearance)) {
        throw new Error('appearance must be "auto", "light" or "dark"');
      }
      await setAppearance(args.appearance as Appearance);
      return { setting: appearance.setting, in_use: appearance.resolved };
    },

    list_languages: async () => {
      // Picks up language files added while the app was running.
      await loadLanguage();
      return {
        setting: language.setting,
        active: language.active,
        available: language.languages,
        folder: language.folder,
      };
    },

    set_language: async (args: { language: string }) => {
      await loadLanguage();
      const code = args.language.trim().toLowerCase();
      if (code !== "auto" && !language.languages.some((l) => l.code === code)) {
        const codes = language.languages.map((l) => l.code).join(", ");
        throw new Error(`Unknown language "${args.language}". Use "auto" or one of: ${codes}.`);
      }
      await setLanguage(code);
      status = t("status.language_changed", { name: language.name });
      return { setting: language.setting, active: language.active };
    },

    get_key_functions: () => {
      const { names, config } = keysReady(true);
      return {
        writes_allowed: keysAllowed,
        support: keymapSupport,
        on_keyboard: !keysMismatch,
        differences_from_keyboard: keysOnBoard ? configDiff(config, keysOnBoard) : null,
        writing: keysWriting,
        changed_keys: (layout?.keys ?? [])
          .filter((k) => keyKinds[k.slot] !== "default")
          .map((k) => ({ key: k.id, label: k.label, function: mcpAction(config.keys[k.slot], names, config) })),
        macros: config.macros.map((m, i) => ({
          name: m.name,
          repeat: m.repeat,
          keys: macroUsers[i],
          steps: m.steps.map((st) => ({ key: names.id(st.code) ?? st.code, pressed: st.pressed, delay_ms: st.delay_ms })),
        })),
        media: MEDIA.map((m) => m.id),
      };
    },

    set_key_function: (args: {
      key: string;
      function: { type: string; key?: string; media?: string; macro?: string };
    }) => {
      const { names, config } = keysReady();
      const slots = resolveKeys({ keys: [args.key] });
      if (slots.length !== 1) throw new Error(`"${args.key}" is not a single key`);
      const slot = slots[0];
      if (keyDefaults[slot]?.kind === "fn") throw new Error("The Fn key can't be changed");
      const f = args.function;
      let action: KeyAction | null;
      switch (f.type) {
        case "default":
          action = null;
          break;
        case "disabled":
          action = { kind: "disabled" };
          break;
        case "key": {
          const code = f.key === undefined ? undefined : names.codeOfId(f.key);
          if (code === undefined) throw new Error(`Unknown key "${f.key}". Use a key id from list_keys.`);
          action = { kind: "key", code };
          break;
        }
        case "media": {
          const m = MEDIA.find((x) => x.id === f.media);
          if (!m) throw new Error(`Unknown media "${f.media}". Use one of: ${MEDIA.map((x) => x.id).join(", ")}.`);
          action = { kind: "media", code: m.code };
          break;
        }
        case "macro": {
          const index = config.macros.findIndex((m) => m.name.toLowerCase() === f.macro?.trim().toLowerCase());
          if (index < 0) throw new Error(`No macro named "${f.macro}". Create it with save_macro.`);
          action = { kind: "macro", index };
          break;
        }
        default:
          throw new Error('function.type must be "default", "key", "media", "macro" or "disabled"');
      }
      setKeyAction(slot, action);
      return { key: args.key, function: mcpAction(keyConfig!.keys[slot], names, keyConfig!) };
    },

    reset_key_functions: (args: { keys?: string[]; groups?: string[] }) => {
      keysReady();
      const scoped = (args.keys?.length ?? 0) + (args.groups?.length ?? 0) > 0;
      const slots = scoped ? resolveKeys(args) : allSlots;
      resetKeys(slots);
      return { reset_keys: slots.length };
    },

    save_macro: (args: {
      name: string;
      repeat?: number;
      steps?: { key: string; pressed: boolean; delay_ms?: number }[];
      text?: string;
    }) => {
      const { names, config } = keysReady();
      const support = keymapSupport!;
      const name = args.name.trim();
      if (!name) throw new Error("The macro name is empty");
      const delay = support.delay_step_ms * 5;
      let steps: Macro["steps"];
      if (args.text !== undefined) {
        const typed = names.textSteps(args.text, delay);
        if (!Array.isArray(typed)) {
          throw new Error(`"${typed.unsupported}" can't be typed on the ${layout?.typing?.name ?? "Windows"} keyboard layout: use steps for it`);
        }
        steps = typed;
      } else {
        steps = (args.steps ?? []).map((st) => {
          const code = names.codeOfId(st.key);
          if (code === undefined) throw new Error(`Unknown key "${st.key}" in steps. Use key ids from list_keys.`);
          const ms = Math.round((st.delay_ms ?? delay) / support.delay_step_ms) * support.delay_step_ms;
          return { code, pressed: st.pressed, delay_ms: Math.min(support.max_delay_ms, Math.max(0, ms)) };
        });
      }
      if (steps.length === 0) throw new Error("Give steps or text");
      if (steps.length > support.max_macro_steps) throw new Error(`At most ${support.max_macro_steps} steps`);
      steps[steps.length - 1].delay_ms = 0;
      const index = config.macros.findIndex((m) => m.name.toLowerCase() === name.toLowerCase());
      if (index < 0 && config.macros.length >= support.max_macros) {
        throw new Error(`The keyboard holds at most ${support.max_macros} macros`);
      }
      saveMacro(index < 0 ? null : index, { name, repeat: Math.min(99, Math.max(1, args.repeat ?? 1)), steps });
      return { saved: name, steps: steps.length, replaced: index >= 0 };
    },

    delete_macro: (args: { name: string }) => {
      const { config } = keysReady();
      const index = config.macros.findIndex((m) => m.name.toLowerCase() === args.name.trim().toLowerCase());
      if (index < 0) throw new Error(`No macro named "${args.name}"`);
      deleteMacro(index);
      return { deleted: args.name };
    },

    send_to_keyboard: async () => {
      if (!connected) throw new Error("Keyboard not connected");
      clearTimeout(sendTimer);
      await send();
      return { upload_ms: lastSendMs };
    },
  };

  function onShortcut(e: KeyboardEvent) {
    if (section === "lighting" && mode === "effect" && !e.repeat) {
      const id = keyIdFromCode(e.code);
      const key = id ? layout?.keys.find((k) => k.id === id) : undefined;
      if (key) previewPress(key.slot);
    }
    const typing = (e.target as HTMLElement).closest("input, textarea");
    if (!(e.ctrlKey || e.metaKey) || typing) return;
    const key = e.key.toLowerCase();
    const keys = section === "keys";
    if (key === "z" && !e.shiftKey) keys ? undoKeys() : undo();
    else if (key === "y" || (key === "z" && e.shiftKey)) keys ? redoKeys() : redo();
    else if (key === "s" && !keys) save();
    else return;
    e.preventDefault();
  }
</script>

<svelte:window onkeydown={onShortcut} />

<div
  class="app"
  style:--ambient={css(mode === "effect" ? effectSetup.color : target === "background" ? background : brush, 0.16)}
>
  <header>
    <div class="header-start">
      <div class="brand">
        <div class="logo" class:lit={glow !== null}>
          <span class="glow" style:background-color={glow ? css(glow[0]) : null}></span>
          <span class="glow second" style:background-color={glow ? css(glow[1]) : null}></span>
          <img src={logoUrl} alt="" />
        </div>
        <div>
          <h1>Lucent</h1>
          <span>
            {layout?.kind === "format"
              ? t("support.subtitle_format", { name: layout.name })
              : support === "unverified" && !layout?.verified_as
                ? layout?.name
                : t("app.subtitle", { standard: layout?.standard.toUpperCase() ?? "", layout: layout?.typing?.name ?? "" })}
          </span>
        </div>
      </div>
      {#if layout && profiles.length}
        <ProfileMenu
          {layout}
          {profiles}
          activeId={activeProfileId}
          details={profileDetails}
          onselect={selectProfile}
          oncreate={createProfile}
          onrename={renameProfile}
          ondelete={deleteProfile}
          onexport={(id) => exportProfile(id).catch((e) => (status = String(e)))}
          onimport={importProfileFile}
        />
      {/if}
    </div>
    <div class="sections segmented" style:--index={section === "lighting" ? 0 : 1}>
      <div class="indicator"></div>
      <button class:active={section === "lighting"} onclick={() => (section = "lighting")}>
        {t("app.section_lighting")}
      </button>
      <button class:active={section === "keys"} onclick={() => (section = "keys")}>{t("app.section_keys")}</button>
    </div>
    <div class="header-actions">
      <AiPanel />
      <button class="pill" class:ok={connected} onclick={reconnect} title={t("app.reconnect")}>
        <span class="dot"></span>
        {connected ? t("app.connected") : t("app.disconnected")}
      </button>
      <SettingsPanel onkeyboardlayout={keyboardLayoutChanged} />
    </div>
  </header>

  <main>
    {#if layout && section === "keys"}
      <MacroList
        macros={keyConfig?.macros ?? []}
        usedBy={macroUsers}
        max={keymapSupport?.max_macros ?? 0}
        onnew={() => keyNames && keymapSupport && (editingMacro = { index: null })}
        onedit={(index) => (editingMacro = { index })}
      />
    {:else if layout}
      <ThemeList
        {layout}
        presets={builtPresets}
        themes={themeStore.themes}
        activeId={themeId}
        {dirty}
        onselect={applyTheme}
        onnew={() => startNaming(t("themes.new_default"))}
        onrename={renameTheme}
        onduplicate={duplicateTheme}
        ondelete={deleteTheme}
      />
    {:else}
      <div></div>
    {/if}

    <section class="stage">
      <div class="toolbar">
        <div class="history">
          <button
            class="tool"
            title={t("app.undo")}
            disabled={section === "keys" ? keyUndo.length === 0 : undoStack.length === 0}
            onclick={() => (section === "keys" ? undoKeys() : undo())}
          >
            <svg viewBox="0 0 16 16"><path d="M6 4L3 7l3 3M3 7h6.5a3.5 3.5 0 010 7H8" /></svg>
          </button>
          <button
            class="tool"
            title={t("app.redo")}
            disabled={section === "keys" ? keyRedo.length === 0 : redoStack.length === 0}
            onclick={() => (section === "keys" ? redoKeys() : redo())}
          >
            <svg viewBox="0 0 16 16"><path d="M10 4l3 3-3 3M13 7H6.5a3.5 3.5 0 000 7H8" /></svg>
          </button>
        </div>

        <div class="title">
          {#if section === "keys"}
            <span class="profile-name">{t("keys.title")}</span>
            {#if keysWriting || keysPending}
              <span class="badge neutral" transition:fade={{ duration: 150 }}>{t("keys.writing")}</span>
            {:else if keysMismatch}
              <span class="badge" transition:fade={{ duration: 150 }}>{t("keys.not_on_keyboard")}</span>
            {/if}
          {:else if naming}
            <input
              class="name-input"
              bind:this={nameInput}
              bind:value={newName}
              placeholder={t("themes.name_placeholder")}
              onkeydown={(e) => {
                if (e.key === "Enter") commitNaming();
                if (e.key === "Escape") naming = false;
              }}
            />
          {:else}
            <span class="profile-name" title={activeName}>{activeName}</span>
            {#if dirty}
              <span class="badge" transition:fade={{ duration: 150 }}>{t("app.modified")}</span>
            {/if}
          {/if}
        </div>

        <div class="save">
          {#if section === "keys"}
            <!-- Key functions are saved and written by themselves. -->
          {:else if naming}
            <button class="btn" onclick={() => (naming = false)}>{t("common.cancel")}</button>
            <button class="btn primary" disabled={!newName.trim()} onclick={commitNaming}>{t("themes.save_new")}</button>
          {:else if dirty}
            <button class="btn" onclick={revert} transition:fade={{ duration: 150 }}>{t("common.revert")}</button>
            <button class="btn primary" onclick={save} title="Ctrl+S" transition:fade={{ duration: 150 }}>
              {activeTheme ? t("themes.save") : t("themes.save_as")}
            </button>
          {/if}
        </div>
      </div>

      {#if support === "unconfigured" || support === "unverified"}
        <SupportBanner
          {support}
          unknown={unknownKeyboard}
          modelName={device?.info.model ?? layout?.name ?? ""}
          shape={layout?.verified_as && layout.verified_as !== layout.standard
            ? { verified: layout.verified_as.toUpperCase(), standard: layout.standard.toUpperCase() }
            : null}
          {formats}
          formatId={layout?.kind === "format" ? layout.id : null}
          onformat={(id) => choosePreviewFormat(id).catch((e) => (status = String(e)))}
        />
      {/if}

      <div class="board-area">
        {#if layout}
          {#if section === "keys"}
            <Keyboard
              {layout}
              colors={keyColors}
              {selected}
              intensity={1}
              oneyedrop={() => {}}
              legends={keyLegends}
              marks={keyMarks}
            />
          {:else}
            <Keyboard
              {layout}
              colors={previewColors}
              {selected}
              intensity={intensity / 100}
              oneyedrop={eyedrop}
              onpress={mode === "effect" ? previewPress : undefined}
            />
          {/if}
        {/if}
        {#if mode === "effect" && section === "lighting"}
          <div class="running" transition:fade={{ duration: 200 }}>
            <span class="pulse"></span>
            {t("app.running", { effect: effectName(effectSetup.id, effectInfo?.name) })}
          </div>
        {/if}
      </div>
      {#if layout}
        <KeyboardLook />
      {/if}
      {#key status}
        <div class="toast" in:fly={{ y: 8, duration: 300 }}>{status}</div>
      {/key}
    </section>

    <aside class="inspector">
      {#if section === "keys"}
        {#if !keymapSupport || !keyConfig || !keyNames}
          <div class="group">
            <div class="caption">{t("keys.title")}</div>
            <p class="hint left">{connected ? keysError ?? t("keys.unsupported") : t("keys.connect")}</p>
          </div>
        {:else if inspectedKey}
          {@const slot = inspectedKey.slot}
          <div class="group" in:fade={{ duration: 150 }}>
            {#key slot}
              <KeyFunctionPanel
                label={inspectedKey.label || t("common.space")}
                action={keyConfig.keys[slot] ?? keyDefaults[slot]}
                factory={keyDefaults[slot]}
                names={keyNames}
                macros={keyConfig.macros}
                macrosSupported={keymapSupport.macros}
                onchange={(action) => setKeyAction(slot, action)}
                onnewmacro={() => (editingMacro = { index: null, forSlot: slot })}
                oneditmacro={(index) => (editingMacro = { index })}
              />
            {/key}
          </div>
        {:else}
          <div class="group" in:fade={{ duration: 150 }}>
            <div class="caption">{t("keys.title")}</div>
            <p class="hint left">{t("keys.select_hint")}</p>
            <div class="legend-list">
              <span><i class="sw key"></i>{t("keys.choice_key")}</span>
              <span><i class="sw media"></i>{t("keys.choice_media")}</span>
              <span><i class="sw macro"></i>{t("keys.choice_macro")}</span>
              <span><i class="sw disabled"></i>{t("keys.choice_disabled")}</span>
            </div>
          </div>
        {/if}

        {#if keymapSupport && keyConfig && keyNames}
          {#if !keysAllowed}
            <div class="group" transition:fade={{ duration: 150 }}>
              <div class="caption">{t("support.unverified_keys_title")}</div>
              <p class="hint left">{t("support.unverified_keys_hint")}</p>
              <button class="btn primary" onclick={() => ((keysUnlocked = true), scheduleKeysWrite())}>
                {t("support.unverified_keys_allow")}
              </button>
            </div>
          {/if}
          {#if keysMismatch && keysAllowed}
            <div class="group" transition:fade={{ duration: 150 }}>
              <div class="caption">{t("keys.mismatch_title")}</div>
              <p class="hint left">{t("keys.mismatch_hint")}</p>
              <div class="actions even">
                <button class="btn" onclick={useBoardKeys}>{t("keys.use_keyboard")}</button>
                <button class="btn primary" onclick={writeKeysNow}>{t("keys.send_mine")}</button>
              </div>
            </div>
          {/if}
          <div class="group">
            <div class="caption-row">
              <span class="caption">{t("keys.summary")}</span>
              <span class="count">{plural("keys.changed", changedKeys)}</span>
            </div>
            {#if selected.size > 1}
              <button class="btn" onclick={() => resetKeys([...selected])}>{t("keys.reset_selection")}</button>
            {/if}
            {#if confirmResetKeys}
              <div class="actions even">
                <button class="btn" onclick={() => (confirmResetKeys = false)}>{t("common.cancel")}</button>
                <button
                  class="btn danger"
                  onclick={() => {
                    confirmResetKeys = false;
                    resetKeys(allSlots);
                  }}>{t("keys.reset_all_confirm")}</button
                >
              </div>
            {:else}
              <button class="btn" disabled={changedKeys === 0} onclick={() => (confirmResetKeys = true)}>
                {t("keys.reset_all")}
              </button>
            {/if}
            <p class="hint left">{t("keys.stored_hint")}</p>
            {#if keysError}<p class="hint left error">{keysError}</p>{/if}
          </div>
        {/if}
      {:else}
      <div class="group">
        <div class="segmented" style:--index={mode === "per_key" ? 0 : 1}>
          <div class="indicator"></div>
          <button class:active={mode === "per_key"} onclick={() => setMode("per_key")}>{t("inspector.per_key")}</button>
          <button class:active={mode === "effect"} onclick={() => setMode("effect")}>{t("inspector.effects")}</button>
        </div>
      </div>

      {#if mode === "effect"}
        <div class="group" in:fade={{ duration: 150 }}>
          <EffectsPanel {effects} value={effectSetup} onchange={updateEffect} />
        </div>
      {:else}
        {#if inspectedKey}
          <div class="group" transition:fade={{ duration: 150 }}>
            <div class="caption">{t("inspector.key")}</div>
            <KeyInfo
              label={inspectedKey.label}
              color={colors[inspectedKey.slot]}
              custom={overrides.has(inspectedKey.slot)}
              {intensity}
              onuse={() => eyedrop(inspectedKey.slot)}
            />
          </div>
        {/if}

        <div class="group">
          <div class="segmented" style:--index={target === "keys" ? 0 : 1}>
            <div class="indicator"></div>
            <button class:active={target === "keys"} onclick={() => (target = "keys")}>{t("inspector.keys")}</button>
            <button class:active={target === "background"} onclick={() => (target = "background")}>
              <span class="bg-dot" style:background={toHex(background)}></span>
              {t("inspector.background")}
            </button>
          </div>
          <ColorPicker value={pickerValue} onpick={pick} />
          <p class="hint left">
            {target === "background"
              ? t("inspector.hint_background")
              : t("inspector.hint_keys")}
          </p>
        </div>

        <div class="group">
          <div class="caption">{t("inspector.harmonies")}</div>
          <div class="chips">
            {#each HARMONIES as h}
              <button class="chip" class:active={h.id === harmonyId} onclick={() => (harmonyId = h.id)}>
                {t(`harmony.${h.id}`)}
              </button>
            {/each}
          </div>
          <div class="palette">
            {#each palette as color, i (i)}
              <button
                class="tone"
                style:background={toHex(color)}
                style:--glow={css(color, 0.45)}
                title={toHex(color)}
                aria-label={t("inspector.use_color", { hex: toHex(color) })}
                onclick={() => pick(color, true)}
              ></button>
            {/each}
          </div>
          <div class="actions even">
            <button class="btn" onclick={applyToGroups}>{t("inspector.apply_groups")}</button>
            <button class="btn" onclick={applyGradient}>{t("inspector.gradient")}</button>
          </div>
        </div>

        <div class="group">
          <div class="caption">{t("inspector.image")}</div>
          <p class="hint left">{t("inspector.image_hint")}</p>
          <div class="actions even">
            <button class="btn" onclick={() => imageInput?.click()}>{t("inspector.load_image")}</button>
            <button class="btn" onclick={downloadTemplate} title={t("inspector.template_hint")}>
              {t("inspector.template")}
            </button>
          </div>
          <input
            bind:this={imageInput}
            type="file"
            accept="image/*"
            hidden
            onchange={(e) => {
              const input = e.target as HTMLInputElement;
              openImage(input.files?.[0]);
              input.value = "";
            }}
          />
        </div>

        <div class="group">
          <div class="caption-row">
            <span class="caption">{t("inspector.selection")}</span>
            <span class="count">{plural("inspector.count", selected.size)}</span>
          </div>
          <div class="chips">
            {#each GROUPS as group}
              <button class="chip" onclick={() => layout && select(groupSlots(layout, group.id))}>
                {t(`group.${group.id}`)}
              </button>
            {/each}
            <button class="chip" onclick={() => select(allSlots)}>{t("inspector.all")}</button>
            <button class="chip" onclick={() => selected.clear()}>{t("inspector.none")}</button>
          </div>
          <button class="btn primary" disabled={selected.size === 0} onclick={() => paint(brush)}>
            {t("inspector.paint_selection")}
          </button>
          <div class="actions even">
            <button class="btn" disabled={selected.size === 0} onclick={resetSelection}>{t("common.revert")}</button>
            <button class="btn" disabled={selected.size === 0} onclick={() => paint([0, 0, 0])}>{t("inspector.turn_off")}</button>
          </div>
        </div>
      {/if}

      <div class="group">
        <div class="caption-row">
          <span class="caption">{t("inspector.intensity")}</span>
          <span class="count">{intensity}%</span>
        </div>
        <input
          type="range"
          class="slider"
          min="0"
          max="100"
          step={mode === "effect" ? 25 : 1}
          value={intensity}
          oninput={(e) => setIntensity(Number((e.target as HTMLInputElement).value))}
          style:--fill="{intensity}%"
        />
      </div>

      <div class="group">
        <label class="toggle-row">
          <span>{t("inspector.auto_send")}</span>
          <input type="checkbox" class="toggle" bind:checked={autoSend} />
        </label>
        <button class="btn wide" disabled={!connected || sending} onclick={send}>
          {sending ? t("inspector.sending") : t("inspector.send_now")}
        </button>
        {#if lastSendMs !== null}
          <div class="hint">{t("inspector.last_send", { ms: lastSendMs })}</div>
        {/if}
      </div>
      {/if}
    </aside>
  </main>

  {#if editingMacro && keyNames && keymapSupport && keyConfig}
    {@const index = editingMacro.index}
    {@const forSlot = editingMacro.forSlot}
    <MacroEditor
      macro={index === null ? null : keyConfig.macros[index]}
      names={keyNames}
      support={keymapSupport}
      usedBy={index === null ? [] : macroUsers[index]}
      onsave={(m) => {
        saveMacro(index, m, forSlot);
        editingMacro = null;
      }}
      ondelete={index === null
        ? undefined
        : () => {
            deleteMacro(index);
            editingMacro = null;
          }}
      oncancel={() => (editingMacro = null)}
    />
  {/if}

  {#if pendingImage && layout}
    <ImageDialog
      {layout}
      image={pendingImage.bitmap}
      name={pendingImage.name}
      onapply={(map) => {
        applyImageColors(map, pendingImage!.name);
        pendingImage = null;
      }}
      oncancel={() => (pendingImage = null)}
    />
  {/if}

</div>

<style>
  .app {
    height: 100%;
    display: flex;
    flex-direction: column;
    background:
      radial-gradient(ellipse 60% 55% at 50% 55%, var(--ambient), transparent 70%),
      var(--bg);
    transition: background 0.6s var(--ease);
  }

  header {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    column-gap: 16px;
    align-items: center;
    padding: 18px 24px 6px;
  }

  .sections {
    width: 260px;
  }

  .header-start {
    display: flex;
    align-items: center;
    gap: 18px;
    min-width: 0;
  }

  .header-actions {
    justify-self: end;
  }

  .badge.neutral {
    background: var(--fill);
    color: var(--text-2);
  }

  .legend-list {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
    color: var(--text-2);
    font-size: 12px;
  }

  .legend-list span {
    display: flex;
    align-items: center;
    gap: 7px;
  }

  .sw {
    width: 9px;
    height: 9px;
    border-radius: 50%;
  }

  .sw.key {
    background: var(--blue-text);
    box-shadow: 0 0 6px var(--blue-text);
  }

  .sw.media {
    background: var(--orange-text);
    box-shadow: 0 0 6px var(--orange-text);
  }

  .sw.macro {
    background: var(--purple-text);
    box-shadow: 0 0 6px var(--purple-text);
  }

  .sw.disabled {
    background: var(--fill-strong);
  }

  .btn.danger {
    background: rgba(255, 69, 58, 0.85);
    color: white;
  }

  .hint.error {
    color: var(--error-text);
  }

  .brand {
    flex: none;
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .logo {
    position: relative;
    width: 38px;
    height: 38px;
  }

  .logo img {
    position: relative;
    display: block;
    width: 100%;
    height: 100%;
  }

  /* Two soft lights behind the logo, in the keyboard's two main colors (lib/glow.ts). */
  .glow {
    position: absolute;
    top: 5px;
    left: -3px;
    width: 30px;
    height: 30px;
    border-radius: 50%;
    filter: blur(11px);
    opacity: 0;
    transition:
      background-color 0.5s ease,
      opacity 0.5s ease;
  }

  .glow.second {
    left: auto;
    right: -3px;
  }

  .logo.lit .glow {
    opacity: var(--logo-glow);
  }

  @media (prefers-reduced-motion: reduce) {
    .glow {
      transition: none;
    }
  }

  h1 {
    margin: 0;
    font-size: 17px;
    font-weight: 700;
    letter-spacing: -0.01em;
  }

  .brand span {
    color: var(--text-2);
    font-size: 12px;
    white-space: nowrap;
  }

  .header-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .pill {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 30px;
    padding: 0 14px;
    border-radius: 15px;
    background: var(--surface);
    border: 0.5px solid var(--hairline);
    backdrop-filter: blur(20px);
    color: var(--text-2);
    font-weight: 500;
    transition: background 0.2s var(--ease);
  }

  .pill:hover {
    background: var(--surface-strong);
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--red);
    box-shadow: 0 0 8px var(--red);
  }

  .pill.ok .dot {
    background: var(--green);
    box-shadow: 0 0 8px var(--green);
    animation: breathe 2.4s ease-in-out infinite;
  }

  @keyframes breathe {
    50% {
      opacity: 0.45;
    }
  }

  main {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 236px 1fr 330px;
    gap: 16px;
    padding: 8px 16px 16px;
  }

  .stage {
    position: relative;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .toolbar {
    display: grid;
    /* The name in the middle shrinks (with an ellipsis) before the buttons on the sides do. */
    grid-template-columns: 1fr minmax(0, max-content) 1fr;
    align-items: center;
    gap: 12px;
    height: 44px;
    padding: 0 6px;
  }

  .history {
    display: flex;
    gap: 4px;
  }

  .tool {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: 9px;
    background: var(--surface);
    border: 0.5px solid var(--hairline);
    color: var(--text);
    transition:
      background 0.2s var(--ease),
      opacity 0.2s var(--ease);
  }

  .tool:hover {
    background: var(--surface-strong);
  }

  .tool:disabled {
    opacity: 0.3;
    pointer-events: none;
  }

  .tool svg {
    width: 16px;
    height: 16px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.5;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .title {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }

  .profile-name {
    font-size: 15px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .badge {
    padding: 2px 7px;
    border-radius: 6px;
    background: rgba(255, 159, 10, 0.18);
    color: var(--orange-text);
    font-size: 11px;
    font-weight: 600;
  }

  .name-input {
    width: 240px;
    height: 32px;
    padding: 0 10px;
    border-radius: 9px;
    border: 0.5px solid var(--accent);
    background: var(--fill);
    box-shadow: 0 0 0 3px rgba(10, 132, 255, 0.25);
    outline: none;
    text-align: center;
  }

  .save {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  .board-area {
    position: relative;
    flex: 1;
    min-height: 0;
  }

  .toast {
    position: absolute;
    left: 50%;
    bottom: 8px;
    transform: translateX(-50%);
    padding: 7px 14px;
    border-radius: 10px;
    background: var(--surface);
    border: 0.5px solid var(--hairline);
    backdrop-filter: blur(20px);
    color: var(--text-2);
    font-size: 12px;
    white-space: nowrap;
  }

  .running {
    position: absolute;
    top: 14px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 8px;
    height: 28px;
    padding: 0 12px;
    border-radius: 14px;
    background: var(--surface);
    border: 0.5px solid var(--hairline);
    backdrop-filter: blur(20px);
    color: var(--text-2);
    font-size: 12px;
    white-space: nowrap;
  }

  .pulse {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--green);
    box-shadow: 0 0 8px var(--green);
    animation: pulse 1.6s ease-in-out infinite;
  }

  @keyframes pulse {
    50% {
      opacity: 0.35;
    }
  }

  .inspector {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 16px;
    overflow-y: auto;
    border-radius: 20px;
    background: var(--surface);
    border: 0.5px solid var(--hairline);
    backdrop-filter: blur(40px) saturate(1.6);
    box-shadow: 0 20px 50px -20px var(--shadow-strong);
  }

  .group {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding-bottom: 14px;
    border-bottom: 0.5px solid var(--hairline);
  }

  .group:last-child {
    border-bottom: none;
    padding-bottom: 0;
  }

  .palette {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    gap: 6px;
    height: 44px;
  }

  .tone {
    border-radius: 10px;
    box-shadow:
      inset 0 0 0 0.5px var(--fill-strong),
      0 6px 16px -6px var(--glow);
    transition: transform 0.25s var(--ease);
  }

  .tone:hover {
    transform: translateY(-2px) scale(1.04);
  }

  .actions.even {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }

  .wide {
    width: 100%;
  }

  .bg-dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    box-shadow: inset 0 0 0 0.5px rgba(255, 255, 255, 0.4);
  }
</style>
