//! Key functions and macros of the GMMK v1 (capture 04_macro, docs/protocollo.md).
//!
//! The base layer is 126 slots of 3 bytes (read 0x07, write 0x08 at offset 0; read 0x0F always
//! answers the factory layer, even after a write). Macros live
//! in their own area (read 0x09, write 0x0A), always rewritten whole. Checked on the keyboard
//! (October 2026): remaps, media keys, disabled keys, macros with Ctrl, several macros.

use crate::driver::{KeyAction, KeyConfig, Macro, MacroStep};
use std::collections::BTreeMap;

pub const SLOTS: usize = 126;
pub const LAYER_BYTES: usize = SLOTS * 3;
/// Layer 0 of the full-size ISO IT as it left the factory, read from the keyboard before any change.
/// Other models read theirs from the keyboard (command 0x0F always answers the factory layer).
pub const FACTORY_LAYER: &[u8; LAYER_BYTES] = include_bytes!("gmmk_v1_factory_keymap.bin");

/// A 100-unit pause played as about a second on the keyboard.
pub const DELAY_UNIT_MS: u32 = 10;
/// Limits proven on the keyboard (capture 06, October 2026): the official app wrote 17 macros, one
/// of 186 steps, in a 990-byte area, and the keyboard kept them all. With their names the area would
/// have been ~1200 bytes and the official app reported a lack of space, so the real size is between
/// the two: probably 1024 bytes, not proven.
pub const MAX_MACROS: usize = 17;
pub const MAX_MACRO_STEPS: usize = 186;
pub const MAX_MACRO_AREA: usize = 990;
/// Largest macro area accepted when reading: generous over the ~1024 bytes the keyboard
/// probably has, since another app may have written more than we would.
pub const MAX_MACRO_AREA_READ: usize = 2048;

const MACRO_HEADER: usize = 16;
const PRESSED: u8 = 0xA0;
const RELEASED: u8 = 0x20;

pub fn decode_slot(b: &[u8]) -> KeyAction {
    match *b {
        [0x02, 0x02, code] => KeyAction::Key { code },
        [0x02, 0x01, bits] if bits.count_ones() == 1 => KeyAction::Key { code: 0xE0 + bits.trailing_zeros() as u8 },
        [0x03, lo, hi] => KeyAction::Media { code: u16::from_le_bytes([lo, hi]) },
        [0x05, 0x01, index] => KeyAction::Macro { index },
        [0x05, 0x02, 0x02] => KeyAction::Fn,
        [0x00, 0x00, 0x00] => KeyAction::Disabled,
        _ => KeyAction::Raw { bytes: b.to_vec() },
    }
}

pub fn encode_slot(action: &KeyAction) -> Result<[u8; 3], String> {
    Ok(match action {
        KeyAction::Key { code: code @ 0xE0..=0xE7 } => [0x02, 0x01, 1 << (code - 0xE0)],
        KeyAction::Key { code } => [0x02, 0x02, *code],
        KeyAction::Media { code } => {
            let [lo, hi] = code.to_le_bytes();
            [0x03, lo, hi]
        }
        KeyAction::Macro { index } => [0x05, 0x01, *index],
        KeyAction::Disabled => [0x00, 0x00, 0x00],
        KeyAction::Fn => [0x05, 0x02, 0x02],
        KeyAction::Raw { bytes } => bytes
            .as_slice()
            .try_into()
            .map_err(|_| format!("A raw key code must be 3 bytes, got {}", bytes.len()))?,
    })
}

/// The keys of one model: the slots its layout draws, and its factory layer.
pub struct Keymap {
    slots: Vec<usize>,
    factory: Vec<u8>,
}

impl Keymap {
    /// `slots` are the model's keys (every other slot is kept as read); `factory` is its layer 0.
    pub fn new(slots: Vec<usize>, factory: Vec<u8>) -> Result<Self, String> {
        if factory.len() < LAYER_BYTES {
            return Err(format!("The factory layer has {} bytes, {LAYER_BYTES} expected", factory.len()));
        }
        if let Some(s) = slots.iter().find(|&&s| s >= SLOTS) {
            return Err(format!("Key slot {s} is past the {SLOTS} keymap slots"));
        }
        Ok(Self { slots, factory })
    }

    fn factory_slot(&self, slot: usize) -> &[u8] {
        &self.factory[3 * slot..3 * slot + 3]
    }

