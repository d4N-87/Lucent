import type { Layout } from "./api";

/**
 * Quick-selection groups, as key ids, so they work on any layout (keys a layout lacks are skipped).
 * Names are `group.<id>` in the language files.
 */
export const GROUPS: { id: string; keys: string[] }[] = [
  { id: "wasd", keys: ["W", "A", "S", "D"] },
  { id: "letters", keys: [..."QWERTYUIOPASDFGHJKLZXCVBNM"] },
  { id: "numbers", keys: [..."1234567890"] },
  { id: "fkeys", keys: Array.from({ length: 12 }, (_, i) => `F${i + 1}`) },
  { id: "arrows", keys: ["Left", "Down", "Up", "Right"] },
  {
    id: "modifiers",
    keys: ["Esc", "Tab", "CapsLock", "ShiftL", "ShiftR", "Enter", "CtrlL", "Win", "Alt", "AltGr", "Fn", "Menu", "CtrlR", "Backspace"],
  },
  { id: "navigation", keys: ["PrintScreen", "ScrollLock", "Pause", "Insert", "Home", "PageUp", "Delete", "End", "PageDown"] },
  {
    id: "numpad",
    keys: [
      "NumLock", "NumSlash", "NumStar", "NumMinus", "Num7", "Num8", "Num9", "NumPlus", "Num4", "Num5", "Num6",
      "Num1", "Num2", "Num3", "NumEnter", "Num0", "NumPeriod",
    ],
  },
];

/** Slots of the layout's keys with these ids. */
export const slotsOf = (layout: Layout, ids: string[]) =>
  layout.keys.filter((k) => ids.includes(k.id)).map((k) => k.slot);

export const groupSlots = (layout: Layout, id: string) => slotsOf(layout, GROUPS.find((g) => g.id === id)?.keys ?? []);

/** Quick colors, fully saturated: screen palettes (e.g. Apple's #FF3B30 red) look washed out on LEDs. */
export const SWATCHES: [number, number, number][] = [
  [255, 0, 0],
  [255, 64, 0],
  [255, 160, 0],
  [0, 255, 0],
  [0, 255, 255],
  [0, 128, 255],
  [0, 0, 255],
  [128, 0, 255],
  [255, 0, 255],
  [255, 255, 255],
];
