use anyhow::{anyhow, Result};
use byteorder::{ReadBytesExt, LittleEndian};
use std::io::{Cursor, Read};

#[derive(Debug)]
pub struct Packet {
    pub payload: Vec<u8>,
    pub packet_type: u32,
    pub time: f32,
    pub length: u32,
}

pub struct PacketStream<'a> {
    reader: &'a mut Cursor<Vec<u8>>,
}

impl<'a> PacketStream<'a> {
    pub fn new(reader: &'a mut Cursor<Vec<u8>>) -> Self {
        Self { reader }
    }
}

impl<'a> Iterator for PacketStream<'a> {
    type Item = Result<Packet>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.reader.position() >= self.reader.get_ref().len() as u64 {
            return None;
        }

        match self.read_packet() {
            Ok(packet) => Some(Ok(packet)),
            Err(e) => Some(Err(e)),
        }
    }
}

impl<'a> PacketStream<'a> {
    fn read_packet(&mut self) -> Result<Packet> {
        // Basic packet structure (based on assumptions/common WoT formats, needs verification against wotdecoder)
        // Usually: Length (4 bytes) + Type (4 bytes) + Time (4 bytes) + Payload
        
        let payload_len = self.reader.read_u32::<LittleEndian>()?;
        let packet_type = self.reader.read_u32::<LittleEndian>()?;
        let time = self.reader.read_f32::<LittleEndian>()?;

        let mut payload = vec![0u8; payload_len as usize];
        self.reader.read_exact(&mut payload)?;

        Ok(Packet {
            payload,
            packet_type,
            time,
            length: payload_len + 12, // storing total length including header for debug/consistency
        })
    }
}
