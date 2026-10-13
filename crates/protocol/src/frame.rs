//! GAIA SPP framing: `FF <ver> <flags> <len[1|2]> <vendor:2> <id:2> <payload> [chk]`.

use std::fmt;

pub const SOF: u8 = 0xFF;
/// Protocol version byte used for normal GAIA v3 control traffic.
pub const VERSION_GAIA3: u8 = 0x03;
const FLAG_CHECKSUM: u8 = 0x01;
const FLAG_LONG_LENGTH: u8 = 0x02;
/// Largest payload that fits the 1-byte length field. Longer payloads need the v4 transport, which we never negotiate.
pub const MAX_SHORT_PAYLOAD: usize = 254;
/// Bytes kept while waiting for a frame end before we give up and resync.
const MAX_BUFFER: usize = 4096;

/// Packet type, encoded in bits 8..7 of the 16-bit command id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Command,
    Notification,
    Response,
    Error,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Packet {
    pub vendor: u16,
    pub id: u16,
    pub payload: Vec<u8>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FrameError {
    #[error("payload of {0} bytes does not fit a short GAIA frame")]
    TooLong(usize),
}

impl Packet {
    pub fn new(vendor: u16, id: u16, payload: impl Into<Vec<u8>>) -> Self {
        Self {
            vendor,
            id,
            payload: payload.into(),
        }
    }

    pub fn kind(&self) -> Kind {
        match (self.id >> 7) & 0b11 {
            0 => Kind::Command,
            1 => Kind::Notification,
            2 => Kind::Response,
            _ => Kind::Error,
        }
    }

    /// GAIA v3 feature id (top 7 bits).
    pub fn feature(&self) -> u8 {
        (self.id >> 9) as u8
    }

    /// The command id with the packet-type bits cleared, i.e. the id of the command this packet relates to.
    pub fn command_id(&self) -> u16 {
        self.id & !0x0180
    }

    /// Error code carried by an error packet.
    pub fn error_code(&self) -> Option<u8> {
        (self.kind() == Kind::Error).then(|| self.payload.first().copied().unwrap_or(0))
    }

    pub fn encode(&self) -> Result<Vec<u8>, FrameError> {
        if self.payload.len() > MAX_SHORT_PAYLOAD {
            return Err(FrameError::TooLong(self.payload.len()));
        }
        let mut out = Vec::with_capacity(8 + self.payload.len());
        out.extend_from_slice(&[SOF, VERSION_GAIA3, 0x00, self.payload.len() as u8]);
        out.extend_from_slice(&self.vendor.to_be_bytes());
        out.extend_from_slice(&self.id.to_be_bytes());
        out.extend_from_slice(&self.payload);
        Ok(out)
    }
}

impl fmt::Debug for Packet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Packet({:04X}:{:04X} {:?} [{}])",
            self.vendor,
            self.id,
            self.kind(),
            hex(&self.payload)
        )
    }
}

pub fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        s.push_str(&format!("{b:02X}"));
    }
    s
}

/// Incremental deframer for the RFCOMM byte stream. Resynchronises on `0xFF` after garbage.
#[derive(Default)]
pub struct Deframer {
    buf: Vec<u8>,
}

enum Parse {
    Frame(Packet, usize),
    NeedMore,
    Invalid,
}

impl Deframer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed received bytes; returns every complete packet found.
    pub fn push(&mut self, data: &[u8]) -> Vec<Packet> {
        self.buf.extend_from_slice(data);
        let mut out = Vec::new();
        loop {
            match self.buf.iter().position(|&b| b == SOF) {
                Some(0) => {}
                Some(n) => {
                    self.buf.drain(..n);
                }
                None => {
                    self.buf.clear();
                    break;
                }
            }
            match parse(&self.buf) {
                Parse::Frame(p, used) => {
                    self.buf.drain(..used);
                    out.push(p);
                }
                Parse::NeedMore => {
                    if self.buf.len() > MAX_BUFFER {
                        self.buf.drain(..1);
                        continue;
                    }
                    break;
                }
                Parse::Invalid => {
                    self.buf.drain(..1);
                }
            }
        }
        out
    }
}

