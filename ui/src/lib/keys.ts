import { invoke } from "@tauri-apps/api/core";
import type { Layout, OsLayout } from "./api";
import { t } from "./i18n.svelte";
import { keyIdFromCode } from "./simulate";

/** What a key does (see KeyAction in crates/lucent-core/src/driver.rs). Codes are USB HID usages. */
export type KeyAction =
  | { kind: "key"; code: number }
  | { kind: "media"; code: number }
  | { kind: "macro"; index: number }
  | { kind: "disabled" }
  | { kind: "fn" }
  | { kind: "raw"; bytes: number[] };

export interface MacroStep {
  code: number;
  pressed: boolean;
  /** Wait after this step. */
  delay_ms: number;
}

export interface Macro {
  name: string;
  repeat: number;
  steps: MacroStep[];
}

/** Every key's function, by slot, and the macros stored in the keyboard. */
export interface KeyConfig {
  keys: Record<string, KeyAction>;
  macros: Macro[];
}

export interface KeymapSupport {
  macros: boolean;
  max_macros: number;
  max_macro_steps: number;
  max_delay_ms: number;
  delay_step_ms: number;
  verified: boolean;
}

export const readKeys = () =>
  invoke<{ config: KeyConfig; defaults: Record<string, KeyAction> }>("read_keys");
export const writeKeys = (config: KeyConfig) => invoke<void>("write_keys", { config });

/** Media keys offered for a key; names are `media.<id>` in the language files. */
export const MEDIA: { id: string; code: number }[] = [
  { id: "play_pause", code: 0xcd },
  { id: "previous", code: 0xb6 },
  { id: "next", code: 0xb5 },
  { id: "stop", code: 0xb7 },
  { id: "mute", code: 0xe2 },
  { id: "volume_down", code: 0xea },
  { id: "volume_up", code: 0xe9 },
  { id: "calculator", code: 0x192 },
  { id: "browser", code: 0x223 },
  { id: "mail", code: 0x18a },
  { id: "computer", code: 0x194 },
];

export const mediaId = (code: number) => MEDIA.find((m) => m.code === code)?.id;

/**
 * JSON with object keys sorted, to compare values: profiles read back from disk have their keys in
 * another order than the ones built here.
 */
export const canonical = (value: unknown) =>
  JSON.stringify(value, (_, v) =>
    v && typeof v === "object" && !Array.isArray(v)
      ? Object.fromEntries(Object.entries(v).sort(([x], [y]) => (x < y ? -1 : x > y ? 1 : 0)))
      : v,
  );

export const sameAction = (a: KeyAction | undefined, b: KeyAction | undefined) =>
  canonical(a ?? null) === canonical(b ?? null);

/** Same keys and macros, whatever the order of the slots. */
export function sameConfig(a: KeyConfig, b: KeyConfig): boolean {
  const norm = (c: KeyConfig) =>
    canonical([Object.entries(c.keys).sort(([x], [y]) => Number(x) - Number(y)), c.macros]);
  return norm(a) === norm(b);
}

/** What differs between two setups: slots, and macro positions. */
export function configDiff(a: KeyConfig, b: KeyConfig): { slots: number[]; macros: number[] } {
  const slots = [...new Set([...Object.keys(a.keys), ...Object.keys(b.keys)])]
    .filter((slot) => !sameAction(a.keys[slot], b.keys[slot]))
    .map(Number);
  const macros = Array.from({ length: Math.max(a.macros.length, b.macros.length) }, (_, i) => i).filter(
    (i) => canonical(a.macros[i] ?? null) !== canonical(b.macros[i] ?? null),
  );
  return { slots, macros };
}

export const cloneConfig = (c: KeyConfig): KeyConfig => JSON.parse(JSON.stringify(c));

/** How a key differs from the factory: drives its color and badge on the keyboard. */
export type KeyKind = "default" | "key" | "media" | "macro" | "disabled";

export function kindOf(action: KeyAction | undefined, factory: KeyAction | undefined): KeyKind {
  if (!action || sameAction(action, factory)) return "default";
  if (action.kind === "media" || action.kind === "macro" || action.kind === "disabled") return action.kind;
  return "key";
}

/** Key names from the layout and the factory keymap: HID code ↔ key ↔ browser key code. */
export class KeyNames {
  private bySlot = new Map<number, { id: string; label: string }>();
  private slotByCode = new Map<number, number>();
  private typing: Map<string, Typing>;
  /** Name of the Windows layout text macros are typed with, e.g. "Italiano". */
  readonly layoutName: string;

  constructor(layout: Layout, defaults: Record<string, KeyAction>) {
    for (const k of layout.keys) this.bySlot.set(k.slot, { id: k.id, label: k.label || t("common.space") });
    for (const [slot, action] of Object.entries(defaults)) {
      if (action.kind === "key") this.slotByCode.set(action.code, Number(slot));
    }
    const ids = new Set(layout.keys.map((k) => k.id));
    this.typing = typingMap(layout.typing, ids);
    this.layoutName = layout.typing?.name ?? "Windows";
  }

