// -------------------------------------------------------
// © Kaplas. Licensed under MIT. See LICENSE for details.
// -------------------------------------------------------
pub mod reader;
pub mod writer;

use chrono::{DateTime, Utc, TimeZone};

#[derive(Debug, Clone)]
pub struct ParArchive {
    pub platform_id: u8,
    pub endianness: u8, // 0 = Little, 1 = Big
    pub version: i32,
    pub root: ParDirectory,
}

#[derive(Debug, Clone)]
pub enum ParNode {
    File(ParFile),
    Directory(ParDirectory),
    Archive(ParArchive),
}

impl ParNode {
    pub fn name(&self) -> &str {
        match self {
            ParNode::File(f) => &f.name,
            ParNode::Directory(d) => &d.name,
            ParNode::Archive(a) => &a.root.name,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParFile {
    pub name: String,
    pub data: Vec<u8>,
    pub decompressed_size: u32,
    pub is_compressed: bool,
    pub attributes: i32,
    pub timestamp: u64,
}

impl ParFile {
    pub fn date_time(&self) -> DateTime<Utc> {
        Utc.timestamp_opt(self.timestamp as i64, 0).unwrap()
    }
}

#[derive(Debug, Clone)]
pub struct ParDirectory {
    pub name: String,
    pub children: Vec<ParNode>,
    pub attributes: i32,
}

impl ParDirectory {
    pub fn new(name: String) -> Self {
        Self {
            name,
            children: Vec::new(),
            attributes: 0x00000010,
        }
    }
}
