use super::dns_reader::DnsReader;
use super::dns_writer::DnsWriter;
use super::records::{DnsQuestion, DnsResource};
use std::io::Result as IoResult;

/// A DNS/mDNS packet, including the header fields (ID, flags, question count, etc.)
#[derive(Debug, Clone)]
pub struct DnsPacket {
    pub id: u16,
    pub is_response: bool,
    pub opcode: u8,
    pub is_conflict: bool,
    pub is_truncated: bool,
    pub is_tentative: bool,
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
        let is_conflict = (flags & 0x0400) != 0;
        let is_truncated = (flags & 0x0200) != 0;
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
            is_conflict,
            is_truncated,
            is_tentative,
            rcode,
            questions,
            answers,
            authorities,
            additionals,
        })
    }

    pub fn to_bytes(&self) -> IoResult<Vec<u8>> {
        let mut writer = DnsWriter::new();
        writer.write_u16(self.id)?;

        // Construct flags
        let mut flags: u16 = 0;
        if self.is_response { flags |= 0x8000; }
        flags |= (self.opcode as u16 & 0x0F) << 11;
        if self.is_conflict { flags |= 0x0400; }
        if self.is_truncated { flags |= 0x0200; }
        if self.is_tentative { flags |= 0x0100; }
        flags |= (self.rcode as u16) & 0x000F;
        writer.write_u16(flags)?;

        writer.write_u16(self.questions.len() as u16)?;
        writer.write_u16(self.answers.len() as u16)?;
        writer.write_u16(self.authorities.len() as u16)?;
        writer.write_u16(self.additionals.len() as u16)?;

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

    pub fn new_response() -> DnsPacket {
        DnsPacket {
            id: 0,
            is_response: true,
            opcode: 0,
            is_conflict: false,
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
