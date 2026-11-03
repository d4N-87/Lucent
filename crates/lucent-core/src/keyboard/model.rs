//! Keyboard models and formats, described by JSON files.
//!
//! A **model** is a real keyboard: its USB ids, the driver that talks to it, and where each key
//! is (`slot` = the key's index in the keyboard's keymap and LED buffer). Built-in models live in
//! `models/`; contributors can drop more into a `models` folder next to the app (see
//! docs/nuova-tastiera.md), and those are never trusted as verified.
//!
//! A **format** (`formats/`) is only a drawing of a common layout (full-size, TKL, 75%, 65%, 60%,
//! ANSI or ISO), used to preview a keyboard the app doesn't know. It never drives a keyboard.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyDef {
    /// Index in the keyboard's keymap and LED buffer.
    pub slot: usize,
    /// Stable, layout-independent name used by the MCP tools and saved data, e.g. "ShiftL".
    pub id: String,
    /// What is printed on the keycap.
    pub label: String,
    /// Position and size in key units (1u = one standard key) from the top-left corner.
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsbIds {
    /// Hex, e.g. "0C45".
    pub vid: String,
    pub pid: String,
}

impl UsbIds {
    pub fn parse(&self) -> Option<(u16, u16)> {
        Some((u16::from_str_radix(&self.vid, 16).ok()?, u16::from_str_radix(&self.pid, 16).ok()?))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    pub id: String,
    pub name: String,
    /// "fullsize", "tkl", "75", "65" or "60".
    pub format: String,
    /// "ansi" or "iso".
    pub standard: String,
    /// Language of the printed legends, e.g. "it", "us".
    #[serde(default)]
    pub legends: String,
    /// Missing on formats, which only draw.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usb: Option<UsbIds>,
    /// Driver that talks to it (e.g. "gmmk_v1"); missing on formats.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub driver: Option<String>,
    /// Checked on the real keyboard. Files from the `models` folder are never verified.
    #[serde(default)]
    pub verified: bool,
    /// Size of the LED buffer; by default one past the highest slot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leds: Option<usize>,
    pub width: f32,
    pub height: f32,
    pub keys: Vec<KeyDef>,
}

impl Model {
    pub fn is_format(&self) -> bool {
        self.driver.is_none()
    }

    pub fn led_count(&self) -> usize {
        self.leds.unwrap_or_else(|| self.keys.iter().map(|k| k.slot + 1).max().unwrap_or(0))
    }

    pub fn slots(&self) -> Vec<usize> {
        self.keys.iter().map(|k| k.slot).collect()
    }

    /// Checks what the app relies on: unique slots and ids, slots inside the LED buffer.
    pub fn validate(&self) -> Result<(), String> {
        let mut slots = std::collections::HashSet::new();
        let mut ids = std::collections::HashSet::new();
        for k in &self.keys {
            if !slots.insert(k.slot) {
                return Err(format!("{}: slot {} is used twice", self.id, k.slot));
            }
            if !ids.insert(k.id.to_lowercase()) {
                return Err(format!("{}: key id {} is used twice", self.id, k.id));
            }
            if k.slot >= self.led_count() {
                return Err(format!("{}: slot {} is past the {} LEDs", self.id, k.slot, self.led_count()));
            }
        }
        if !self.is_format() && self.usb.as_ref().and_then(UsbIds::parse).is_none() {
            return Err(format!("{}: usb.vid and usb.pid must be hex numbers", self.id));
        }
        Ok(())
    }

