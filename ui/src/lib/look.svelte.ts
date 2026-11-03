import { invoke } from "@tauri-apps/api/core";
import { fromHex } from "./color";

/** Case and keycap colors of the keyboard preview, to match the real keyboard. An app setting. */
export interface KeyboardLook {
  case: string;
  keys: string;
}

/** Ready-made colors; names are `look.<id>` in the language files. */
export const CASE_PRESETS = [
  { id: "black", hex: "#1c1c22" },
  { id: "white", hex: "#e8e8ec" },
  { id: "gray", hex: "#3c3c43" },
];

export const KEY_PRESETS = [
  { id: "black", hex: "#2a2a31" },
  { id: "white", hex: "#f2f2f4" },
  { id: "gray", hex: "#6e6e75" },
];

export const DEFAULT_LOOK: KeyboardLook = { case: CASE_PRESETS[0].hex, keys: KEY_PRESETS[0].hex };

let current = $state<KeyboardLook>({ ...DEFAULT_LOOK });

export const look = {
  get case() {
    return current.case;
  },
  get keys() {
    return current.keys;
  },
};

/** Light keycaps need darker text and an outline on the legends. */
export function isLight(hex: string): boolean {
  const rgb = fromHex(hex);
  if (!rgb) return false;
  const [r, g, b] = rgb.map((c) => c / 255);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b > 0.5;
}

export async function loadLook() {
  const saved = await invoke<Partial<KeyboardLook> | null>("keyboard_look");
  current = { ...DEFAULT_LOOK, ...(saved ?? {}) };
}

export async function setLook(patch: Partial<KeyboardLook>) {
  current = { ...current, ...patch };
  await invoke("set_keyboard_look", { look: $state.snapshot(current) });
}
