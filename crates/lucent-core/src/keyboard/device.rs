use crate::keyboard::layout::ColorBuffer;
use crate::protocol::commands::{CFG_UNKNOWN_13, Command};
use crate::protocol::modes::{HardwareEffect, ModeConfig};
use crate::protocol::packet::{COLOR_CHUNK, GmmkPacket, MAX_DATA};
use hidapi::{HidApi, HidDevice};
use std::time::{Duration, Instant};

pub const GMMK_VID: u16 = 0x0C45;
pub const GMMK_PID: u16 = 0x652F;

pub struct GmmkDevice {
    device: HidDevice,
    info: DeviceMetadata,
}

#[derive(Debug, Clone)]
pub struct DeviceMetadata {
    pub path: String,
    pub vid: u16,
    pub pid: u16,
    pub manufacturer: String,
    pub product: String,
    pub interface_number: i32,
    pub usage_page: u16,
    pub usage: u16,
}

impl GmmkDevice {
    /// Every HID interface of the keyboard with these USB ids.
    pub fn find_devices(api: &HidApi, vid: u16, pid: u16) -> Vec<DeviceMetadata> {
        let mut list = Vec::new();
        for dev in api.device_list() {
            if dev.vendor_id() == vid && dev.product_id() == pid {
                list.push(DeviceMetadata {
                    path: dev.path().to_string_lossy().to_string(),
                    vid,
                    pid,
                    manufacturer: dev.manufacturer_string().unwrap_or("Unknown").to_string(),
                    product: dev.product_string().unwrap_or("GMMK Keyboard").to_string(),
                    interface_number: dev.interface_number(),
                    usage_page: dev.usage_page(),
                    usage: dev.usage(),
                });
            }
        }
        list
    }

    /// Opens the GMMK vendor control interface.
    /// Prioritizes the vendor-defined collection (usage_page >= 0xFF00, e.g. 0xFF1C)
    /// which allows custom vendor packet exchange on Windows.
    pub fn open(api: &HidApi) -> Result<Self, String> {
        Self::open_ids(api, GMMK_VID, GMMK_PID)
    }

    /// Like `open`, for a keyboard of the same family with other USB ids.
    pub fn open_ids(api: &HidApi, vid: u16, pid: u16) -> Result<Self, String> {
        let candidates = Self::find_devices(api, vid, pid);
        if candidates.is_empty() {
            return Err(format!("No keyboard with VID 0x{vid:04X}, PID 0x{pid:04X} found on USB"));
        }

        let target = candidates
            .iter()
            .find(|c| c.usage_page >= 0xFF00)
            .or_else(|| candidates.iter().find(|c| c.interface_number == 1))
            .unwrap_or(&candidates[0]);

        let c_path = match std::ffi::CString::new(target.path.as_str()) {
            Ok(p) => p,
            Err(_) => return Err("Invalid HID device path".to_string()),
        };

        let device = api
            .open_path(&c_path)
            .map_err(|e| format!("Cannot open the HID device at {}: {}", target.path, e))?;

        log::info!(
            "Aperta {} {} (interfaccia {}, usage page 0x{:04X}) su {}",
            target.manufacturer,
            target.product,
            target.interface_number,
            target.usage_page,
            target.path
        );
        Ok(Self {
            device,
            info: target.clone(),
        })
    }

    pub fn metadata(&self) -> &DeviceMetadata {
        &self.info
    }

    /// Sends a low-level packet and waits for the keyboard's response,
    /// which always echoes the command byte (byte 3).
    ///
    /// Every open handle receives every response, so another program talking to the keyboard
    /// (e.g. lucent-dev) leaves stray reports in our queue: drop them before writing, and skip
    /// responses to other commands while waiting.
    pub fn send_packet(&self, packet: &GmmkPacket) -> Result<[u8; 64], String> {
        self.exchange(packet, 1)
    }

    /// Like `send_packet` for a read: the answer must also echo the length and offset asked
    /// (bytes 4-6), so another program's read of the same command can't be taken for ours.
    pub fn read_packet(&self, packet: &GmmkPacket) -> Result<[u8; 64], String> {
        self.exchange(packet, 4)
    }

    /// Sends `packet` and waits for the answer whose bytes 3.. match it for `echo` bytes.
    fn exchange(&self, packet: &GmmkPacket, echo: usize) -> Result<[u8; 64], String> {
        let bytes = packet.as_bytes();
        let mut response = [0u8; 64];
        while self.device.read_timeout(&mut response, 0).unwrap_or(0) > 0 {
            log::warn!("Scartata risposta in coda non nostra: {}", hex(&response[..8]));
        }

        log::trace!("TX {}", hex(bytes));
        self.device
            .write(bytes)
            .map_err(|e| format!("HID write failed: {}", e))?;

        let deadline = Instant::now() + Duration::from_millis(500);
        loop {
            let left = deadline.saturating_duration_since(Instant::now()).as_millis() as i32;
            let n = self
                .device
                .read_timeout(&mut response, left)
                .map_err(|e| format!("HID read failed: {}", e))?;
            if n == 0 {
                return Err(format!("No answer to command 0x{:02X}", bytes[3]));
            }
            log::trace!("RX {}", hex(&response));
            if response[3..3 + echo] == bytes[3..3 + echo] {
                return Ok(response);
            }
            log::warn!(
                "Ignorata risposta {} mentre aspettavo {}",
                hex(&response[3..7]),
                hex(&bytes[3..3 + echo])
            );
        }
    }

    /// Executes a high-level command.
    pub fn execute(&self, cmd: &Command) -> Result<(), String> {
        let pkt = cmd.to_packet();
        self.send_packet(&pkt)?;
        Ok(())
    }

