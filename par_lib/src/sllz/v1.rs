use anyhow::{Result, bail};

const MAX_WINDOW_SIZE: usize = 4096;
const MAX_ENCODED_LENGTH: usize = 18;

pub fn decompress(input: &[u8], decompressed_size: usize) -> Result<Vec<u8>> {
    let mut output = Vec::with_capacity(decompressed_size);
    let mut input_pos = 0;

    if input.is_empty() {
        return Ok(output);
    }

    let mut flag = input[input_pos];
    input_pos += 1;
    let mut flag_count = 8;

    while output.len() < decompressed_size {
        if (flag & 0x80) == 0x80 {
            flag <<= 1;
            flag_count -= 1;
            if flag_count == 0 {
                if input_pos >= input.len() {
                    break;
                }
                flag = input[input_pos];
                input_pos += 1;
                flag_count = 8;
            }

            if input_pos + 1 >= input.len() {
                bail!("SLLZV1: Unexpected end of input while reading copy flags");
            }
            let copy_flags = (input[input_pos] as u16) | ((input[input_pos + 1] as u16) << 8);
            input_pos += 2;

            let copy_distance = 1 + (copy_flags >> 4) as usize;
            let copy_count = 3 + (copy_flags & 0xF) as usize;

            for _ in 0..copy_count {
                if output.len() < copy_distance {
                    bail!("SLLZV1: Invalid copy distance");
                }
                let val = output[output.len() - copy_distance];
                output.push(val);
            }
        } else {
            flag <<= 1;
            flag_count -= 1;
            if flag_count == 0 {
                if input_pos >= input.len() && output.len() < decompressed_size {
                    bail!("SLLZV1: Unexpected end of input while reading literal");
                }
                if input_pos < input.len() {
                    flag = input[input_pos];
                    input_pos += 1;
                    flag_count = 8;
                }
            }

            if input_pos >= input.len() {
                if output.len() < decompressed_size {
                    bail!("SLLZV1: Unexpected end of input while reading literal data");
                }
                break;
            }
            output.push(input[input_pos]);
            input_pos += 1;
        }
    }

    Ok(output)
}

pub fn compress(input: &[u8]) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut input_pos = 0;
    
    let mut current_flag = 0u8;
    let mut bit_count = 0;
    
    // Placeholder for flag byte
    let mut flag_pos = output.len();
    output.push(0);

    while input_pos < input.len() {
        let window_size = std::cmp::min(input_pos, MAX_WINDOW_SIZE);
        let max_offset_length = std::cmp::min(input.len() - input_pos, MAX_ENCODED_LENGTH);

        if let Some((offset, length)) = find_match(input, input_pos, window_size, max_offset_length) {
            current_flag |= 1 << (7 - bit_count);
            bit_count += 1;

            if bit_count == 8 {
                output[flag_pos] = current_flag;
                current_flag = 0;
                bit_count = 0;
                flag_pos = output.len();
                output.push(0);
            }

            let tuple = ((offset as u16 - 1) << 4) | ((length as u16 - 3) & 0x0F);
            output.push(tuple as u8);
            output.push((tuple >> 8) as u8);
            input_pos += length;
        } else {
            // Literal
            bit_count += 1;

            if bit_count == 8 {
                output[flag_pos] = current_flag;
                current_flag = 0;
                bit_count = 0;
                flag_pos = output.len();
                output.push(0);
            }

            output.push(input[input_pos]);
            input_pos += 1;
        }
    }

    output[flag_pos] = current_flag;
    Ok(output)
}

fn find_match(input: &[u8], input_pos: usize, window_size: usize, max_offset_length: usize) -> Option<(usize, usize)> {
    if window_size == 0 || max_offset_length < 3 {
        return None;
    }

    let window_start = input_pos - window_size;
    let window = &input[window_start..input_pos];
    
    for length in (3..=max_offset_length).rev() {
        if window.len() < length {
            continue;
        }
        let pattern = &input[input_pos..input_pos + length];
        
        // Search from the end of the window to get the closest match
        for i in (0..=window.len() - length).rev() {
            if &window[i..i + length] == pattern {
                return Some((window.len() - i, length));
            }
        }
    }

    None
}
