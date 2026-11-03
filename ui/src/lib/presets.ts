import type { Layout, Lighting, Rgb } from "./api";
import { groupSlots, slotsOf } from "./groups";
import { samplePalette } from "./palette";

export interface Preset {
  /** "preset:<name>"; the display name is `preset.<name>` in the language files. */
  id: string;
  build: (layout: Layout) => Lighting;
}

function solid(background: Rgb, layers: [slots: number[], color: Rgb][] = []): Lighting {
  const overrides: Record<string, Rgb> = {};
  for (const [slots, color] of layers) {
    for (const slot of slots) overrides[slot] = color;
  }
  return { background, overrides, intensity: 100 };
}

/** Left-to-right gradient through `stops`, on every key. */
function gradient(layout: Layout, stops: Rgb[]): Lighting {
  const overrides: Record<string, Rgb> = {};
  for (const key of layout.keys) {
    overrides[key.slot] = samplePalette(stops, (key.x + key.w / 2) / layout.width);
  }
  return { background: stops[0], overrides, intensity: 100 };
}

export const PRESETS: Preset[] = [
  { id: "preset:neve", build: () => solid([255, 255, 255]) },
  {
    id: "preset:synthwave",
    build: (l) => gradient(l, [[255, 40, 180], [150, 60, 255], [40, 220, 255]]),
  },
  {
    id: "preset:tramonto",
    build: (l) => gradient(l, [[255, 140, 0], [255, 60, 90], [150, 40, 210]]),
  },
  {
    id: "preset:aurora",
    build: (l) => gradient(l, [[40, 255, 140], [0, 200, 210], [120, 80, 255]]),
  },
  {
    id: "preset:oceano",
    build: (l) => gradient(l, [[0, 40, 255], [0, 140, 255], [0, 255, 220]]),
  },
  {
    id: "preset:lava",
    build: (l) => gradient(l, [[255, 0, 0], [255, 80, 0], [255, 190, 0]]),
  },
  {
    id: "preset:ghiacciaio",
    build: (l) =>
      solid([90, 190, 255], [
        [groupSlots(l, "letters"), [255, 255, 255]],
        [groupSlots(l, "modifiers"), [20, 80, 255]],
      ]),
  },
  {
    id: "preset:matrix",
    build: (l) =>
      solid([0, 70, 12], [
        [[...groupSlots(l, "letters"), ...groupSlots(l, "numbers")], [40, 255, 80]],
      ]),
  },
  {
    id: "preset:gaming",
    build: (l) =>
      solid([40, 10, 90], [
        [[...groupSlots(l, "wasd"), ...groupSlots(l, "arrows")], [255, 30, 30]],
        [slotsOf(l, ["Esc"]), [255, 120, 0]],
        [slotsOf(l, ["Space"]), [0, 220, 255]],
      ]),
  },
  {
    id: "preset:sakura",
    build: (l) =>
      solid([255, 120, 180], [
        [groupSlots(l, "letters"), [255, 235, 245]],
        [groupSlots(l, "modifiers"), [200, 60, 140]],
      ]),
  },
];

/** Key of a preset's name in the language files. */
export const presetKey = (id: string) => `preset.${id.replace(/^preset:/, "")}`;
