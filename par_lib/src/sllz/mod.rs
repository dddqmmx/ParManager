pub mod v1;
pub mod v2;

use anyhow::{Result, bail};
use byteorder::{ReadBytesExt, WriteBytesExt, BigEndian, LittleEndian};
use std::io::{Cursor, Read, Write};

#[derive(Debug, Copy, Clone)]
pub enum Endianness {
    LittleEndian = 0,
    BigEndian = 1,
}

pub fn decompress(input: &[u8]) -> Result<Vec<u8>> {
    let mut reader = Cursor::new(input);
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic)?;
    
    if &magic != b"SLLZ" {
        bail!("SLLZ: Bad magic Id.");
    }

    let endianness_val = reader.read_u8()?;
    let version = reader.read_u8()?;
    let header_size = if endianness_val == 0 {
        reader.read_u16::<LittleEndian>()?
    } else {
        reader.read_u16::<BigEndian>()?
    };

    let decompressed_size = if endianness_val == 0 {
        reader.read_i32::<LittleEndian>()?
    } else {
        reader.read_i32::<BigEndian>()?
    };

    let _compressed_size = if endianness_val == 0 {
        reader.read_i32::<LittleEndian>()?
    } else {
        reader.read_i32::<BigEndian>()?
    };

    let data_start = header_size as usize;
    let compressed_data = &input[data_start..];

    match version {
        1 => v1::decompress(compressed_data, decompressed_size as usize),
        2 => v2::decompress(compressed_data, decompressed_size as usize),
        _ => bail!("SLLZ: Unknown compression version {}", version),
    }
}

