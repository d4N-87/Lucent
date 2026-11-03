//! Glorious GMMK v1 with its original firmware (Sonix SN32F248B). Protocol: docs/protocollo.md.

use super::gmmk_v1_keys as keys;
use crate::driver::{
    Capabilities, ColorSupport, DeviceInfo, Direction, EffectInfo, EffectSettings, KeyAction, KeyConfig,
    KeyboardDriver, KeymapSupport,
};
use std::collections::BTreeMap;
use crate::keyboard::layout::{ColorBuffer, TOTAL_LEDS};
use crate::keyboard::model::{Model, DEFAULT_MODEL};
use crate::keyboard::GmmkDevice;
use crate::protocol::commands::CFG_POLLING_RATE;
use crate::protocol::modes::{HardwareEffect, ModeConfig};
use hidapi::HidApi;

/// Brightness 0..4 and speed 0..3, as the official app's sliders (capture 02_effetti).
/// On the wire speed 0 is the fastest; `EffectSettings::speed` counts the other way.
const BRIGHTNESS_LEVELS: u8 = 5;
const SPEED_LEVELS: u8 = 4;
/// Wire speed written for effects without one: the official app's default. Wire speed 0
/// dims the per-key mode, so avoid it where the firmware might use it.
const DEFAULT_SPEED: u8 = 3;
/// Config byte 0x0F to USB report rate (capture 03_tasti_macro).
const REPORT_RATES: &[(u8, u16)] = &[(0, 125), (1, 250), (2, 500), (3, 1000)];

const LR: &[Direction] = &[Direction::Left, Direction::Right];
const NO_DIR: &[Direction] = &[];

const fn fx(
    code: HardwareEffect,
    id: &'static str,
    name: &'static str,
    speed_levels: u8,
    directions: &'static [Direction],
    color: ColorSupport,
) -> (HardwareEffect, EffectInfo) {
    (code, EffectInfo { id, name, speed_levels, directions, color, verified: true })
}

use ColorSupport::{None as NoColor, SingleOrRandom as AnyColor};

/// Every effect in the official app's menu order. Direction and color support were checked on
/// the keyboard (October 2026); speed is assumed to work wherever the effect moves.
/// Some effects also speed up or change color with typing: that is firmware behavior, not a setting.
const EFFECTS: &[(HardwareEffect, EffectInfo)] = &[
    fx(HardwareEffect::Wave1, "wave", "Wave #1", SPEED_LEVELS, LR, AnyColor),
    fx(HardwareEffect::Wave2, "wave_2", "Wave #2", SPEED_LEVELS, LR, AnyColor),
    fx(HardwareEffect::SpiralingWave, "spiral", "Spiraling wave", SPEED_LEVELS, LR, AnyColor),
    fx(HardwareEffect::AcidMode, "acid", "Acid Mode", SPEED_LEVELS, NO_DIR, NoColor),
    fx(HardwareEffect::Breathing, "breathing", "Breathing", SPEED_LEVELS, NO_DIR, AnyColor),
    fx(HardwareEffect::NormallyOn, "static", "Normally on", 0, NO_DIR, AnyColor),
    fx(HardwareEffect::RippleGraff, "ripple", "Ripple graff", SPEED_LEVELS, NO_DIR, AnyColor),
    // Lights the pressed key.
    fx(HardwareEffect::PassWithoutTrace, "reactive", "Pass without trace", SPEED_LEVELS, NO_DIR, AnyColor),
    // Lights the pressed key's row.
    fx(HardwareEffect::FastRunWithoutTrace, "reactive_row", "Fast run without trace", SPEED_LEVELS, NO_DIR, AnyColor),
    fx(HardwareEffect::Matrix3, "matrix_3", "Matrix 3", SPEED_LEVELS, NO_DIR, AnyColor),
    fx(HardwareEffect::Matrix2, "matrix_2", "Matrix 2", SPEED_LEVELS, NO_DIR, AnyColor),
    fx(HardwareEffect::Rainbow, "rainbow", "Rainbow", SPEED_LEVELS, NO_DIR, AnyColor),
    fx(HardwareEffect::HeartbeatSensor, "heartbeat", "Heartbeat Sensor", SPEED_LEVELS, NO_DIR, AnyColor),
    fx(HardwareEffect::Kamehameha, "kamehameha", "Kamehameha", SPEED_LEVELS, NO_DIR, AnyColor),
    fx(HardwareEffect::DigitalTimes, "digital_times", "Digital Times", SPEED_LEVELS, NO_DIR, AnyColor),
    fx(HardwareEffect::PingPong, "ping_pong", "Ping Pong", SPEED_LEVELS, NO_DIR, AnyColor),
    fx(HardwareEffect::Surmount, "surmount", "Surmount", SPEED_LEVELS, NO_DIR, NoColor),
    fx(HardwareEffect::GloriousMode, "glorious", "Glorious Mode", SPEED_LEVELS, NO_DIR, AnyColor),
];

