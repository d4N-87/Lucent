// Turns a picture into per-key colors: the picture is cropped to the keyboard's proportions
// and every key takes the average color of the area under it, so wide keys (Space, Shift)
// get their whole strip instead of one grid cell.

import type { Layout, Rgb } from "./api";
import { hsvToRgb, rgbToHsv } from "./color";

/** Which part of the picture lands on the keyboard. */
export interface Crop {
  /** Center of the crop, as a fraction of the picture's width and height. */
  cx: number;
  cy: number;
  /** 1 = the largest crop with the keyboard's proportions; higher zooms in. */
  zoom: number;
}

export const DEFAULT_CROP: Crop = { cx: 0.5, cy: 0.5, zoom: 1 };

/** Pixels per key unit used for sampling. */
const SAMPLE_SCALE = 10;
/** Margin left out on each side of a key, in key units, so neighbors don't bleed in. */
const KEY_INSET = 0.12;
/** Below this brightness a key is turned off: near-black looks like a dim smudge on LEDs. */
const OFF_BELOW = 0.06;

/** Source rectangle, in picture pixels, for a crop. */
export function cropRect(imgW: number, imgH: number, aspect: number, crop: Crop) {
  let sw = imgW;
  let sh = imgW / aspect;
  if (sh > imgH) {
    sh = imgH;
    sw = imgH * aspect;
  }
  sw /= crop.zoom;
  sh /= crop.zoom;
  const sx = Math.min(imgW - sw, Math.max(0, crop.cx * imgW - sw / 2));
  const sy = Math.min(imgH - sh, Math.max(0, crop.cy * imgH - sh / 2));
  return { sx, sy, sw, sh };
}

/** Keeps the crop inside the picture (the center can't drift past what the zoom allows). */
export function clampCrop(imgW: number, imgH: number, aspect: number, crop: Crop): Crop {
  const zoom = Math.min(8, Math.max(1, crop.zoom));
  const { sx, sy, sw, sh } = cropRect(imgW, imgH, aspect, { ...crop, zoom });
  return { zoom, cx: (sx + sw / 2) / imgW, cy: (sy + sh / 2) / imgH };
}

/**
 * Average color under every key. `vividness` 0..1 raises saturation, since photos look
 * dull on LEDs.
 */
export function sampleKeys(
  image: CanvasImageSource & { width: number; height: number },
  layout: Layout,
  crop: Crop,
  vividness: number,
): Map<number, Rgb> {
  const w = Math.round(layout.width * SAMPLE_SCALE);
  const h = Math.round(layout.height * SAMPLE_SCALE);
  const canvas = new OffscreenCanvas(w, h);
  const ctx = canvas.getContext("2d", { willReadFrequently: true })!;
  ctx.fillStyle = "#000";
  ctx.fillRect(0, 0, w, h);
  const { sx, sy, sw, sh } = cropRect(image.width, image.height, layout.width / layout.height, crop);
  ctx.imageSmoothingQuality = "high";
  ctx.drawImage(image, sx, sy, sw, sh, 0, 0, w, h);
  const data = ctx.getImageData(0, 0, w, h).data;

  const out = new Map<number, Rgb>();
  for (const k of layout.keys) {
    const x0 = Math.floor((k.x + KEY_INSET) * SAMPLE_SCALE);
    const y0 = Math.floor((k.y + KEY_INSET) * SAMPLE_SCALE);
    const x1 = Math.ceil((k.x + k.w - KEY_INSET) * SAMPLE_SCALE);
    const y1 = Math.ceil((k.y + k.h - KEY_INSET) * SAMPLE_SCALE);
    let r = 0;
    let g = 0;
    let b = 0;
    let n = 0;
    for (let y = Math.max(0, y0); y < Math.min(h, y1); y++) {
      for (let x = Math.max(0, x0); x < Math.min(w, x1); x++) {
        const i = (y * w + x) * 4;
        // Transparent areas count as black (an unlit key).
        const a = data[i + 3] / 255;
        r += data[i] * a;
        g += data[i + 1] * a;
        b += data[i + 2] * a;
        n++;
      }
    }
    if (n === 0) continue;
    out.set(k.slot, toLed([r / n, g / n, b / n], vividness));
  }
  return out;
}

function toLed(rgb: number[], vividness: number): Rgb {
  const hsv = rgbToHsv(rgb.map((c) => Math.round(c)) as Rgb);
  if (hsv.v < OFF_BELOW) return [0, 0, 0];
  return hsvToRgb({ h: hsv.h, s: Math.min(1, hsv.s * (1 + vividness * 1.5)), v: hsv.v });
}

/**
 * A PNG with the outline of every key, at `scale` pixels per key unit, to draw pixel art on.
 * Keys are white outlines with their legend on a dark board.
 */
export async function keyTemplate(layout: Layout, scale = 40): Promise<Blob> {
  const canvas = new OffscreenCanvas(Math.round(layout.width * scale), Math.round(layout.height * scale));
  const ctx = canvas.getContext("2d")!;
  ctx.fillStyle = "#111115";
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  ctx.strokeStyle = "rgba(255, 255, 255, 0.55)";
  ctx.lineWidth = 1;
  ctx.fillStyle = "rgba(255, 255, 255, 0.35)";
  ctx.font = `${Math.round(scale * 0.22)}px sans-serif`;
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  for (const k of layout.keys) {
    const x = k.x * scale + 2;
    const y = k.y * scale + 2;
    const w = k.w * scale - 4;
    const h = k.h * scale - 4;
    ctx.beginPath();
    ctx.roundRect(x + 0.5, y + 0.5, w, h, scale * 0.12);
    ctx.stroke();
    ctx.fillText(k.label, x + w / 2, y + h / 2);
  }
  return canvas.convertToBlob({ type: "image/png" });
}
