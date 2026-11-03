import type { EffectState } from "./api";
import { has, t, tEn } from "./i18n.svelte";

// Names and descriptions live in the language files as `effect.<id>.name` and
// `effect.<id>.description`, written from watching the real effect. Unknown ids fall back
// to the manufacturer's name.

export const effectName = (id: string, fallback = id) => (has(`effect.${id}.name`) ? t(`effect.${id}.name`) : fallback);
export const effectDescription = (id: string) =>
  has(`effect.${id}.description`) ? t(`effect.${id}.description`) : "";

/** English name, for MCP answers. */
export const effectNameEn = (id: string, fallback = id) =>
  has(`effect.${id}.name`) ? tEn(`effect.${id}.name`) : fallback;
export const effectDescriptionEn = (id: string) =>
  has(`effect.${id}.description`) ? tEn(`effect.${id}.description`) : "";

export const DEFAULT_EFFECT: EffectState = {
  id: "wave",
  speed: 2,
  direction: "left",
  color: [255, 0, 0],
  random: false,
};
