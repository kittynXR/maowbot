use std::io::Cursor;
use std::collections::HashMap;
use byteorder::{BigEndian, ReadBytesExt};
use std::io::{Read, Result as IoResult};
use tracing::debug;

/// A simplified "big-endian" DNS reader. This parallels the C# `BigReader` code.
pub struct DnsReader {
    inner: Cursor<Vec<u8>>,
    name_cache: HashMap<u64, Vec<String>>,
}

impl DnsReader {
    pub fn new(data: Vec<u8>) -> Self {
        DnsReader {
            inner: Cursor::new(data),
            name_cache: HashMap::new(),
        }
    }

    pub fn position(&self) -> u64 {
        self.inner.position()
    }

    pub fn read_u16(&mut self) -> std::io::Result<u16> {
        self.inner.read_u16::<BigEndian>()
    }

    pub fn read_u32(&mut self) -> std::io::Result<u32> {
        self.inner.read_u32::<BigEndian>()
    }

    /// Read a specified number of bytes directly
    pub fn read_bytes(&mut self, len: usize) -> std::io::Result<Vec<u8>> {
        let mut buf = vec![0u8; len];
        self.inner.read_exact(&mut buf)?;
        Ok(buf)
    }

    /// Read a DNS "domain name" with potential compression. Recursively handles pointers.
    /// Returns a list of labels. A terminating 0-length label indicates the end.
    pub fn read_domain_labels(&mut self) -> std::io::Result<Vec<String>> {
        let start_pos = self.position();
        let mut out_labels = Vec::new();
        let mut first_pass = true;

        loop {
            let b = match self.inner.read_u8() {
                Ok(v) => v,
                Err(e) => return Err(e),
            };

            if b == 0 {
                // End of name.
                break;
            }

            // Check if the top two bits (0xC0) are set, indicating a pointer.
            if (b & 0xC0) == 0xC0 {
                let pointer_low = self.inner.read_u8()?;
                let offset = ((b as u16 ^ 0xC0) << 8) | pointer_low as u16;
                let saved_pos = self.position();

                self.inner.set_position(offset as u64);
                if let Some(cached) = self.name_cache.get(&(offset as u64)) {
                    debug!("Using cached labels for pointer at offset {}: {:?}", offset, cached);
                    out_labels.extend_from_slice(cached);
                } else {
                    let sub_labels = self.read_domain_labels()?;
                    debug!("Read pointer labels at offset {}: {:?}", offset, sub_labels);
                    self.name_cache.insert(offset as u64, sub_labels.clone());
                    out_labels.extend(sub_labels);
                }
                self.inner.set_position(saved_pos);
                // The pointer terminates the label sequence.
                break;
            } else {
                // b is the length of the next label.
                let length = b as usize;
                let mut buf = vec![0u8; length];
                self.inner.read_exact(&mut buf)?;
                let s = String::from_utf8_lossy(&buf).to_string();
                debug!("Read label: '{}'", s);
                out_labels.push(s);
            }

            if first_pass {
                self.name_cache.insert(start_pos, out_labels.clone());
                first_pass = false;
            }
        }

        self.name_cache.insert(start_pos, out_labels.clone());
        debug!("Completed reading domain labels starting at {}: {:?}", start_pos, out_labels);
        Ok(out_labels)
    }
}
