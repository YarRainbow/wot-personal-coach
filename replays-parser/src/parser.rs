use crate::types::{BattleConfig, Replay, ReplayHeader};
use anyhow::{anyhow, Context, Result};
use byteorder::{ReadBytesExt, LittleEndian};
use std::io::Read;
use std::path::Path;
use std::{fs::File, io::Cursor};

pub struct Parser {
    reader: Cursor<Vec<u8>>, 
}

impl Parser {
    pub fn parse_file(path: &Path) -> Result<Replay> {
        let mut file = File::open(path).with_context(|| format!("Failed to open file: {:?}", path))?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        let mut parser = Parser {
            reader: Cursor::new(buffer),
        };
        parser.parse()
    }

    pub fn parse(&mut self) -> Result<Replay> {
        let magic = self.read_magic()?;
        let block_count = self.read_block_count()?;
        
        let battle_config: BattleConfig = self.read_json_block("BattleConfig")?;
        
        let mut battle_results = None;
        if block_count >= 2 {
             // Try to read block 2 (Battle Results)
             // In some replays (incomplete), this might be missing or empty.
             if let Ok(results) = self.read_json_block::<serde_json::Value>("BattleResults") {
                 battle_results = Some(results);
             } else {
                 // If we fail to read the second block but block_count >= 2, 
                 // it likely means it's an incomplete replay or structure difference.
                 // We can either warn or continue. For now, let's treat it as optional if it fails.
             }
        }

        // The binary block is always at the end.
        let packets_buffer = self.read_binary_block()?;

        Ok(Replay {
            header: ReplayHeader { magic, block_count },
            battle_config,
            battle_results,
            packets_buffer,
        })
    }

    fn read_magic(&mut self) -> Result<u32> {
        let magic = self.reader.read_u32::<LittleEndian>()?;
        if magic != 0x11343212 {
            return Err(anyhow!("Invalid magic number: {:x}, expected 11343212", magic));
        }
        Ok(magic)
    }

    fn read_block_count(&mut self) -> Result<u32> {
        Ok(self.reader.read_u32::<LittleEndian>()?)
    }

    fn read_json_block<T: serde::de::DeserializeOwned>(&mut self, block_name: &str) -> Result<T> {
        let block_size = self.reader.read_u32::<LittleEndian>()
            .with_context(|| format!("Failed to read size for {}", block_name))?;
            
        if block_size == 0 {
             return Err(anyhow!("Block size is 0 for {}", block_name));
        }

        let mut block_data = vec![0u8; block_size as usize];
        self.reader.read_exact(&mut block_data)
            .with_context(|| format!("Failed to read data for {}", block_name))?;
            
        let result: T = serde_json::from_slice(&block_data)
            .with_context(|| format!("Failed to parse JSON for {}", block_name))?;
            
        Ok(result)
    }

    fn read_binary_block(&mut self) -> Result<Vec<u8>> {
        // Binary block header
        let decompressed_size = self.reader.read_u32::<LittleEndian>()
            .with_context(|| "Failed to read binary decompressed size")?;
        let compressed_size = self.reader.read_u32::<LittleEndian>()
            .with_context(|| "Failed to read binary compressed size")?;

        // Encrypted data must be a multiple of 8 bytes (Blowfish block size)
        let encrypted_len = ((compressed_size + 7) / 8) * 8;
        
        let mut encrypted_data = vec![0u8; encrypted_len as usize];
        self.reader.read_exact(&mut encrypted_data)
            .with_context(|| "Failed to read encrypted binary data")?;

        // Decrypt
        use crate::encryption::decrypt_replay;
        let decrypted_data = decrypt_replay(&encrypted_data)
            .with_context(|| "Failed to decrypt replay")?;

        // Decompress
        // Only slice the valid compressed data (ignore padding)
        if (compressed_size as usize) > decrypted_data.len() {
             return Err(anyhow!("Compressed size {} > Decrypted data length {}", compressed_size, decrypted_data.len()));
        }
        
        let valid_compressed_data = &decrypted_data[0..compressed_size as usize];
        
        use flate2::read::ZlibDecoder;
        let mut decoder = ZlibDecoder::new(valid_compressed_data);
        let mut decompressed_data = Vec::with_capacity(decompressed_size as usize);
        decoder.read_to_end(&mut decompressed_data)
            .with_context(|| "Failed to decompress replay")?;

        Ok(decompressed_data)
    }
}