    pub fn factory_keys(&self) -> BTreeMap<usize, KeyAction> {
        self.slots.iter().map(|&s| (s, decode_slot(self.factory_slot(s)))).collect()
    }

    /// The functions of the drawn keys in a layer.
    pub fn decode_layer(&self, layer: &[u8]) -> BTreeMap<usize, KeyAction> {
        self.slots.iter().map(|&s| (s, decode_slot(&layer[3 * s..3 * s + 3]))).collect()
    }

    pub fn decode(&self, layer: &[u8], macro_area: &[u8]) -> Result<KeyConfig, String> {
        Ok(KeyConfig { keys: self.decode_layer(layer), macros: decode_macros(macro_area)? })
    }

    /// `current` with the drawn keys set from `keys` (factory function where missing). The Fn key
    /// always keeps its factory function so the keyboard can't lose it. Raw codes the app doesn't
    /// understand are only kept where the keyboard or the factory already had them: a profile
    /// file can't bring in bytes of its own.
    pub fn encode_layer(
        &self,
        current: &[u8],
        keys: &BTreeMap<usize, KeyAction>,
        macros: usize,
    ) -> Result<Vec<u8>, String> {
        let mut out = current[..LAYER_BYTES].to_vec();
        for &slot in &self.slots {
            let factory: [u8; 3] = self.factory_slot(slot).try_into().unwrap();
            let bytes = match keys.get(&slot) {
                _ if decode_slot(&factory) == KeyAction::Fn => factory,
                None => factory,
                Some(KeyAction::Fn) => return Err("Only the Fn key can be Fn".into()),
                Some(KeyAction::Macro { index }) if *index as usize >= macros => {
                    return Err(format!("Key slot {slot} plays macro {index}, but there are {macros} macros"));
                }
                Some(KeyAction::Raw { bytes })
                    if bytes[..] != factory[..] && bytes[..] != current[3 * slot..3 * slot + 3] =>
                {
                    return Err(format!("Key slot {slot} has an unknown raw code {bytes:02x?}"));
                }
                Some(action) => encode_slot(action)?,
            };
            out[3 * slot..3 * slot + 3].copy_from_slice(&bytes);
        }
        Ok(out)
    }
}

/// The keyboard event of a macro step: modifiers as a bit, like in the keymap (capture 05).
fn step_key(code: u8) -> (u8, u8) {
    match code {
        0xE0..=0xE7 => (0x01, 1 << (code - 0xE0)),
        _ => (0x02, code),
    }
}

fn step_code(kind: u8, value: u8) -> Option<u8> {
    match kind {
        0x02 => Some(value),
        0x01 if value.count_ones() == 1 => Some(0xE0 + value.trailing_zeros() as u8),
        _ => None,
    }
}

/// Macro area: a 16-byte header, a table with the offset of each macro (u16, from the start
/// of the area), then the macros: steps (u16), repeat (u8), name length (u8), 4 bytes per
/// step, the name in UTF-16LE. Captures 04 and 05.
pub fn encode_macros(macros: &[Macro]) -> Result<Vec<u8>, String> {
    if macros.len() > MAX_MACROS {
        return Err(format!("At most {MAX_MACROS} macros fit in the keyboard"));
    }
    let mut records = Vec::new();
    for m in macros {
        if m.steps.is_empty() || m.steps.len() > MAX_MACRO_STEPS {
            return Err(format!("Macro \"{}\" needs 1 to {MAX_MACRO_STEPS} steps", m.name));
        }
        let name: Vec<u16> = m.name.encode_utf16().take(32).collect();
        let mut r = Vec::new();
        r.extend_from_slice(&(m.steps.len() as u16).to_le_bytes());
        r.push(m.repeat.max(1));
        r.push(name.len() as u8);
        for step in &m.steps {
            let (kind, value) = step_key(step.code);
            let delay = (step.delay_ms + DELAY_UNIT_MS / 2) / DELAY_UNIT_MS;
            r.push(delay.min(255) as u8);
            r.push(if step.pressed { PRESSED } else { RELEASED });
            r.push(kind);
            r.push(value);
        }
        for unit in name {
            r.extend_from_slice(&unit.to_le_bytes());
        }
        records.push(r);
    }
    let mut offset = MACRO_HEADER + 2 * macros.len();
    let mut table = Vec::new();
    for r in &records {
        table.extend_from_slice(&(offset as u16).to_le_bytes());
        offset += r.len();
    }
    let mut out = vec![0xAA, 0x55];
    out.extend_from_slice(&(offset as u16).to_le_bytes());
    out.extend_from_slice(&(macros.len() as u16).to_le_bytes());
    out.extend_from_slice(&[0x01, 0x00]);
    // Unknown field: ff ff ff with no macros; 19 with 4 steps in all, 27 with 8 (captures 04
    // and 05), so taken as 2 per step + 11.
    if macros.is_empty() {
        out.extend_from_slice(&[0xFF, 0xFF, 0xFF]);
    } else {
        let steps: usize = macros.iter().map(|m| m.steps.len()).sum();
        out.extend_from_slice(&((2 * steps + 11) as u16).to_le_bytes());
        out.push(0);
    }
    out.extend_from_slice(&[0; 5]);
    out.extend_from_slice(&table);
    for r in records {
        out.extend_from_slice(&r);
    }
    if out.len() > MAX_MACRO_AREA {
        return Err(format!(
            "The macros take {} bytes, more than the {MAX_MACRO_AREA} known to fit: shorten them or remove one",
            out.len()
        ));
    }
    Ok(out)
}

