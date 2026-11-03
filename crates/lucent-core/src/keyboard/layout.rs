use serde::{Deserialize, Serialize};

/// The color buffer has 144 slots (432 bytes); only the first 126 map to keymap slots.
pub const TOTAL_LEDS: usize = 144;
/// Keymap slots (cmd 0x0F). LED index == keymap slot index.
pub const KEY_SLOTS: usize = 126;

// GMMK 1 slot indices, from the official app's keymap read (see docs/protocollo.md)
pub const KEY_ESC: usize = 1;
pub const KEY_Q: usize = 36;
pub const KEY_W: usize = 37;
pub const KEY_E: usize = 38;
pub const KEY_R: usize = 39;

pub const KEY_A: usize = 53;
pub const KEY_S: usize = 54;
pub const KEY_D: usize = 55;
pub const KEY_F: usize = 56;

pub const KEY_SPACE: usize = 89;

pub const WASD_KEYS: [usize; 4] = [KEY_W, KEY_A, KEY_S, KEY_D];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorBuffer {
    pub leds: Vec<[u8; 3]>,
}

impl Default for ColorBuffer {
    fn default() -> Self {
        Self {
            leds: vec![[0, 0, 0]; TOTAL_LEDS],
        }
    }
}

impl ColorBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_all(&mut self, rgb: [u8; 3]) {
        self.leds.fill(rgb);
    }

    pub fn set_key(&mut self, index: usize, rgb: [u8; 3]) {
        if index < self.leds.len() {
            self.leds[index] = rgb;
        }
    }

    pub fn set_keys(&mut self, indices: &[usize], rgb: [u8; 3]) {
        for &idx in indices {
            self.set_key(idx, rgb);
        }
    }

    pub fn set_wasd(&mut self, rgb: [u8; 3]) {
        self.set_keys(&WASD_KEYS, rgb);
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = vec![0u8; TOTAL_LEDS * 3];
        for (dst, led) in buf.chunks_mut(3).zip(&self.leds) {
            dst.copy_from_slice(led);
        }
        buf
    }

    /// Linear gradient across sequential keys
    pub fn apply_linear_gradient(&mut self, start: [u8; 3], end: [u8; 3]) {
        let total = KEY_SLOTS.min(self.leds.len());
        for i in 0..total {
            let t = i as f32 / (total - 1) as f32;
            let r = (start[0] as f32 * (1.0 - t) + end[0] as f32 * t).round() as u8;
            let g = (start[1] as f32 * (1.0 - t) + end[1] as f32 * t).round() as u8;
            let b = (start[2] as f32 * (1.0 - t) + end[2] as f32 * t).round() as u8;
            self.set_key(i, [r, g, b]);
        }
    }
}
