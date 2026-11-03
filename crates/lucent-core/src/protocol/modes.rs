use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum HardwareEffect {
    Wave1 = 0x01,
    Wave2 = 0x02,
    SpiralingWave = 0x03,
    AcidMode = 0x04,
    Breathing = 0x05,
    NormallyOn = 0x06, // Static
    PassWithoutTrace = 0x07, // Reactive single
    RippleGraff = 0x08,
    FastRunWithoutTrace = 0x09,
    Matrix2 = 0x0A,
    Matrix3 = 0x0B,
    Rainbow = 0x0C,
    HeartbeatSensor = 0x0D,
    Kamehameha = 0x0E,
    DigitalTimes = 0x0F,
    PingPong = 0x10,
    Surmount = 0x11,
    GloriousMode = 0x12,
    Custom = 0x14,
}

impl HardwareEffect {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Wave1 => "Wave #1",
            Self::Wave2 => "Wave #2",
            Self::SpiralingWave => "Spiraling Wave",
            Self::AcidMode => "Acid Mode",
            Self::Breathing => "Breathing",
            Self::NormallyOn => "Static (Normally On)",
            Self::PassWithoutTrace => "Pass Without Trace",
            Self::RippleGraff => "Ripple",
            Self::FastRunWithoutTrace => "Fast Run",
            Self::Matrix2 => "Matrix #2",
            Self::Matrix3 => "Matrix #3",
            Self::Rainbow => "Rainbow Wave",
            Self::HeartbeatSensor => "Heartbeat Sensor",
            Self::Kamehameha => "Kamehameha",
            Self::DigitalTimes => "Digital Times",
            Self::PingPong => "Ping Pong",
            Self::Surmount => "Surmount",
            Self::GloriousMode => "Glorious Mode",
            Self::Custom => "Custom Per-Key RGB",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum PollingRate {
    Hz125 = 0,
    Hz250 = 1,
    Hz500 = 2,
    Hz1000 = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModeConfig {
    pub effect: HardwareEffect,
    pub brightness: u8, // 0..4
    pub speed: u8,      // 0..3
    pub direction: u8,  // 0 or nonzero; sent as 0x00 / 0xFF
    pub random_color: bool,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Default for ModeConfig {
    fn default() -> Self {
        Self {
            effect: HardwareEffect::GloriousMode,
            brightness: 4,
            speed: 3,
            direction: 0,
            random_color: true,
            r: 255,
            g: 255,
            b: 255,
        }
    }
}