pub fn compress(input: &[u8], version: u8, endianness: Endianness) -> Result<Vec<u8>> {
    let compressed_data = match version {
        1 => v1::compress(input)?,
        2 => v2::compress(input)?,
        _ => bail!("SLLZ: Unknown compression version {}", version),
    };

    let mut output = Vec::new();
    let mut writer = Cursor::new(&mut output);

    writer.write_all(b"SLLZ")?;
    writer.write_u8(endianness as u8)?;
    writer.write_u8(version)?;
    
    match endianness {
        Endianness::LittleEndian => {
            writer.write_u16::<LittleEndian>(0x10)?;
            writer.write_i32::<LittleEndian>(input.len() as i32)?;
            writer.write_i32::<LittleEndian>((compressed_data.len() + 0x10) as i32)?;
        }
        Endianness::BigEndian => {
            writer.write_u16::<BigEndian>(0x10)?;
            writer.write_i32::<BigEndian>(input.len() as i32)?;
            writer.write_i32::<BigEndian>((compressed_data.len() + 0x10) as i32)?;
        }
    }

    writer.write_all(&compressed_data)?;

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sllz_v1() -> Result<()> {
        let sample_text = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed pulvinar leo nec pulvinar pellentesque. Sed id dui et nisl tincidunt dignissim. Suspendisse ullamcorper eget ipsum et vehicula. Maecenas scelerisque dapibus rutrum. Suspendisse tincidunt dictum maximus. Ut rhoncus, lorem scelerisque euismod rhoncus, nunc augue egestas magna, ac mattis elit sapien eu erat. Pellentesque auctor in erat id molestie. Nam vehicula odio eget ipsum porta euismod. Donec eget placerat turpis. Aliquam erat volutpat. Etiam faucibus ligula sit amet ante tincidunt, sit amet efficitur justo lobortis. Nam volutpat augue at purus viverra tincidunt. Nam sapien eros, fringilla sollicitudin semper sed, bibendum eu nisl.\nPellentesque mattis, sem a placerat dictum, risus nisl faucibus odio, quis blandit est erat sed tellus. Mauris iaculis odio sed leo suscipit ultrices. Nunc leo purus, tempor at lobortis vel, molestie pellentesque dui. Sed ac nunc et metus placerat euismod non vel nisl. Proin fringilla viverra aliquet. Pellentesque scelerisque fermentum eleifend. Quisque rutrum orci nulla, sed ullamcorper nulla porttitor nec. Fusce elementum a nulla et ultricies. Pellentesque bibendum blandit leo nec gravida. Donec egestas vitae tellus id auctor.\nMorbi ultrices maximus mattis. Aliquam lobortis at lectus ut scelerisque. Morbi sem tortor, blandit non risus vel, commodo interdum neque. Fusce eu hendrerit mauris, in venenatis est. Vivamus vulputate placerat justo at condimentum. Proin porttitor ac velit quis pretium. Proin vitae lacus sit amet felis aliquam tempor. Fusce dignissim mi id dui imperdiet, nec commodo neque malesuada. Nullam tincidunt augue pellentesque aliquam fringilla. Sed placerat, nibh id fermentum volutpat, ligula felis sagittis lectus, vel semper ipsum arcu at ipsum. Etiam posuere tincidunt augue non gravida. Vivamus posuere posuere dui, a semper urna imperdiet a. Proin quis odio condimentum, pulvinar ipsum vitae, eleifend lorem.";
        let input = sample_text.as_bytes();

        let compressed = compress(input, 1, Endianness::LittleEndian)?;
        assert!(compressed.len() < input.len() + 0x10); // Not always smaller for very short strings, but should be for this one
        
        let decompressed = decompress(&compressed)?;
        assert_eq!(input, decompressed.as_slice());
        Ok(())
    }

    #[test]
    fn test_sllz_v2() -> Result<()> {
        let sample_text = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed pulvinar leo nec pulvinar pellentesque. Sed id dui et nisl tincidunt dignissim. Suspendisse ullamcorper eget ipsum et vehicula. Maecenas scelerisque dapibus rutrum. Suspendisse tincidunt dictum maximus. Ut rhoncus, lorem scelerisque euismod rhoncus, nunc augue egestas magna, ac mattis elit sapien eu erat. Pellentesque auctor in erat id molestie. Nam vehicula odio eget ipsum porta euismod. Donec eget placerat turpis. Aliquam erat volutpat. Etiam faucibus ligula sit amet ante tincidunt, sit amet efficitur justo lobortis. Nam volutpat augue at purus viverra tincidunt. Nam sapien eros, fringilla sollicitudin semper sed, bibendum eu nisl.\nPellentesque mattis, sem a placerat dictum, risus nisl faucibus odio, quis blandit est erat sed tellus. Mauris iaculis odio sed leo suscipit ultrices. Nunc leo purus, tempor at lobortis vel, molestie pellentesque dui. Sed ac nunc et metus placerat euismod non vel nisl. Proin fringilla viverra aliquet. Pellentesque scelerisque fermentum eleifend. Quisque rutrum orci nulla, sed ullamcorper nulla porttitor nec. Fusce elementum a nulla et ultricies. Pellentesque bibendum blandit leo nec gravida. Donec egestas vitae tellus id auctor.\nMorbi ultrices maximus mattis. Aliquam lobortis at lectus ut scelerisque. Morbi sem tortor, blandit non risus vel, commodo interdum neque. Fusce eu hendrerit mauris, in venenatis est. Vivamus vulputate placerat justo at condimentum. Proin porttitor ac velit quis pretium. Proin vitae lacus sit amet felis aliquam tempor. Fusce dignissim mi id dui imperdiet, nec commodo neque malesuada. Nullam tincidunt augue pellentesque aliquam fringilla. Sed placerat, nibh id fermentum volutpat, ligula felis sagittis lectus, vel semper ipsum arcu at ipsum. Etiam posuere tincidunt augue non gravida. Vivamus posuere posuere dui, a semper urna imperdiet a. Proin quis odio condimentum, pulvinar ipsum vitae, eleifend lorem.";
        let input = sample_text.as_bytes();

        let compressed = compress(input, 2, Endianness::LittleEndian)?;
        // V2 might be slightly larger if input is small due to chunk headers, but this text is long enough
        
        let decompressed = decompress(&compressed)?;
        assert_eq!(input, decompressed.as_slice());
        Ok(())
    }
}
