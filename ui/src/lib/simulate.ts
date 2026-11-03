// On-screen imitation of the keyboard's hardware effects. The real animations live in the
// keyboard firmware and are not readable, so these are recreated from watching the keyboard
// (descriptions in effects.ts) and tuned by eye.

import type { EffectState, Layout, Rgb } from "./api";
import { hsvToRgb } from "./color";

interface SimKey {
  slot: number;
  /** Key center, in key units. */
  x: number;
  y: number;
  /** Center normalized to 0..1 across the keyboard. */
  nx: number;
  ny: number;
}

interface Press {
  key: SimKey;
  /** Seconds, same clock as `frame`. */
  t: number;
  hue: number;
}

/** Speed level 0..3 (slowest to fastest) as a time multiplier. */
const SPEED = [0.55, 0.85, 1.3, 2];
/** Presses older than this no longer show. */
const PRESS_LIFE = 4;

const TAU = Math.PI * 2;
const clamp01 = (v: number) => Math.min(1, Math.max(0, v));
const fract = (v: number) => v - Math.floor(v);
const gauss = (d: number, width: number) => Math.exp(-((d / width) ** 2));

/** Stable pseudo-random 0..1 for an integer seed. */
function hash(n: number): number {
  const s = Math.sin(n * 127.1 + 311.7) * 43758.5453;
  return s - Math.floor(s);
}

const scale = (c: Rgb, b: number): Rgb => [c[0] * b, c[1] * b, c[2] * b].map((v) => Math.round(v)) as Rgb;
const hue = (h: number, b = 1): Rgb => hsvToRgb({ h: ((h % 360) + 360) % 360, s: 1, v: clamp01(b) });

export class EffectSimulator {
  private keys: SimKey[];
  private width: number;
  private height: number;
  private ledCount: number;
  private presses: Press[] = [];
  private pressCount = 0;
  /** Hue Surmount moves to with each key press, and the hue it shows while easing there. */
  private surmountTarget = 0;
  private surmountHue = 0;
  private lastT = 0;

  constructor(layout: Layout) {
    this.width = layout.width;
    this.height = layout.height;
    this.ledCount = layout.led_count;
    this.keys = layout.keys.map((k) => {
      const x = k.x + k.w / 2;
      const y = k.y + k.h / 2;
      return { slot: k.slot, x, y, nx: x / layout.width, ny: y / layout.height };
    });
  }

  /** A key was pressed (on the real keyboard or clicked in the preview). */
  press(slot: number, t: number) {
    const key = this.keys.find((k) => k.slot === slot);
    if (!key) return;
    this.pressCount++;
    // Small steps: it takes a lot of typing to move to another color.
    this.surmountTarget += 5;
    this.presses.push({ key, t, hue: hash(this.pressCount) * 360 });
    this.presses = this.presses.filter((p) => t - p.t < PRESS_LIFE);
  }

  /** Colors of every LED slot at time `t` (seconds). */
  frame(fx: EffectState, t: number): Rgb[] {
    const out: Rgb[] = Array.from({ length: this.ledCount }, () => [0, 0, 0] as Rgb);
    const f = SPEED[Math.min(3, Math.max(0, fx.speed))];
    const dir = fx.direction === "right" ? 1 : -1;
    // A color for a key: the chosen one at brightness `b`, or a hue when colors are random.
    const paint = (h: number, b: number): Rgb => (fx.random ? hue(h, b) : scale(fx.color, clamp01(b)));
    const recent = this.presses.filter((p) => t - p.t < PRESS_LIFE);
    const typing = recent.filter((p) => t - p.t < 2).length;
    // Surmount glides to its new color instead of jumping.
    const dt = Math.min(0.25, Math.max(0, t - this.lastT));
    this.lastT = t;
    this.surmountHue += (this.surmountTarget - this.surmountHue) * Math.min(1, dt * 0.8);

    for (const k of this.keys) {
      out[k.slot] = this.key(fx.id, k, t, f, dir, fx.random, paint, recent, typing);
    }
    return out;
  }

