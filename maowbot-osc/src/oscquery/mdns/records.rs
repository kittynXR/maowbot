use super::dns_reader::DnsReader;
use super::dns_writer::DnsWriter;
use std::io::Result as IoResult;
use std::io::Error;
use tracing::trace; // Added for verbose logging

/// DNS TYPE constants
pub const TYPE_A: u16 = 0x0001;
pub const TYPE_PTR: u16 = 0x000C;
pub const TYPE_TXT: u16 = 0x0010;
pub const TYPE_SRV: u16 = 0x0021;
// etc. for any other record types we need

#[derive(Debug, Clone)]
pub struct DnsQuestion {
    pub labels: Vec<String>,
    pub qtype: u16,
    pub qclass: u16,
}

impl DnsQuestion {
    pub fn parse(reader: &mut DnsReader) -> IoResult<DnsQuestion> {
        let labels = reader.read_domain_labels()?;
        let qtype = reader.read_u16()?;
        let qclass = reader.read_u16()?;
        Ok(DnsQuestion { labels, qtype, qclass })
    }

    pub fn write(&self, writer: &mut DnsWriter) -> IoResult<()> {
        writer.write_domain_labels(&self.labels)?;
        writer.write_u16(self.qtype)?;
        writer.write_u16(self.qclass)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct DnsResource {
    pub labels: Vec<String>,
    pub rtype: u16,
    pub rclass: u16,
    pub ttl: u32,
    pub rdata: RData,
}

impl DnsResource {
    pub fn parse(reader: &mut DnsReader) -> IoResult<DnsResource> {
        let labels = reader.read_domain_labels()?;
        let rtype = reader.read_u16()?;
        let rclass = reader.read_u16()?;
        let ttl = reader.read_u32()?;
        let rdlength = reader.read_u16()?;

        let data_start = reader.position();
        let rdata = RData::parse(rtype, reader, rdlength)?;
        let data_end = reader.position();
        if data_end - data_start != rdlength as u64 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Resource data read mismatch",
            ));
        }

        Ok(DnsResource {
            labels,
            rtype,
            rclass,
            ttl,
            rdata,
        })
    }

    pub fn write(&self, writer: &mut DnsWriter) -> IoResult<()> {
        writer.write_domain_labels(&self.labels)?;
        writer.write_u16(self.rtype)?;
        writer.write_u16(self.rclass)?;
        writer.write_u32(self.ttl)?;

        // We have to write rdata length after we gather rdata bytes
        let mut inner = DnsWriter::new();
        self.rdata.write(&mut inner)?;

        let rdata_bytes = inner.into_inner();
        writer.write_u16(rdata_bytes.len() as u16)?;
        writer.write_bytes(&rdata_bytes)?;

        Ok(())
    }
}

/// The different record data types we can handle
#[derive(Debug, Clone)]
pub enum RData {
    ARecord(Vec<u8>),          // 4 bytes for IPv4
    PTR(Vec<String>),          // domain name
    TXT(Vec<String>),          // repeated strings
    SRV(u16, u16, u16, Vec<String>), // priority, weight, port, target
    Unknown(Vec<u8>),          // fallback
}

impl RData {
    pub fn parse(rtype: u16, reader: &mut DnsReader, length: u16) -> IoResult<RData> {
        match rtype {
            TYPE_A => {
                let bytes = reader.read_bytes(length as usize)?;
                Ok(RData::ARecord(bytes))
            },
            TYPE_PTR => {
                // entire RDATA is a domain name
                let start = reader.position();
                let labels = reader.read_domain_labels()?;
                trace!("Parsed PTR record at offset {}: {:?}", start, labels);
                Ok(RData::PTR(labels))
            },
            TYPE_TXT => {
                // One or more strings, each is length-prefixed
                let mut raw = reader.read_bytes(length as usize)?;
                let mut out = Vec::new();
                while !raw.is_empty() {
                    let first = raw[0] as usize;
                    if first + 1 > raw.len() {
                        break; // broken
                    }
                    let txt_data = &raw[1..(1 + first)];
                    out.push(String::from_utf8_lossy(txt_data).to_string());
                    raw.drain(0..(1 + first));
                }
                trace!("Parsed TXT record: {:?}", out);
                Ok(RData::TXT(out))
            },
            TYPE_SRV => {
                let priority = reader.read_u16()?;
                let weight = reader.read_u16()?;
                let port = reader.read_u16()?;
                let target = reader.read_domain_labels()?;
                trace!("Parsed SRV record: priority: {}, weight: {}, port: {}, target: {:?}", priority, weight, port, target);
                Ok(RData::SRV(priority, weight, port, target))
            },
            _ => {
                // just read raw bytes
                let data = reader.read_bytes(length as usize)?;
                Ok(RData::Unknown(data))
            },
        }
    }

    pub fn write(&self, writer: &mut DnsWriter) -> IoResult<()> {
        match self {
            RData::ARecord(bytes) => {
                writer.write_bytes(bytes)?;
            },
            RData::PTR(labels) => {
                writer.write_domain_labels(labels)?;
            },
            RData::TXT(strings) => {
                for s in strings {
                    let len = s.len() as u8;
                    writer.write_bytes(&[len])?;
                    writer.write_bytes(s.as_bytes())?;
                }
            },
            RData::SRV(priority, weight, port, target) => {
                writer.write_u16(*priority)?;
                writer.write_u16(*weight)?;
                writer.write_u16(*port)?;
                writer.write_domain_labels(target)?;
            },
            RData::Unknown(bytes) => {
                writer.write_bytes(bytes)?;
            },
        }
        Ok(())
    }
}