fn parse(b: &[u8]) -> Parse {
    if b.len() < 4 {
        return Parse::NeedMore;
    }
    let version = b[1];
    let flags = b[2];
    if !matches!(version, 1 | 3 | 4) || flags & !(FLAG_CHECKSUM | FLAG_LONG_LENGTH) != 0 {
        return Parse::Invalid;
    }
    let long = flags & FLAG_LONG_LENGTH != 0;
    if long && version != 4 {
        return Parse::Invalid;
    }
    let (len, header) = if long {
        if b.len() < 5 {
            return Parse::NeedMore;
        }
        (u16::from_be_bytes([b[3], b[4]]) as usize, 5)
    } else {
        (b[3] as usize, 4)
    };
    let checksum = flags & FLAG_CHECKSUM != 0;
    let total = header + 4 + len + usize::from(checksum);
    if b.len() < total {
        return Parse::NeedMore;
    }
    if checksum {
        let end = total - 1;
        let all = b[..end].iter().fold(0u8, |a, x| a ^ x);
        let body = b[header..end].iter().fold(0u8, |a, x| a ^ x);
        if b[end] != all && b[end] != body {
            return Parse::Invalid;
        }
    }
    let vendor = u16::from_be_bytes([b[header], b[header + 1]]);
    let id = u16::from_be_bytes([b[header + 2], b[header + 3]]);
    let payload = b[header + 4..header + 4 + len].to_vec();
    Parse::Frame(
        Packet {
            vendor,
            id,
            payload,
        },
        total,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(s: &str) -> Vec<u8> {
        s.split_whitespace()
            .map(|h| u8::from_str_radix(h, 16).unwrap())
            .collect()
    }

    #[test]
    fn encodes_reference_frames() {
        assert_eq!(
            Packet::new(0x0495, 0x0603, []).encode().unwrap(),
            bytes("FF 03 00 00 04 95 06 03")
        );
        assert_eq!(
            Packet::new(0x0495, 0x1A00, [3, 1]).encode().unwrap(),
            bytes("FF 03 00 02 04 95 1A 00 03 01")
        );
        assert_eq!(
            Packet::new(0x0495, 0x0007, [13]).encode().unwrap(),
            bytes("FF 03 00 01 04 95 00 07 0D")
        );
        assert_eq!(
            Packet::new(0x0495, 0x0001, vec![0; 255]).encode(),
            Err(FrameError::TooLong(255))
        );
    }

    #[test]
    fn decodes_real_m4_responses() {
        let mut d = Deframer::new();
        let p = d.push(&bytes("FF 03 00 06 04 95 13 01 00 03 00 26 00 03"));
        assert_eq!(p, vec![Packet::new(0x0495, 0x1301, [0, 3, 0, 0x26, 0, 3])]);
        assert_eq!(p[0].kind(), Kind::Response);
        assert_eq!(p[0].command_id(), 0x1201);
        assert_eq!(p[0].feature(), 9);

        let e = d.push(&bytes("FF 03 00 01 04 95 11 87 01"));
        assert_eq!(e[0].kind(), Kind::Error);
        assert_eq!(e[0].command_id(), 0x1007);
        assert_eq!(e[0].error_code(), Some(1));

        let n = d.push(&bytes("FF 03 00 01 04 95 1A 83 32"));
        assert_eq!(n[0].kind(), Kind::Notification);
        assert_eq!(n[0].command_id(), 0x1A03);
    }

    #[test]
    fn handles_split_merged_and_garbage() {
        let mut d = Deframer::new();
        let a = bytes("FF 03 00 01 04 95 07 03 64");
        let b = bytes("FF 03 00 02 04 95 07 02 00 00");
        let mut stream = bytes("00 12 FF 07 34"); // garbage incl. a bogus SOF
        stream.extend(&a);
        stream.extend(&b);
        let mut got = Vec::new();
        for chunk in stream.chunks(3) {
            got.extend(d.push(chunk));
        }
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].payload, vec![0x64]);
        assert_eq!(got[1].id, 0x0702);
    }

    #[test]
    fn accepts_checksum_and_long_frames() {
        let mut f = bytes("FF 03 01 01 04 95 07 03 64");
        f.push(f.iter().fold(0, |a, x| a ^ x));
        assert_eq!(Deframer::new().push(&f).len(), 1);

        let mut long = vec![0xFF, 0x04, 0x02, 0x01, 0x00, 0x04, 0x95, 0x01, 0x02];
        long.extend(vec![0xAB; 256]);
        let p = Deframer::new().push(&long);
        assert_eq!(p[0].payload.len(), 256);
    }

    #[test]
    fn bad_checksum_is_dropped_and_stream_recovers() {
        let mut d = Deframer::new();
        let mut bad = bytes("FF 03 01 01 04 95 07 03 64 00");
        bad.extend(bytes("FF 03 00 01 04 95 07 03 50"));
        let p = d.push(&bad);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].payload, vec![0x50]);
    }
}