  private key(
    id: string,
    k: SimKey,
    t: number,
    f: number,
    dir: number,
    random: boolean,
    paint: (h: number, b: number) => Rgb,
    presses: Press[],
    typing: number,
  ): Rgb {
    const ft = t * f;
    // Random colors on the keyboard come in two kinds, depending on the effect: a fixed
    // rainbow from red (left) to violet (right), or a random color for every key.
    const rainbow = k.nx * 290;
    const perKey = (round: number) => hash(k.slot * 31 + round) * 360;
    switch (id) {
      case "wave": {
        // Continuous, smooth wave across the keyboard. With random colors the whole
        // keyboard stays lit and a rainbow slides sideways.
        const phase = k.nx - dir * ft * 0.22;
        if (random) return hue(phase * 360);
        return paint(phase * 360, 0.55 + 0.45 * Math.cos(TAU * phase * 1.4));
      }
      case "wave_2": {
        // Flashing sweep, like an ambulance light, then a pause. With random colors it is
        // a sliding rainbow like Wave, only faster.
        if (random) return hue((k.nx - dir * ft * 0.5) * 360);
        const cycle = 3 / f;
        const c = Math.floor(t / cycle);
        const p = (t % cycle) / cycle;
        const x = dir > 0 ? k.nx : 1 - k.nx;
        const band = p < 0.55 ? gauss(x - p / 0.55, 0.09) : 0;
        return paint(c * 70 + k.nx * 120, band);
      }
      case "spiral": {
        // Soft rotation around the center.
        const a = Math.atan2(k.y - this.height / 2, k.x - this.width / 2) / TAU;
        const phase = a - dir * ft * 0.18;
        return paint(phase * 360, 0.3 + 0.7 * (0.5 + 0.5 * Math.cos(TAU * phase)));
      }
      case "acid":
        // The whole keyboard drifts through the colors.
        return hue(ft * 35);
      case "breathing": {
        const period = 4.5;
        return paint(rainbow, 0.5 - 0.5 * Math.cos((TAU * ft) / period));
      }
      case "static":
        return paint(rainbow, 1);
      case "ripple": {
        // A ring grows from every pressed key.
        let best: Rgb = [0, 0, 0];
        let bestB = 0;
        presses.forEach((p, i) => {
          const age = t - p.t;
          const d = Math.hypot(k.x - p.key.x, (k.y - p.key.y) * 1.2);
          const b = gauss(d - age * 9 * f, 0.9) * clamp01(1 - age / 2);
          if (b > bestB) {
            bestB = b;
            best = paint(perKey(i), b);
          }
        });
        return best;
      }
      case "reactive": {
        // The pressed key lights up, then fades.
        const p = presses.findLast((p) => p.key.slot === k.slot);
        if (!p) return [0, 0, 0];
        return paint(p.hue, clamp01(1 - ((t - p.t) * f) / 1.6));
      }
      case "reactive_row": {
        // The pressed key's row lights up from the key outward, then fades.
        let best: Rgb = [0, 0, 0];
        let bestB = 0;
        presses.forEach((p, i) => {
          if (Math.abs(k.y - p.key.y) > 0.5) return;
          const age = t - p.t;
          const reached = Math.abs(k.x - p.key.x) <= age * 30 * f;
          const b = reached ? clamp01(1 - (age * f) / 1.4) : 0;
          if (b > bestB) {
            bestB = b;
            best = paint(perKey(i), b);
          }
        });
        return best;
      }
      case "matrix_3":
      case "matrix_2": {
        // Soft drops down the columns: each key fades in and out, as if breathing.
        const col = Math.round(k.x);
        const speed = 0.9 * (0.4 + 0.25 * f) * (0.7 + 0.6 * hash(col));
        const span = this.height + 4;
        const head = fract(t * speed * 0.18 + hash(col + 50)) * span - 2;
        const drop = gauss(k.y - head, 1.1);
        const base = id === "matrix_2" ? 0.35 : 0;
        return paint(hash(col) * 360, base + (1 - base) * drop);
      }
      case "digital_times": {
        // Like Matrix, with longer trails.
        const col = Math.round(k.x);
        const trail = 3.5;
        const speed = 2.2 * f * (0.7 + 0.6 * hash(col));
        const head = fract(t * speed * 0.12 + hash(col + 50)) * (this.height + trail + 2) - 1;
        const behind = head - k.y;
        const drop = behind >= 0 && behind <= trail ? 1 - behind / trail : 0;
        return paint(rainbow, drop);
      }
      case "rainbow": {
        // Light flows vertically, row by row, blended.
        const phase = k.ny + ft * 0.25;
        return paint(phase * 360, 0.45 + 0.55 * (0.5 + 0.5 * Math.cos(TAU * phase)));
      }
      case "heartbeat": {
        // A sine curve born at the center and running out to both sides.
        const cycle = Math.floor(ft * 0.9);
        const curve = 0.5 + 0.38 * Math.sin(TAU * (Math.abs(k.nx - 0.5) * 2.4 - ft * 0.9));
        return paint(perKey(cycle), gauss(k.ny - curve, 0.13));
      }
      case "kamehameha": {
        // Light enters from the top-left and bottom-right, merges into a pulsing sphere at
        // the center, goes dark for a moment, then a shockwave lights the whole keyboard.
        const cycle = 5.5 / f;
        const c = Math.floor(t / cycle);
        const p = (t % cycle) / cycle;
        const ax = (k.nx - 0.5) * 2.2;
        const ay = (k.ny - 0.5) * 1.2;
        const d = Math.hypot(ax, ay);
        let b = 0;
        if (p < 0.3) {
          const s = 1 - p / 0.3;
          b = Math.max(gauss(Math.hypot(ax + 1.1 * s, ay + 0.6 * s), 0.35), gauss(Math.hypot(ax - 1.1 * s, ay - 0.6 * s), 0.35));
        } else if (p < 0.55) {
          b = gauss(d, 0.25 + 0.08 * Math.sin(TAU * ((p - 0.3) / 0.25) * 2));
        } else if (p < 0.62) {
          b = 0;
        } else {
          const r = ((p - 0.62) / 0.25) * 1.6;
          const lit = d <= r ? 1 : gauss(d - r, 0.15);
          b = lit * clamp01(1 - Math.max(0, p - 0.87) / 0.13);
        }
        return paint(perKey(c), b);
      }
      case "ping_pong": {
        // A diagonal band bouncing between the sides, tilted the way it travels;
        // it speeds up while typing.
        const boost = 1 + Math.min(2, typing * 0.25);
        const tri = fract(ft * 0.15 * boost) * 2;
        const forward = tri < 1;
        const pos = (forward ? tri : 2 - tri) * 1.3 - 0.15;
        const u = k.nx + (forward ? 0.25 : -0.25) * (k.ny - 0.5);
        return paint(u * 300, gauss(u - pos, 0.07));
      }
      case "surmount":
        // A fixed color that moves on, gently, while typing.
        return hue(this.surmountHue);
      case "glorious": {
        // Light born at the center, spreading to both sides like a pulse.
        const d = Math.abs(k.nx - 0.5) * 2;
        return paint(d * 200 - ft * 60, 0.25 + 0.75 * (0.5 + 0.5 * Math.cos(TAU * (d * 1.5 - ft * 0.45))));
      }
      default:
        return paint(0, 1);
    }
  }
}

