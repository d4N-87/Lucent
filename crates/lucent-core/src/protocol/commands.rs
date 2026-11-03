use super::modes::ModeConfig;
use super::packet::GmmkPacket;

/// Config offsets (cmd 0x05 read / 0x06 write).
pub const CFG_MODE: u16 = 0x00;
pub const CFG_POLLING_RATE: u16 = 0x0F;
/// Set to 1 by the official app before every per-key color upload. Meaning not yet known; it is
/// not a way to keep colors out of flash (tested October 2026: they survive unplugging with 00 too).
pub const CFG_UNKNOWN_13: u16 = 0x13;

#[derive(Debug, Clone)]
pub enum Command {
    Begin,
    End,
    SetMode(ModeConfig),
    WriteConfig { offset: u16, data: Vec<u8> },
    WriteColorData { offset: u16, data: Vec<u8> },
    /// Any write command with its data at `offset` (0x08 keymap, 0x0A macros).
    WriteBlock { cmd: u8, offset: u16, data: Vec<u8> },
}

impl Command {
    pub fn to_packet(&self) -> GmmkPacket {
        match self {
            Command::Begin => GmmkPacket::begin(),
            Command::End => GmmkPacket::end(),
            Command::SetMode(cfg) => {
                let payload = [
                    cfg.effect as u8,
                    cfg.brightness.min(4),
                    cfg.speed.min(3),
                    if cfg.direction != 0 { 0xFF } else { 0x00 },
                    if cfg.random_color { 1 } else { 0 },
                    cfg.r,
                    cfg.g,
                    cfg.b,
                ];
                GmmkPacket::with_command(0x06, CFG_MODE, &payload)
            }
            Command::WriteConfig { offset, data } => GmmkPacket::with_command(0x06, *offset, data),
            Command::WriteColorData { offset, data } => {
                GmmkPacket::with_color_chunk(data, *offset)
            }
            Command::WriteBlock { cmd, offset, data } => GmmkPacket::with_command(*cmd, *offset, data),
        }
    }
}
