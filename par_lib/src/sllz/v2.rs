use anyhow::{Result, bail};
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io::{Read, Write};

pub fn decompress(input: &[u8], decompressed_size: usize) -> Result<Vec<u8>> {
    let mut output = Vec::with_capacity(decompressed_size);
    let mut input_pos = 0;

    while output.len() < decompressed_size {
        if input_pos + 5 > input.len() {
            bail!("SLLZV2: Unexpected end of input while reading chunk header");
        }

        let mut compressed_chunk_size = ((input[input_pos] as u32) << 16)
            | ((input[input_pos + 1] as u32) << 8)
            | (input[input_pos + 2] as u32);
        let decompressed_chunk_size = (((input[input_pos + 3] as u32) << 8) | (input[input_pos + 4] as u32)) + 1;

        let is_compressed = (compressed_chunk_size & 0x00800000) == 0;
        compressed_chunk_size &= 0x007FFFFF;

        if input_pos + compressed_chunk_size as usize > input.len() {
            bail!("SLLZV2: Unexpected end of input while reading chunk data");
        }

        if is_compressed {
            let mut decoder = ZlibDecoder::new(&input[input_pos + 5..input_pos + compressed_chunk_size as usize]);
            let mut decompressed_data = Vec::with_capacity(decompressed_chunk_size as usize);
            decoder.read_to_end(&mut decompressed_data)?;

            if decompressed_data.len() != decompressed_chunk_size as usize {
                bail!("SLLZV2: Wrong decompressed data size. Expected {}, got {}", decompressed_chunk_size, decompressed_data.len());
            }

            output.extend_from_slice(&decompressed_data);
        } else {
            let start = input_pos + 5;
            let end = start + decompressed_chunk_size as usize;
            if end > input.len() {
                bail!("SLLZV2: Unexpected end of input while copying uncompressed chunk");
            }
            output.extend_from_slice(&input[start..end]);
        }

        input_pos += compressed_chunk_size as usize;
    }

    Ok(output)
}

pub fn compress(input: &[u8]) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut current_pos = 0;

    while current_pos < input.len() {
        let decompressed_chunk_size = std::cmp::min(input.len() - current_pos, 0x10000);
        let chunk_data = &input[current_pos..current_pos + decompressed_chunk_size];

        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
        encoder.write_all(chunk_data)?;
        let compressed_data = encoder.finish()?;

        let compressed_chunk_size = (compressed_data.len() + 5) as u32;
        
        // Write compressed chunk size (3 bytes, Big Endian)
        output.push((compressed_chunk_size >> 16) as u8);
        output.push((compressed_chunk_size >> 8) as u8);
        output.push(compressed_chunk_size as u8);

        // Write decompressed chunk size - 1 (2 bytes, Big Endian)
        let temp = (decompressed_chunk_size - 1) as u16;
        output.push((temp >> 8) as u8);
        output.push(temp as u8);

        output.extend_from_slice(&compressed_data);

        current_pos += decompressed_chunk_size;
    }

    Ok(output)
}