/** Our key id for a physical key (`KeyboardEvent.code`), independent of the OS layout. */
export function keyIdFromCode(code: string): string | undefined {
  const aliases: Record<string, string> = {
    Escape: "Esc",
    Backquote: "Grave",
    BracketLeft: "LeftBracket",
    BracketRight: "RightBracket",
    Backslash: "NonUsHash",
    IntlBackslash: "NonUsBackslash",
    ShiftLeft: "ShiftL",
    ShiftRight: "ShiftR",
    ControlLeft: "CtrlL",
    ControlRight: "CtrlR",
    MetaLeft: "Win",
    AltLeft: "Alt",
    AltRight: "AltGr",
    ContextMenu: "Menu",
    ArrowUp: "Up",
    ArrowDown: "Down",
    ArrowLeft: "Left",
    ArrowRight: "Right",
    NumpadDivide: "NumSlash",
    NumpadMultiply: "NumStar",
    NumpadSubtract: "NumMinus",
    NumpadAdd: "NumPlus",
    NumpadEnter: "NumEnter",
    NumpadDecimal: "NumPeriod",
  };
  if (aliases[code]) return aliases[code];
  if (code.startsWith("Key")) return code.slice(3);
  if (code.startsWith("Digit")) return code.slice(5);
  if (/^Numpad\d$/.test(code)) return "Num" + code.slice(6);
  return code || undefined;
}