pub struct GmmkV1 {
    dev: GmmkDevice,
    info: DeviceInfo,
    caps: Capabilities,
    keymap: keys::Keymap,
}

impl GmmkV1 {
    /// Opens the keyboard `model` describes. Only the built-in model has a known factory keymap;
    /// others read theirs from the keyboard.
    pub fn open(api: &HidApi, model: &Model) -> Result<Self, String> {
        let (vid, pid) = model.usb.as_ref().and_then(|u| u.parse()).ok_or("The model has no USB ids")?;
        let dev = GmmkDevice::open_ids(api, vid, pid)?;
        let factory = if model.id == DEFAULT_MODEL && model.verified {
            keys::FACTORY_LAYER.to_vec()
        } else {
            dev.read_block(0x0F, keys::LAYER_BYTES)?
        };
        let keymap = keys::Keymap::new(model.slots(), factory)?;
        // Offer only what this firmware declares; fall back to the full list if the read fails.
        let effects = match dev.read_supported_effects() {
            Ok(codes) => EFFECTS
                .iter()
                .filter(|(code, _)| codes.contains(&(*code as u8)))
                .map(|(_, info)| info.clone())
                .collect(),
            Err(e) => {
                log::warn!("Elenco effetti non letto ({e}): uso quello completo");
                EFFECTS.iter().map(|(_, info)| info.clone()).collect()
            }
        };
        Ok(Self {
            dev,
            keymap,
            info: DeviceInfo {
                model: model.name.clone(),
                driver: "GMMK v1 (stock Sonix firmware)".into(),
                layout: model.id.clone(),
            },
            caps: Capabilities {
                per_key_rgb: true,
                led_count: TOTAL_LEDS,
                brightness_levels: BRIGHTNESS_LEVELS,
                effects,
                per_key_upload_ms: 150,
                report_rates: REPORT_RATES.iter().map(|&(_, hz)| hz).collect(),
                keymap: Some(KeymapSupport {
                    macros: true,
                    max_macros: keys::MAX_MACROS,
                    max_macro_steps: keys::MAX_MACRO_STEPS,
                    max_delay_ms: 255 * keys::DELAY_UNIT_MS,
                    delay_step_ms: keys::DELAY_UNIT_MS,
                    verified: model.verified,
                }),
            },
        })
    }
}

impl KeyboardDriver for GmmkV1 {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn capabilities(&self) -> &Capabilities {
        &self.caps
    }

    fn read_keys(&self) -> Result<KeyConfig, String> {
        let layer = self.read_layer()?;
        self.keymap.decode(&layer, &self.read_macro_area()?)
    }

    fn default_keys(&self) -> BTreeMap<usize, KeyAction> {
        self.keymap.factory_keys()
    }

    /// Macro area, then the base layer, in one transaction. Slots without a key keep what the
    /// keyboard has; layers 1 and 2 are not touched.
    fn write_keys(&self, config: &KeyConfig) -> Result<(), String> {
        let macros = keys::encode_macros(&config.macros)?;
        let current = self.read_layer()?;
        let layer = self.keymap.encode_layer(&current, &config.keys, config.macros.len())?;
        self.dev.write_blocks(&[(0x0A, &macros), (0x08, &layer)])?;
        log::info!(
            "Tasti scritti: {} tasti modificati, {} macro ({} byte)",
            config.keys.iter().filter(|(s, a)| self.keymap.factory_keys().get(s) != Some(a)).count(),
            config.macros.len(),
            macros.len()
        );
        Ok(())
    }

