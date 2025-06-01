use super::dns_reader::DnsReader;
use super::dns_writer::DnsWriter;
use super::records::{DnsQuestion, DnsResource};
use std::io::Result as IoResult;

/// A DNS/mDNS packet, including the header fields (ID, flags, question count, etc.)
#[derive(Debug, Clone)]
pub struct DnsPacket {
    pub id: u16,
    /// QR bit (0x8000). True if response, false if query.
    pub is_response: bool,
    /// OPCODE bits (bits 11..14). Usually 0 for a standard query/response.
    pub opcode: u8,
    /// AA bit (0x0400). For mDNS, all answers should set AA=1.
    pub is_authoritative: bool,
    /// TC bit (0x0200). Truncated message
    pub is_truncated: bool,
    /// This was previously `is_tentative`;
    /// if you don’t need it, you can remove/ignore it
    pub is_tentative: bool,
    /// RCODE bits (bits 0..3). Usually 0 for "No error".
    pub rcode: u8,

    pub questions: Vec<DnsQuestion>,
    pub answers: Vec<DnsResource>,
    pub authorities: Vec<DnsResource>,
    pub additionals: Vec<DnsResource>,
}

impl DnsPacket {
    pub fn parse(mut reader: DnsReader) -> IoResult<DnsPacket> {
        let id = reader.read_u16()?;
        let flags = reader.read_u16()?;

        let is_response = (flags & 0x8000) != 0;
        let opcode = ((flags & 0x7800) >> 11) as u8;
        // In DNS, 0x0400 is the Authoritative-Answer bit (AA)
        let is_authoritative = (flags & 0x0400) != 0;
        let is_truncated = (flags & 0x0200) != 0;
        // 0x0100 can be RD or something else,
        // but in some mDNS usage might track "is_tentative".
        let is_tentative = (flags & 0x0100) != 0;
        let rcode = (flags & 0x000F) as u8;

        let qdcount = reader.read_u16()?;
        let ancount = reader.read_u16()?;
        let nscount = reader.read_u16()?;
        let arcount = reader.read_u16()?;

        let mut questions = Vec::with_capacity(qdcount as usize);
        for _ in 0..qdcount {
            questions.push(DnsQuestion::parse(&mut reader)?);
        }

        let mut answers = Vec::with_capacity(ancount as usize);
        for _ in 0..ancount {
            answers.push(DnsResource::parse(&mut reader)?);
        }

        let mut authorities = Vec::with_capacity(nscount as usize);
        for _ in 0..nscount {
            authorities.push(DnsResource::parse(&mut reader)?);
        }

        let mut additionals = Vec::with_capacity(arcount as usize);
        for _ in 0..arcount {
            additionals.push(DnsResource::parse(&mut reader)?);
        }

        Ok(DnsPacket {
            id,
            is_response,
            opcode,
            is_authoritative,
            is_truncated,
            is_tentative,
            rcode,
            questions,
            answers,
            authorities,
            additionals,
        })
    }

    /// Convert this packet into raw bytes for sending over UDP.
    pub fn to_bytes(&self) -> IoResult<Vec<u8>> {
        let mut writer = DnsWriter::new();

        // Transaction ID (in mDNS, usually 0)
        writer.write_u16(self.id)?;

        // Construct flags
        let mut flags: u16 = 0;
        // QR bit
        if self.is_response {
            flags |= 0x8000;
        }
        // OPCODE (bits 11..14)
        flags |= (self.opcode as u16 & 0x0F) << 11;
        // AA bit
        if self.is_authoritative {
            flags |= 0x0400;
        }
        // TC bit
        if self.is_truncated {
            flags |= 0x0200;
        }
        // RD or "is_tentative" bit (if used)
        if self.is_tentative {
            flags |= 0x0100;
        }
        // RCODE (bits 0..3)
        flags |= (self.rcode as u16) & 0x000F;

        writer.write_u16(flags)?;

        // Write question/answer/authority/additional counts
        writer.write_u16(self.questions.len() as u16)?;
        writer.write_u16(self.answers.len() as u16)?;
        writer.write_u16(self.authorities.len() as u16)?;
        writer.write_u16(self.additionals.len() as u16)?;

        // Serialize each section
        for q in &self.questions {
            q.write(&mut writer)?;
        }
        for ans in &self.answers {
            ans.write(&mut writer)?;
        }
        for auth in &self.authorities {
            auth.write(&mut writer)?;
        }
        for add in &self.additionals {
            add.write(&mut writer)?;
        }

        Ok(writer.into_inner())
    }

    /// Construct a brand-new response packet with typical mDNS defaults:
    /// - `id=0`
    /// - `is_response=true` (QR=1)
    /// - `is_authoritative=true` (AA=1)
    /// - `opcode=0` (standard query)
    /// - `rcode=0` (no error)
    pub fn new_response() -> DnsPacket {
        DnsPacket {
            id: 0,
            is_response: true,
            opcode: 0,
            // mDNS specs say to set AA=1 in answers
            is_authoritative: true,
            is_truncated: false,
            is_tentative: false,
            rcode: 0,
            questions: vec![],
            answers: vec![],
            authorities: vec![],
            additionals: vec![],
        }
    }
}