    /// Reads `len` bytes of the current config (cmd 0x05). Read-only.
    pub fn read_config(&self, len: usize) -> Result<Vec<u8>, String> {
        self.read_block(0x05, len)
    }

    /// Reads `len` bytes with a read command (0x05 config, 0x07 keymap layers, 0x09 macros,
    /// 0x0F base keymap), in 56-byte requests. Read-only.
    pub fn read_block(&self, cmd: u8, len: usize) -> Result<Vec<u8>, String> {
        let mut out = Vec::with_capacity(len);
        while out.len() < len {
            let n = (len - out.len()).min(MAX_DATA);
            let resp = self.read_packet(&GmmkPacket::read_request(cmd, out.len() as u16, n as u8))?;
            out.extend_from_slice(&resp[8..8 + n]);
        }
        Ok(out)
    }

    /// Effect codes the firmware lists in its device info (cmd 0x03), in menu order. Read-only.
    ///
    /// The block is writable (cmd 0x04, which the official app sends on exit), and the official
    /// app has been seen leaving other bytes in it: without the 55 aa ff 02 signature and our
    /// USB ids it is not the list, so this fails and the caller offers every known effect.
    pub fn read_supported_effects(&self) -> Result<Vec<u8>, String> {
        let resp = self.read_packet(&GmmkPacket::read_request(0x03, 0, 0x2C))?;
        // Data: 55 aa ff 02 | VID | PID | 4 bytes | 4 bytes | effect codes ... ff
        let data = &resp[8..];
        let ids = [self.info.vid.to_le_bytes(), self.info.pid.to_le_bytes()].concat();
        if data[..4] != [0x55, 0xAA, 0xFF, 0x02] || data[4..8] != ids[..] {
            return Err(format!("the device info block holds other data ({})", hex(&data[..16])));
        }
        let codes: Vec<u8> = data[16..].iter().copied().take_while(|&c| c != 0xFF).collect();
        log::info!("Effetti dichiarati dal firmware: {}", hex(&codes));
        Ok(codes)
    }

    /// Writes raw config bytes at `offset` (cmd 0x06) in its own transaction.
    pub fn write_config(&self, offset: u16, data: &[u8]) -> Result<(), String> {
        self.transaction(&[Command::WriteConfig { offset, data: data.to_vec() }])
    }

    /// Writes each `(cmd, data)` block in 56-byte packets, all in one transaction (keymap and
    /// macros are written together, as the official app does).
    pub fn write_blocks(&self, blocks: &[(u8, &[u8])]) -> Result<(), String> {
        let cmds: Vec<Command> = blocks
            .iter()
            .flat_map(|&(cmd, data)| {
                data.chunks(MAX_DATA).enumerate().map(move |(i, chunk)| Command::WriteBlock {
                    cmd,
                    offset: (i * MAX_DATA) as u16,
                    data: chunk.to_vec(),
                })
            })
            .collect();
        self.transaction(&cmds)
    }

    /// Runs `cmds` inside a Begin/End transaction, as the official app does. If a command fails,
    /// End is still sent, so the keyboard isn't left waiting inside a transaction.
    fn transaction(&self, cmds: &[Command]) -> Result<(), String> {
        self.execute(&Command::Begin)?;
        let result = cmds.iter().try_for_each(|cmd| self.execute(cmd));
        let end = self.execute(&Command::End);
        if let (Err(_), Err(e)) = (&result, &end) {
            log::warn!("Transazione interrotta, anche la chiusura è fallita: {e}");
        }
        result.and(end)
    }

    /// Sets a hardware lighting effect.
    pub fn set_hardware_mode(&self, config: &ModeConfig) -> Result<(), String> {
        self.transaction(&[Command::SetMode(*config)])
    }

    /// Uploads per-key colors (all 432 bytes) and switches to the custom effect.
    /// Mirrors the official app: config 0x13 = 1, then the color data in its own transaction.
    pub fn set_custom_colors(&self, buffer: &ColorBuffer, brightness: u8) -> Result<(), String> {
        self.transaction(&[Command::WriteConfig {
            offset: CFG_UNKNOWN_13,
            data: vec![0x01],
        }])?;
        self.upload_colors(buffer)?;

        // Speed 3 and a white effect color are what the official app leaves set. With speed 0
        // and black, the firmware drives the per-key LEDs visibly dimmer.
        let custom_cfg = ModeConfig {
            effect: HardwareEffect::Custom,
            brightness: brightness.min(4),
            speed: 3,
            direction: 0,
            random_color: false,
            r: 0xFF,
            g: 0xFF,
            b: 0xFF,
        };
        self.set_hardware_mode(&custom_cfg)
    }

    /// Only the per-key color buffer (command 0x11), nothing else: for experiments.
    pub fn upload_colors(&self, buffer: &ColorBuffer) -> Result<(), String> {
        let chunks: Vec<Command> = buffer
            .to_bytes()
            .chunks(COLOR_CHUNK)
            .enumerate()
            .map(|(i, chunk)| Command::WriteColorData {
                offset: (i * COLOR_CHUNK) as u16,
                data: chunk.to_vec(),
            })
            .collect();
        self.transaction(&chunks)
    }
}

/// Space-separated hex dump, trailing zeros trimmed (packets are mostly padding).
fn hex(bytes: &[u8]) -> String {
    let end = bytes.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
    let mut out: Vec<String> = bytes[..end].iter().map(|b| format!("{b:02x}")).collect();
    if end < bytes.len() {
        out.push(format!("… ({} × 00)", bytes.len() - end));
    }
    out.join(" ")
}
