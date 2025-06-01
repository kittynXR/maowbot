use std::io::{Cursor, Write};
use byteorder::{BigEndian, WriteBytesExt};

/// A "big-endian" DNS writer to parallel the C# `BigWriter`.
pub struct DnsWriter {
    pub data: Cursor<Vec<u8>>,
}

impl DnsWriter {
    pub fn new() -> Self {
        DnsWriter {
            data: Cursor::new(Vec::new()),
        }
    }

    pub fn write_u16(&mut self, v: u16) -> std::io::Result<()> {
        self.data.write_u16::<BigEndian>(v)
    }

    pub fn write_u32(&mut self, v: u32) -> std::io::Result<()> {
        self.data.write_u32::<BigEndian>(v)
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.data.write_all(bytes)
    }

    /// Write domain labels. Each label is prefixed by a length byte; ends with 0 length.
    pub fn write_domain_labels(&mut self, labels: &[String]) -> std::io::Result<()> {
        for label in labels {
            let len = label.len() as u8;
            self.data.write_all(&[len])?;
            self.data.write_all(label.as_bytes())?;
        }
        // Null label terminates
        self.data.write_all(&[0])?;
        Ok(())
    }

    pub fn into_inner(self) -> Vec<u8> {
        self.data.into_inner()
    }
}
