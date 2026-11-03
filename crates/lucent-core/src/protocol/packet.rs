pub const PACKET_SIZE: usize = 64;
pub const REPORT_ID: u8 = 0x04;
/// Max payload bytes per packet (0x38).
pub const MAX_DATA: usize = 56;
/// The official app writes per-key colors in 54-byte (18 LED) chunks.
pub const COLOR_CHUNK: usize = 54;

#[derive(Debug, Clone)]
pub struct GmmkPacket {
    pub raw: [u8; PACKET_SIZE],
}

impl Default for GmmkPacket {
    fn default() -> Self {
        Self::new()
    }
}

impl GmmkPacket {
    /// Creates a zeroed packet initialized with the Report ID.
    pub fn new() -> Self {
        let mut packet = Self {
            raw: [0u8; PACKET_SIZE],
        };
        packet.raw[0] = REPORT_ID;
        packet
    }

    /// Computes and writes the 16-bit checksum at bytes 1 and 2.
    /// Checksum is the sum of bytes [3..64].
    pub fn compute_checksum(&mut self) {
        let mut sum: u16 = 0;
        for &b in &self.raw[3..PACKET_SIZE] {
            sum = sum.wrapping_add(b as u16);
        }
        self.raw[1] = (sum & 0xFF) as u8;
        self.raw[2] = (sum >> 8) as u8;
    }

    /// Assembles a command packet: [04, sum_lo, sum_hi, cmd, len, off_lo, off_hi, 00, data...].
    /// `data.len()` is used as the length byte (max 56).
    pub fn with_command(cmd: u8, offset: u16, data: &[u8]) -> Self {
        let mut pkt = Self::new();
        let size = data.len().min(MAX_DATA);
        pkt.raw[3] = cmd;
        pkt.raw[4] = size as u8;
        pkt.raw[5] = (offset & 0x00FF) as u8;
        pkt.raw[6] = (offset >> 8) as u8;
        pkt.raw[8..8 + size].copy_from_slice(&data[..size]);

        pkt.compute_checksum();
        pkt
    }

    /// Assembles a read request: same header as a write, with `len` bytes requested and no data.
    pub fn read_request(cmd: u8, offset: u16, len: u8) -> Self {
        let mut pkt = Self::new();
        pkt.raw[3] = cmd;
        pkt.raw[4] = len.min(MAX_DATA as u8);
        pkt.raw[5] = (offset & 0x00FF) as u8;
        pkt.raw[6] = (offset >> 8) as u8;

        pkt.compute_checksum();
        pkt
    }

    /// Assembles a custom color data chunk packet (cmd 0x11, up to 54 bytes per chunk).
    pub fn with_color_chunk(data: &[u8], offset: u16) -> Self {
        Self::with_command(0x11, offset, &data[..data.len().min(COLOR_CHUNK)])
    }

    pub fn begin() -> Self {
        let mut pkt = Self::new();
        pkt.raw[1] = 0x01;
        pkt.raw[2] = 0x00;
        pkt.raw[3] = 0x01;
        pkt
    }

    pub fn end() -> Self {
        let mut pkt = Self::new();
        pkt.raw[1] = 0x02;
        pkt.raw[2] = 0x00;
        pkt.raw[3] = 0x02;
        pkt
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.raw
    }
}