    fn backup_keys(&self) -> Result<Vec<(String, Vec<u8>)>, String> {
        Ok(vec![
            ("layers".into(), self.dev.read_block(0x07, 3 * keys::LAYER_BYTES)?),
            ("macros".into(), self.read_macro_area()?),
        ])
    }

    fn report_rate(&self) -> Result<u16, String> {
        let config = self.dev.read_config(CFG_POLLING_RATE as usize + 1)?;
        let code = config[CFG_POLLING_RATE as usize];
        REPORT_RATES
            .iter()
            .find(|&&(c, _)| c == code)
            .map(|&(_, hz)| hz)
            .ok_or_else(|| format!("Unknown report rate code 0x{code:02X}"))
    }

    /// One config byte in its own transaction, as the official app writes it.
    fn set_report_rate(&self, hz: u16) -> Result<(), String> {
        let &(code, _) = REPORT_RATES
            .iter()
            .find(|&&(_, h)| h == hz)
            .ok_or_else(|| format!("Unsupported report rate: {hz} Hz"))?;
        self.dev.write_config(CFG_POLLING_RATE, &[code])?;
        log::info!("Report rate impostata a {hz} Hz (codice {code:02x})");
        Ok(())
    }

    fn set_per_key(&self, colors: &[[u8; 3]], brightness: u8) -> Result<(), String> {
        let mut buffer = ColorBuffer::new();
        for (slot, &rgb) in colors.iter().enumerate().take(TOTAL_LEDS) {
            buffer.set_key(slot, rgb);
        }
        self.dev.set_custom_colors(&buffer, brightness)
    }

    fn set_effect(&self, s: &EffectSettings) -> Result<(), String> {
        let (code, info) = EFFECTS
            .iter()
            .find(|(_, info)| info.id == s.effect)
            .ok_or_else(|| format!("Unknown effect: {}", s.effect))?;
        // Every parameter is written as asked, even ones the table says the effect ignores:
        // the firmware ignores what it does not use, and unverified effects need testing.
        let levels = match info.speed_levels {
            0 if !info.verified => SPEED_LEVELS,
            n => n,
        };
        let speed = match levels {
            0 => DEFAULT_SPEED,
            n => n - 1 - s.speed.min(n - 1),
        };
        let random = s.color.is_none();
        let [r, g, b] = s.color.unwrap_or([0xFF, 0xFF, 0xFF]);
        let config = ModeConfig {
            effect: *code,
            brightness: s.brightness.min(BRIGHTNESS_LEVELS - 1),
            speed,
            // Checked on the keyboard: 0xFF moves left, 0x00 right.
            direction: if s.direction == Some(Direction::Left) { 1 } else { 0 },
            random_color: random,
            r,
            g,
            b,
        };
        log::debug!("Effetto {} ({:?}): {:?}", info.id, code, config);
        self.dev.set_hardware_mode(&config)
    }
}

impl GmmkV1 {
    /// The base layer as written (first third of 0x07). Read 0x0F keeps answering with the
    /// factory keymap after a write (October 2026), so it can't be used here.
    fn read_layer(&self) -> Result<Vec<u8>, String> {
        self.dev.read_block(0x07, keys::LAYER_BYTES)
    }

    /// A damaged header could ask for up to 64 KB (over a thousand reads): refuse sizes the
    /// keyboard can't hold.
    fn read_macro_area(&self) -> Result<Vec<u8>, String> {
        let len = keys::macro_area_len(&self.dev.read_block(0x09, 4)?)?;
        if len > keys::MAX_MACRO_AREA_READ {
            return Err(format!("The macro area header says {len} bytes, more than the keyboard can hold"));
        }
        self.dev.read_block(0x09, len.max(16))
    }
}
