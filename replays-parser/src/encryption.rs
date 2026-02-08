use blowfish::Blowfish;
use blowfish::cipher::{BlockDecrypt, KeyInit, generic_array::GenericArray};
use anyhow::{Result, anyhow};
use byteorder::BigEndian;

// World of Tanks keys (from wotreplay-parser reference)
// 0xDE, 0x72, 0xBE, 0xA0, ...
const WOT_KEY: [u8; 16] = [
    0xDE, 0x72, 0xBE, 0xA0, 0xDE, 0x04, 0xBE, 0xB1, 
    0xDE, 0xFE, 0xBE, 0xEF, 0xDE, 0xAD, 0xBE, 0xEF
];

pub fn decrypt_replay(encrypted_data: &[u8]) -> Result<Vec<u8>> {
    let cipher = Blowfish::<byteorder::BigEndian>::new_from_slice(&WOT_KEY).map_err(|e| anyhow!("Invalid key length: {}", e))?;

    let block_size = 8;
    if encrypted_data.len() % block_size != 0 {
        return Err(anyhow!("Encrypted data length is not a multiple of block size"));
    }

    let mut decrypted_data = vec![0u8; encrypted_data.len()];
    let mut previous_block = [0u8; 8];

    // Padding/boundary check
    let chunks = encrypted_data.chunks_exact(block_size);
    
    // The C++ implementation:
    // cipherContext.update(decrypted, &decrypted_len, begin + pin, block_size);
    // std::transform(previous, previous + decrypted_len, decrypted, decrypted, std::bit_xor<unsigned char>());
    // std::copy_n(decrypted, block_size, previous);
    // std::copy_n(decrypted, block_size, begin + pout);

    // Essentially: Decrypt(Current) -> XOR with Previous -> Update Previous -> Output
    // Note: The 'previous' in C++ starts as 0.
    // Wait, the C++ loop logic:
    // 1. Decrypt into `decrypted`
    // 2. `previous` (which was the PREVIOUS decrypted block) XOR `decrypted` -> `decrypted`
    // 3. `decrypted` (now XORed) is saved as `previous` for the NEXT step.
    // 4. `decrypted` is written to output.
    
    // Let's trace closely:
    // Iteration 1:
    //   Decrypt(Cipher1) -> Temp
    //   Previous (0) XOR Temp -> Decrypted1
    //   Previous = Decrypted1
    //   Output = Decrypted1
    
    // Iteration 2:
    //   Decrypt(Cipher2) -> Temp
    //   Previous (Decrypted1) XOR Temp -> Decrypted2
    //   Previous = Decrypted2
    //   Output = Decrypted2

    // This is effectively: Decrypted[i] = Decrypt(Cipher[i]) ^ Decrypted[i-1]
    
    for (i, chunk) in chunks.enumerate() {
        let mut block = GenericArray::clone_from_slice(chunk);
        cipher.decrypt_block(&mut block);
        
        let mut decrypted_block = [0u8; 8];
        decrypted_block.copy_from_slice(block.as_slice());

        for j in 0..8 {
            decrypted_block[j] ^= previous_block[j];
        }

        previous_block = decrypted_block;
        
        let start = i * block_size;
        decrypted_data[start..start + 8].copy_from_slice(&decrypted_block);
    }

    Ok(decrypted_data)
}