  /** Printed label of the key that sends `code` at the factory, e.g. 0x29 → "Esc". */
  label(code: number): string {
    const slot = this.slotByCode.get(code);
    return slot === undefined ? `0x${code.toString(16).padStart(2, "0")}` : this.bySlot.get(slot)!.label;
  }

  /** Stable key id for `code` (MCP), e.g. 0x29 → "Esc". */
  id(code: number): string | undefined {
    const slot = this.slotByCode.get(code);
    return slot === undefined ? undefined : this.bySlot.get(slot)?.id;
  }

  /** HID code the factory keymap gives this key id. */
  codeOfId(id: string): number | undefined {
    const n = id.trim().toLowerCase();
    for (const [code, slot] of this.slotByCode) {
      if (this.bySlot.get(slot)?.id.toLowerCase() === n) return code;
    }
    return undefined;
  }

  /** HID code of a physical key from a browser event (`KeyboardEvent.code`). */
  codeOfEvent(eventCode: string): number | undefined {
    const id = keyIdFromCode(eventCode);
    return id ? this.codeOfId(id) : undefined;
  }

  /** Keys that can be chosen as a replacement, by label. */
  choices(): { code: number; label: string }[] {
    return [...this.slotByCode.keys()]
      .map((code) => ({ code, label: this.label(code) }))
      .sort((a, b) => a.label.localeCompare(b.label, undefined, { numeric: true }));
  }

  /**
   * Steps that type `text` with the Windows keyboard layout: Shift and AltGr are pressed around the
   * keys that need them, and held over a run of characters that need the same ones, to save steps.
   * A character on a dead key (an accent alone) is its key followed by Space.
   * Returns the first character it can't type instead.
   */
  textSteps(text: string, delay: number): MacroStep[] | { unsupported: string } {
    const steps: MacroStep[] = [];
    const step = (code: number, pressed: boolean) => steps.push({ code, pressed, delay_ms: delay });
    const tap = (code: number) => (step(code, true), step(code, false));
    let held: number[] = [];
    const hold = (wanted: number[]) => {
      if (wanted.join() === held.join()) return;
      for (const m of [...held].reverse()) step(m, false);
      for (const m of wanted) step(m, true);
      held = wanted;
    };
    const space = this.codeOfId("Space");
    for (const ch of text.replace(/\r\n?/g, "\n")) {
      const typed = this.typing.get(ch);
      const code = typed && this.codeOfId(typed.key);
      const mods = typed?.mods.map((m) => this.codeOfId(m));
      if (code === undefined || !mods || mods.some((m) => m === undefined)) return { unsupported: ch };
      if (typed!.dead && space === undefined) return { unsupported: ch };
      hold(mods as number[]);
      tap(code);
      if (typed!.dead) {
        hold([]);
        tap(space!);
      }
    }
    hold([]);
    return steps;
  }
}

/** Key and modifiers (by key id) that type a character. */
interface Typing {
  key: string;
  mods: string[];
  dead: boolean;
}

/** For each character, the key that types it on this Windows layout, with the fewest modifiers;
 *  only keys the model has. Space, Enter and Tab always type themselves. */
function typingMap(os: OsLayout | null, ids: Set<string>): Map<string, Typing> {
  const map = new Map<string, Typing>();
  const add = (ch: string, t: Typing) => {
    const had = map.get(ch);
    // A key that types the character outright beats a dead key followed by Space.
    if (!had || (had.dead && !t.dead)) map.set(ch, t);
  };
  for (const [ch, key] of [[" ", "Space"], ["\n", "Enter"], ["\t", "Tab"]]) {
    if (ids.has(key)) add(ch, { key, mods: [], dead: false });
  }
  if (!os) return map;
  const combos: [keyof OsLayout["keys"][string], string[]][] = [
    ["base", []],
    ["shift", ["ShiftL"]],
    ["altgr", ["AltGr"]],
    ["shift_altgr", ["AltGr", "ShiftL"]],
  ];
  for (const [combo, mods] of combos) {
    for (const [key, chars] of Object.entries(os.keys)) {
      const typed = chars[combo];
      if (!typed || !ids.has(key) || [...typed.text].length !== 1) continue;
      add(typed.text, { key, mods, dead: typed.dead });
    }
  }
  return map;
}

/** Short text for what a key does, shown on the keycap and in lists. */
export function describe(action: KeyAction, names: KeyNames, macros: Macro[]): string {
  switch (action.kind) {
    case "key":
      return names.label(action.code);
    case "media": {
      const id = mediaId(action.code);
      return id ? t(`media.${id}.short`) : `0x${action.code.toString(16)}`;
    }
    case "macro":
      return macros[action.index]?.name ?? t("keys.macro_missing");
    case "disabled":
      return t("keys.disabled");
    case "fn":
      return "Fn";
    case "raw":
      return "?";
  }
}
