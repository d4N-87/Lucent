import type { Rgb } from "./api";
import { hsvToRgb, rgbToHsv } from "./color";

/** A color derived from the base: hue offset in degrees, saturation and value deltas. */
type Step = [dh: number, ds?: number, dv?: number];

export interface Harmony {
  id: string;
  /** Its name is `harmony.<id>` in the language files. */
  steps: Step[];
}

export const HARMONIES: Harmony[] = [
  { id: "complementary", steps: [[0], [0, -0.35], [180], [180, -0.35], [0, 0, -0.35]] },
  { id: "analogous", steps: [[-60], [-30], [0], [30], [60]] },
  { id: "triadic", steps: [[0], [120], [240], [0, -0.4], [120, -0.4]] },
  { id: "split", steps: [[0], [150], [210], [0, -0.4], [180, -0.3, -0.2]] },
  { id: "tetradic", steps: [[0], [90], [180], [270], [0, -0.4]] },
  { id: "mono", steps: [[0], [0, -0.25], [0, -0.5], [0, 0, -0.3], [0, -0.25, -0.5]] },
];

const clamp = (x: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, x));

export function buildPalette(base: Rgb, harmony: Harmony): Rgb[] {
  const { h, s, v } = rgbToHsv(base);
  return harmony.steps.map(([dh, ds = 0, dv = 0]) =>
    hsvToRgb({
      h: (h + dh + 360) % 360,
      // Keep some saturation and light: very dark colors are barely visible on LEDs.
      s: clamp(s + ds, 0.12, 1),
      v: clamp(v + dv, 0.25, 1),
    }),
  );
}

/** Color at position t (0..1) along evenly spaced palette stops. */
export function samplePalette(palette: Rgb[], t: number): Rgb {
  const pos = clamp(t, 0, 1) * (palette.length - 1);
  const i = Math.min(Math.floor(pos), palette.length - 2);
  const f = pos - i;
  const [a, b] = [palette[i], palette[i + 1]];
  return a.map((c, k) => Math.round(c + (b[k] - c) * f)) as Rgb;
}
