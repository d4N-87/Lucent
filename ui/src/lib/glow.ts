import type { Rgb } from "./api";
import { hsvToRgb, rgbToHsv } from "./color";

/** Hue groups the keys are sorted into; whites and grays go into one more. */
const HUES = 12;
/** Darker keys count as off. */
const MIN_VALUE = 0.08;
/** Less saturated keys count as white. */
const MIN_SATURATION = 0.25;

/**
 * The keyboard's two main colors as it is lit, for the glow behind the logo. Keys are grouped by
 * hue and the two biggest groups win, at full brightness so a dimmed keyboard still glows; with a
 * single color both are the same. `null` when every key is off.
 */
export function glowColors(keys: Rgb[]): [Rgb, Rgb] | null {
  const groups = Array.from({ length: HUES + 1 }, () => ({ n: 0, x: 0, y: 0, s: 0 }));
  for (const rgb of keys) {
    const { h, s, v } = rgbToHsv(rgb);
    if (v < MIN_VALUE) continue;
    const g = groups[s < MIN_SATURATION ? HUES : Math.round(h / (360 / HUES)) % HUES];
    // Hues are averaged as angles, so 350° and 10° make red, not cyan.
    g.n++;
    g.x += Math.cos((h * Math.PI) / 180);
    g.y += Math.sin((h * Math.PI) / 180);
    g.s += s;
  }
  const color = (i: number): Rgb => {
    const g = groups[i];
    if (i === HUES) return [255, 255, 255];
    const h = ((Math.atan2(g.y, g.x) * 180) / Math.PI + 360) % 360;
    return hsvToRgb({ h, s: g.s / g.n, v: 1 });
  };
  const ranked = groups
    .map((g, i) => ({ i, n: g.n }))
    .filter((g) => g.n > 0)
    .sort((a, b) => b.n - a.n);
  if (ranked.length === 0) return null;
  return [color(ranked[0].i), color((ranked[1] ?? ranked[0]).i)];
}