    /// The ANSI shape of an ISO model: the key next to left Shift goes (Shift takes its place),
    /// Enter becomes one row high and wide, and the ISO key beside it becomes the Backslash above
    /// it. Every key keeps its slot. None if the model isn't ISO or lacks one of those keys.
    pub fn as_ansi(&self) -> Option<Model> {
        if self.standard != "iso" {
            return None;
        }
        let find = |id: &str| self.keys.iter().find(|k| k.id == id).cloned();
        let (enter, hash, extra) = (find("Enter")?, find("NonUsHash")?, find("NonUsBackslash")?);
        let (bracket, quote) = (find("RightBracket")?, find("Quote")?);
        find("ShiftL")?;
        let right = enter.x + enter.w;
        let mut ansi = self.clone();
        ansi.standard = "ansi".into();
        ansi.keys.retain(|k| k.id != "NonUsBackslash");
        for k in &mut ansi.keys {
            match k.id.as_str() {
                "NonUsHash" => {
                    *k = KeyDef { id: "Backslash".into(), label: "\\".into(), x: bracket.x + bracket.w, y: bracket.y, h: 1.0, ..hash.clone() };
                    k.w = right - k.x;
                }
                "Enter" => {
                    k.x = quote.x + quote.w;
                    k.y = quote.y;
                    k.w = right - k.x;
                    k.h = 1.0;
                }
                "ShiftL" => k.w += extra.w,
                _ => {}
            }
        }
        Some(ansi)
    }
}

/// The user's keyboard, the one every other model is measured against.
pub const DEFAULT_MODEL: &str = "gmmk_v1_fullsize_iso_it";

const BUILTIN_MODELS: &[&str] = &[include_str!("../../models/gmmk_v1_fullsize_iso_it.json")];

const BUILTIN_FORMATS: &[&str] = &[
    include_str!("../../formats/fullsize_ansi.json"),
    include_str!("../../formats/fullsize_iso.json"),
    include_str!("../../formats/tkl_ansi.json"),
    include_str!("../../formats/tkl_iso.json"),
    include_str!("../../formats/75_ansi.json"),
    include_str!("../../formats/75_iso.json"),
    include_str!("../../formats/65_ansi.json"),
    include_str!("../../formats/65_iso.json"),
    include_str!("../../formats/60_ansi.json"),
    include_str!("../../formats/60_iso.json"),
];

fn parse_builtin(text: &str) -> Model {
    let model: Model = serde_json::from_str(text).expect("built-in model file");
    model.validate().expect("built-in model file");
    model
}

pub fn builtin_models() -> Vec<Model> {
    BUILTIN_MODELS.iter().map(|t| parse_builtin(t)).collect()
}

pub fn builtin_formats() -> Vec<Model> {
    BUILTIN_FORMATS.iter().map(|t| parse_builtin(t)).collect()
}

/// The built-in model of the user's keyboard.
pub fn default_model() -> Model {
    builtin_models().into_iter().find(|m| m.id == DEFAULT_MODEL).expect("default model")
}

/// Model files in `dir` (a contributor's own keyboards). Bad files are skipped with a reason.
pub fn load_dir(dir: &Path) -> (Vec<Model>, Vec<String>) {
    let mut models = Vec::new();
    let mut problems = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return (models, problems) };
    for path in entries.flatten().map(|e| e.path()) {
        if path.extension().is_none_or(|x| x != "json") {
            continue;
        }
        let parsed = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            // Editors on Windows may start the file with a byte order mark.
            .and_then(|t| serde_json::from_str::<Model>(t.trim_start_matches('\u{feff}')).map_err(|e| e.to_string()))
            .and_then(|m| m.validate().map(|()| m));
        match parsed {
            Ok(mut m) if !m.is_format() => {
                m.verified = false;
                models.push(m);
            }
            Ok(_) => problems.push(format!("{}: a model needs \"usb\" and \"driver\"", path.display())),
            Err(e) => problems.push(format!("{}: {e}", path.display())),
        }
    }
    (models, problems)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_files_are_valid() {
        assert_eq!(builtin_models().len(), BUILTIN_MODELS.len());
        assert_eq!(builtin_formats().len(), BUILTIN_FORMATS.len());
        assert_eq!(default_model().keys.len(), 105);
    }

    #[test]
    fn ansi_shape_matches_the_ansi_format() {
        let iso = default_model();
        let ansi = iso.as_ansi().expect("the default model is ISO");
        ansi.validate().unwrap();
        assert_eq!(ansi.keys.len(), iso.keys.len() - 1);
        assert!(ansi.as_ansi().is_none());
        let key = |m: &Model, id: &str| m.keys.iter().find(|k| k.id == id).cloned();
        assert!(key(&ansi, "NonUsBackslash").is_none() && key(&ansi, "NonUsHash").is_none());
        // Same place and size as the drawing of a full-size ANSI keyboard; slots stay the model's.
        let format = builtin_formats().into_iter().find(|f| f.id == "format_fullsize_ansi").unwrap();
        for id in ["Backslash", "Enter", "ShiftL"] {
            let (a, f) = (key(&ansi, id).unwrap(), key(&format, id).unwrap());
            assert_eq!((a.x, a.y, a.w, a.h), (f.x, f.y, f.w, f.h), "{id}");
        }
        assert_eq!(key(&ansi, "Backslash").unwrap().slot, key(&iso, "NonUsHash").unwrap().slot);
        assert_eq!(key(&ansi, "Enter").unwrap().slot, key(&iso, "Enter").unwrap().slot);
    }

    #[test]
    fn formats_have_the_usual_keys() {
        for f in builtin_formats() {
            let has = |id: &str| f.keys.iter().any(|k| k.id == id);
            assert!(has("Space") && has("Enter") && has("ShiftL"), "{}", f.id);
            assert_eq!(has("NonUsBackslash"), f.standard == "iso", "{}", f.id);
        }
    }
}
