import { invoke } from "@tauri-apps/api/core";

export type Rgb = [number, number, number];

export interface KeyDef {
  slot: number;
  /** Stable name used by MCP tools, e.g. "ShiftL", "Num7". */
  id: string;
  label: string;
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface Layout {
  /** Model or format id, e.g. "gmmk_v1_fullsize_iso_it", "format_tkl_ansi". */
  id: string;
  name: string;
  /** "model": a real keyboard; "format": a drawing that previews a keyboard without a model. */
  kind: "model" | "format";
  /** The shape drawn. */
  standard: "iso" | "ansi";
  /** The shape the model was verified in, when it was: another one makes it "unverified". */
  verified_as: "iso" | "ansi" | null;
  width: number;
  height: number;
  keys: KeyDef[];
  led_count: number;
  /** The Windows keyboard layout the legends come from; null when Windows can't tell. */
  typing: OsLayout | null;
}

/** What a key writes with one combination of modifiers; a dead key only accents the next letter. */
export interface Typed {
  text: string;
  dead: boolean;
}

/** A Windows keyboard layout: what each key (by id) writes alone, with Shift, AltGr, or both. */
export interface OsLayout {
  /** Windows' layout id (KLID), e.g. "00000410" for Italian. */
  id: string;
  name: string;
  keys: Record<string, { base: Typed | null; shift: Typed | null; altgr: Typed | null; shift_altgr: Typed | null }>;
}

/** A keyboard on USB that the app has no model for: nothing is sent to it. */
export interface UnknownKeyboard {
  vid: string;
  pid: string;
  vendor: string;
  manufacturer: string;
  product: string;
}

/**
 * "verified": everything checked on this model. "unverified": a model from the `models` folder,
 * key writes wait for the user's OK. "unconfigured": a keyboard without a model, preview only.
 */
export type Support = "verified" | "unverified" | "unconfigured" | "none";

export interface Status {
  connected: boolean;
  message: string;
  support: Support;
  unknown: UnknownKeyboard | null;
}

export interface LayoutSummary {
  id: string;
  name: string;
  kind: "model" | "format";
  format: string;
  standard: string;
  keys: number;
  verified: boolean;
}

export type Direction = "left" | "right";

/** A hardware effect as the user set it up (see EffectSettings in crates/lucent-core/src/driver.rs). */
export interface EffectState {
  /** EffectInfo.id */
  id: string;
  speed: number;
  direction: Direction;
  color: Rgb;
  /** Colors picked by the keyboard; `color` is kept for when it is turned off. */
  random: boolean;
}

/** Lighting setup: per-key colors painted over a background, or a hardware effect. */
export interface Lighting {
  /** Missing in profiles saved before effects existed: per-key. */
  mode?: "per_key" | "effect";
  background: Rgb;
  /** Per-key colors by slot (JSON object keys are strings). */
  overrides: Record<string, Rgb>;
  /** 0..100. Effects use the hardware steps, so it moves in steps of 25 there. */
  intensity: number;
  effect?: EffectState;
}

/** A saved lighting setup ("My themes"). */
export interface Theme extends Lighting {
  id: string;
  name: string;
}

export interface ThemeStore {
  themes: Theme[];
}

/** What the keyboard does as a whole: lighting, key functions and macros. */
export interface UserProfile {
  id: string;
  name: string;
  /** Unix ms; profiles are listed oldest first. */
  created: number;
  lighting: Lighting;
  /** Theme (user theme or preset id) the lighting was last taken from. */
  theme: string | null;
  /** `null` until the keyboard is first read: then it takes the keyboard's. */
  keys: import("./keys").KeyConfig | null;
}

export interface EffectInfo {
  id: string;
  /** Manufacturer's name. */
  name: string;
  /** 0 when the effect has no speed. */
  speed_levels: number;
  directions: Direction[];
  color: "none" | "single" | "single_or_random";
  /** False while the parameters are a best guess. */
  verified: boolean;
}

export interface DeviceDescription {
  info: { model: string; driver: string; layout: string };
  capabilities: {
    per_key_rgb: boolean;
    led_count: number;
    brightness_levels: number;
    effects: EffectInfo[];
    per_key_upload_ms: number;
    /** USB report rates in Hz, slowest first; empty when they can't be changed. */
    report_rates: number[];
    /** How key functions and macros are stored; null when they can't be changed. */
    keymap: import("./keys").KeymapSupport | null;
  };
}

export interface McpStatus {
  enabled: boolean;
  running: boolean;
  url: string;
  error: string | null;
}

export const connect = () => invoke<Status>("connect");
/** Whether the keyboard came or went, from the USB device list only (nothing is sent to it). */
export const checkKeyboard = () => invoke<"lost" | "available" | "same">("check_keyboard");
export const mcpStatus = () => invoke<McpStatus>("mcp_status");
export const mcpSetEnabled = (enabled: boolean) => invoke<McpStatus>("mcp_set_enabled", { enabled });
export const mcpReply = (id: number, ok: unknown, error: string | null) =>
  invoke<void>("mcp_reply", { id, ok, error });
export const loadThemes = () => invoke<ThemeStore>("load_themes");
export const saveThemes = (store: ThemeStore) => invoke<void>("save_themes", { store });
export const loadUserProfiles = () =>
  invoke<{ profiles: UserProfile[]; active: string | null }>("load_user_profiles");
export const saveUserProfile = (profile: UserProfile) => invoke<void>("save_user_profile", { profile });
export const deleteUserProfile = (id: string) => invoke<void>("delete_user_profile", { id });
export const setActiveProfile = (id: string) => invoke<void>("set_active_profile", { id });
export const getLayout = () => invoke<Layout>("layout");

/** The keyboard's shape and the Windows layout its legends and text macros follow. */
export interface KeyboardLayoutState {
  /** "auto", "iso" or "ansi". */
  format: string;
  /** "auto" or a Windows layout id (KLID). */
  layout: string;
  /** The Windows layout in use and its id. */
  layout_name: string | null;
  layout_id: string | null;
  installed: { id: string; name: string }[];
  /** The shape drawn and the support level that follows from it. */
  standard: "iso" | "ansi";
  support: Support;
  /** The shape "auto" would draw. */
  auto_standard: "iso" | "ansi";
}

export const keyboardLayout = () => invoke<KeyboardLayoutState>("keyboard_layout");
/** Nothing is sent to the keyboard: only the drawing and text macros change. */
export const setKeyboardLayout = (change: { format?: string; layout?: string }) =>
  invoke<KeyboardLayoutState>("set_keyboard_layout", change);
export const listLayouts = () => invoke<LayoutSummary[]>("list_layouts");
/** Format that draws a keyboard without a model (remembered). */
export const setPreviewFormat = (id: string) => invoke<Layout>("set_preview_format", { id });
/** Uploads every LED color; resolves with the upload time in ms. */
export const sendColors = (colors: Rgb[], brightness: number) =>
  invoke<number>("send_colors", { colors, brightness });

export interface LogStatus {
  verbose: boolean;
  /** Current log file. */
  path: string;
}

export const deviceInfo = () => invoke<DeviceDescription | null>("device_info");
export const setEffect = (settings: {
  effect: string;
  brightness: number;
  speed: number;
  direction: Direction | null;
  color: Rgb | null;
}) => invoke<void>("set_effect", { settings });

export const logStatus = () => invoke<LogStatus>("log_status");
export const setVerboseLog = (enabled: boolean) => invoke<LogStatus>("set_verbose_log", { enabled });
export const openLogDir = () => invoke<void>("open_log_dir");
/** Saves a file into the data folder's exports and shows it unless `reveal` is false; resolves with its path. */
export const saveExport = (name: string, data: Uint8Array, reveal = true) =>
  invoke<string>("save_export", { name, data: Array.from(data), reveal });

export interface StartupOptions {
  /** Starts with Windows, in the tray. */
  autostart: boolean;
  /** Closing the window keeps the app in the tray. */
  close_to_tray: boolean;
  autostart_supported: boolean;
}

export const startupOptions = () => invoke<StartupOptions>("startup_options");
/** Omitted options stay as they are. */
export const setStartupOptions = (options: { autostart?: boolean; closeToTray?: boolean }) =>
  invoke<StartupOptions>("set_startup_options", options);

export interface TrayItem {
  id: string;
  name: string;
}

/** Rebuilds the tray menu (labels in the interface language). */
export const setTrayMenu = (menu: {
  show: string;
  quit: string;
  profiles: TrayItem[];
  active_profile: string | null;
  themes_title: string;
  themes: TrayItem[];
  presets: TrayItem[];
  active_theme: string | null;
}) => invoke<void>("set_tray_menu", { menu });

/** Current USB report rate in Hz. */
export const reportRate = () => invoke<number>("report_rate");
/** Writes the USB report rate into the keyboard, which keeps it. */
export const setReportRate = (hz: number) => invoke<void>("set_report_rate", { hz });