/// Length of the macro area from its first 4 bytes.
pub fn macro_area_len(head: &[u8]) -> Result<usize, String> {
    match head {
        [0xAA, 0x55, lo, hi, ..] => Ok(u16::from_le_bytes([*lo, *hi]) as usize),
        _ => Err(format!("Unexpected macro area header: {:02x?}", &head[..head.len().min(4)])),
    }
}

pub fn decode_macros(area: &[u8]) -> Result<Vec<Macro>, String> {
    let len = macro_area_len(area)?.min(area.len());
    let count = u16::from_le_bytes([area[4], area[5]]) as usize;
    let u16_at = |at: usize| area.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as usize);
    let mut macros = Vec::with_capacity(count);
    for i in 0..count {
        let short = || format!("Macro area ends inside macro {i}");
        let at = u16_at(MACRO_HEADER + 2 * i).ok_or_else(short)?;
        let head = area.get(at..at + 4).filter(|_| at + 4 <= len).ok_or_else(short)?;
        let steps = u16::from_le_bytes([head[0], head[1]]) as usize;
        let repeat = head[2];
        let name_len = head[3] as usize;
        let names_at = at + 4 + 4 * steps;
        let end = names_at + 2 * name_len;
        if end > len {
            return Err(short());
        }
        let mut list = Vec::with_capacity(steps);
        for e in area[at + 4..names_at].chunks(4) {
            let code = step_code(e[2], e[3]).ok_or_else(|| format!("Macro {i}: unknown step {e:02x?}"))?;
            list.push(MacroStep { code, pressed: e[1] & 0x80 != 0, delay_ms: e[0] as u32 * DELAY_UNIT_MS });
        }
        let units: Vec<u16> = area[names_at..end].chunks(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        macros.push(Macro { name: String::from_utf16_lossy(&units), repeat, steps: list });
    }
    Ok(macros)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyboard::model::default_model;

    const FN_SLOT: usize = 91;

    fn keymap() -> Keymap {
        Keymap::new(default_model().slots(), FACTORY_LAYER.to_vec()).unwrap()
    }

    fn factory_keys() -> BTreeMap<usize, KeyAction> {
        keymap().factory_keys()
    }

    fn hex(s: &str) -> Vec<u8> {
        s.split_whitespace().map(|b| u8::from_str_radix(b, 16).unwrap()).collect()
    }

    /// Macro area written by the official app for "Macro1" = A, B (capture 04).
    const CAPTURED: &str = "aa 55 32 00 01 00 01 00 13 00 00 00 00 00 00 00 12 00 04 00 01 06 \
        05 a0 02 04 05 20 02 04 05 a0 02 05 00 20 02 05 4d 00 61 00 63 00 72 00 6f 00 31 00";

    #[test]
    fn decodes_factory_layer() {
        let keys = factory_keys();
        assert_eq!(keys[&1], KeyAction::Key { code: 0x29 }); // Esc
        assert_eq!(keys[&86], KeyAction::Key { code: 0xE0 }); // left Ctrl
        assert_eq!(keys[&FN_SLOT], KeyAction::Fn);
        assert!(!keys.values().any(|k| matches!(k, KeyAction::Raw { .. })), "{keys:?}");
    }

    #[test]
    fn slots_round_trip() {
        for action in factory_keys().values() {
            assert_eq!(&decode_slot(&encode_slot(action).unwrap()), action);
        }
        assert_eq!(encode_slot(&KeyAction::Media { code: 0xE9 }).unwrap(), [0x03, 0xE9, 0x00]);
        assert_eq!(encode_slot(&KeyAction::Macro { index: 0 }).unwrap(), [0x05, 0x01, 0x00]);
    }

    #[test]
    fn reproduces_the_captured_macro_area() {
        let macros = decode_macros(&hex(CAPTURED)).unwrap();
        assert_eq!(macros.len(), 1);
        assert_eq!(macros[0].name, "Macro1");
        assert_eq!(macros[0].steps.len(), 4);
        assert_eq!(macros[0].steps[0], MacroStep { code: 0x04, pressed: true, delay_ms: 50 });
        assert_eq!(encode_macros(&macros).unwrap(), hex(CAPTURED));
    }

    /// Two macros written by the official app: "uno" = Ctrl+C, "due" = x, y (capture 05).
    const CAPTURED_TWO: &str = "aa 55 48 00 02 00 01 00 1b 00 00 00 00 00 00 00 14 00 2e 00 04 00 01 03         4c a0 01 01 35 a0 02 06 32 20 02 06 00 20 01 01 75 00 6e 00 6f 00 04 00 01 03 09 a0 02 1b 80 20         02 1b 09 a0 02 1c 00 20 02 1c 64 00 75 00 65 00";

    #[test]
    fn reproduces_two_captured_macros_with_ctrl() {
        let macros = decode_macros(&hex(CAPTURED_TWO)).unwrap();
        assert_eq!(macros.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["uno", "due"]);
        assert_eq!(macros[0].steps[0], MacroStep { code: 0xE0, pressed: true, delay_ms: 760 });
        assert_eq!(macros[0].steps[1].code, 0x06); // C
        assert_eq!(encode_macros(&macros).unwrap(), hex(CAPTURED_TWO));
    }

    #[test]
    fn fits_what_the_official_app_wrote() {
        let step = |pressed| MacroStep { code: 0x04, pressed, delay_ms: 50 };
        let long = Macro { name: String::new(), repeat: 1, steps: (0..186).map(|i| step(i % 2 == 0)).collect() };
        let short = Macro { name: String::new(), repeat: 1, steps: vec![step(true), step(false)] };
        let mut macros = vec![long];
        macros.extend(std::iter::repeat_n(short, 16));
        assert_eq!(encode_macros(&macros).unwrap().len(), 990); // the size in capture 06
        macros[1].name = "x".into();
        assert!(encode_macros(&macros).is_err());
    }

    #[test]
    fn empty_macro_area_matches_capture() {
        assert_eq!(encode_macros(&[]).unwrap(), hex("aa 55 10 00 00 00 01 00 ff ff ff 00 00 00 00 00"));
    }

    #[test]
    fn layer_keeps_unknown_slots_and_fn() {
        let mut current = FACTORY_LAYER.to_vec();
        current[3 * 119] = 0x77; // a slot without a key
        let mut keys = BTreeMap::new();
        keys.insert(52, KeyAction::Key { code: 0x29 }); // Caps Lock -> Esc
        keys.insert(FN_SLOT, KeyAction::Disabled);
        let out = keymap().encode_layer(&current, &keys, 0).unwrap();
        assert_eq!(&out[3 * 52..3 * 52 + 3], &[0x02, 0x02, 0x29]);
        assert_eq!(&out[3 * FN_SLOT..3 * FN_SLOT + 3], &[0x05, 0x02, 0x02]);
        assert_eq!(out[3 * 119], 0x77);
        assert!(keymap().encode_layer(&current, &BTreeMap::from([(52, KeyAction::Macro { index: 0 })]), 0).is_err());
    }

    #[test]
    fn raw_codes_only_where_the_keyboard_had_them() {
        let mut current = FACTORY_LAYER.to_vec();
        current[3 * 52..3 * 52 + 3].copy_from_slice(&[0x09, 0x01, 0x02]);
        let raw = |bytes: [u8; 3]| BTreeMap::from([(52, KeyAction::Raw { bytes: bytes.to_vec() })]);
        assert!(keymap().encode_layer(&current, &raw([0x09, 0x01, 0x02]), 0).is_ok());
        // Fn on another key, or a code the keyboard never had: refused.
        assert!(keymap().encode_layer(&current, &raw([0x05, 0x02, 0x02]), 0).is_err());
        assert!(keymap().encode_layer(&current, &raw([0x07, 0x00, 0x00]), 0).is_err());
    }
}
