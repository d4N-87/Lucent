//! What the app needs from a keyboard, independent of brand and firmware.
//!
//! A driver opens one kind of keyboard and describes what it can do (`Capabilities`).
//! The app and the MCP tools build their controls from that description instead of
//! assuming a model, so a new driver (QMK, another brand) needs no UI changes.

use serde::{Deserialize, Serialize};

/// Identity of the connected keyboard.
#[derive(Debug, Clone, Serialize)]
pub struct DeviceInfo {
    /// Product name, e.g. "GMMK full-size".
    pub model: String,
    /// Driver and firmware family, e.g. "GMMK v1 (stock Sonix firmware)".
    pub driver: String,
    /// Id of the model that draws it, e.g. "gmmk_v1_fullsize_iso_it".
    pub layout: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Capabilities {
    pub per_key_rgb: bool,
    pub led_count: usize,
    /// Number of hardware brightness steps; values go from 0 to `brightness_levels - 1`.
    pub brightness_levels: u8,
    /// Hardware effects the keyboard runs on its own.
    pub effects: Vec<EffectInfo>,
    /// Typical time to upload every per-key color, in ms (limits software animations).
    pub per_key_upload_ms: u32,
    /// USB report rates the keyboard accepts, in Hz, slowest first; empty when it can't be changed.
    pub report_rates: Vec<u16>,
    /// `None` when key functions can't be changed.
    pub keymap: Option<KeymapSupport>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EffectInfo {
    /// Stable id shared across drivers when the effect is the same idea (`wave`, `breathing`...).
    pub id: &'static str,
    /// Name used by the manufacturer, shown when the app has no translation.
    pub name: &'static str,
    /// Number of speed steps (0 when the effect has no speed); values go from 0 to `speed_levels - 1`.
    pub speed_levels: u8,
    /// Directions the effect accepts; empty when it has none.
    pub directions: &'static [Direction],
    pub color: ColorSupport,
    /// Parameters checked on real hardware; otherwise they are a best guess.
    pub verified: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorSupport {
    None,
    Single,
    /// One chosen color, or colors picked by the keyboard.
    SingleOrRandom,
}

/// A request to run a hardware effect. Parameters the effect does not support are ignored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectSettings {
    /// `EffectInfo::id`.
    pub effect: String,
    pub brightness: u8,
    /// 0 is the slowest.
    #[serde(default)]
    pub speed: u8,
    #[serde(default)]
    pub direction: Option<Direction>,
    /// `None` asks for random colors when the effect supports them.
    #[serde(default)]
    pub color: Option<[u8; 3]>,
}

/// What a key does. Codes are USB HID usages, the same on every keyboard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum KeyAction {
    /// A keyboard key (page 0x07), modifiers included (0xE0 left Ctrl .. 0xE7 right GUI).
    Key { code: u8 },
    /// A media or system key (consumer page 0x0C), e.g. 0xE9 volume up.
    Media { code: u16 },
    /// Plays the macro at this position in `KeyConfig::macros`.
    Macro { index: u8 },
    /// The key does nothing.
    Disabled,
    /// The Fn key, which reaches the keyboard's own second layer. Never assigned by the app.
    Fn,
    /// A code the app does not understand, kept as read.
    Raw { bytes: Vec<u8> },
}

/// One step of a macro: a key goes down or up, then the macro waits `delay_ms`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacroStep {
    /// USB HID usage of the key (page 0x07).
    pub code: u8,
    pub pressed: bool,
    pub delay_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Macro {
    pub name: String,
    /// How many times it plays on each press (at least 1).
    pub repeat: u8,
    pub steps: Vec<MacroStep>,
}

/// Functions of the base layer and the macros stored in the keyboard.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyConfig {
    /// Every key slot with a physical key, by slot.
    pub keys: std::collections::BTreeMap<usize, KeyAction>,
    pub macros: Vec<Macro>,
}

/// How a keyboard stores key functions and macros.
#[derive(Debug, Clone, Serialize)]
pub struct KeymapSupport {
    /// Macros are stored in the keyboard and work without the app.
    pub macros: bool,
    pub max_macros: usize,
    /// Most steps one macro can have.
    pub max_macro_steps: usize,
    /// Longest pause between two steps.
    pub max_delay_ms: u32,
    /// Pauses are rounded to this many ms.
    pub delay_step_ms: u32,
    /// The model was checked on the real keyboard. False for models from the `models` folder:
    /// key writes then wait for the user's OK.
    pub verified: bool,
}

pub trait KeyboardDriver: Send {
    fn info(&self) -> &DeviceInfo;
    fn capabilities(&self) -> &Capabilities;
    /// Uploads one color per LED slot and switches to per-key lighting.
    fn set_per_key(&self, colors: &[[u8; 3]], brightness: u8) -> Result<(), String>;
    /// Switches to a hardware effect.
    fn set_effect(&self, settings: &EffectSettings) -> Result<(), String>;
    /// Current USB report rate in Hz.
    fn report_rate(&self) -> Result<u16, String> {
        Err("This keyboard's report rate can't be read".into())
    }
    /// Sets the USB report rate, one of `Capabilities::report_rates`. The keyboard keeps it.
    fn set_report_rate(&self, hz: u16) -> Result<(), String> {
        let _ = hz;
        Err("This keyboard's report rate can't be changed".into())
    }
    /// Key functions and macros as stored in the keyboard.
    fn read_keys(&self) -> Result<KeyConfig, String> {
        Err("This keyboard's key functions can't be read".into())
    }
    /// Factory function of every key, by slot.
    fn default_keys(&self) -> std::collections::BTreeMap<usize, KeyAction> {
        Default::default()
    }
    /// Writes key functions and macros. Keys missing from `config` get their factory function.
    fn write_keys(&self, config: &KeyConfig) -> Result<(), String> {
        let _ = config;
        Err("This keyboard's key functions can't be changed".into())
    }
    /// Raw keymap and macro memory, saved as a backup before the first write.
    fn backup_keys(&self) -> Result<Vec<(String, Vec<u8>)>, String> {
        Err("This keyboard's key functions can't be read".into())
    }
}
