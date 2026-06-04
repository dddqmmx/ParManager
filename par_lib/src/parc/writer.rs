// -------------------------------------------------------
// © Kaplas. Licensed under MIT. See LICENSE for details.
// -------------------------------------------------------
use crate::parc::{ParArchive, ParDirectory, ParFile, ParNode};
use crate::sllz;
use anyhow::Result;
use byteorder::{WriteBytesExt, BigEndian, LittleEndian};
use encoding_rs::WINDOWS_1252;
use std::io::{Cursor, Write};

pub struct WriterOptions {
    pub compressor_version: u8,
}

pub fn write(archive: &ParArchive, options: &WriterOptions) -> Result<Vec<u8>> {
    let mut folders = Vec::new();
    let mut files = Vec::new();

    // Flatten tree
    flatten_tree(&archive.root, &mut folders, &mut files, options)?;

    // Compress files if needed
    for file in &mut files {
        if options.compressor_version != 0 && !file.is_compressed && file.data.len() > 0 {
            let endianness = if archive.endianness == 0 {
                sllz::Endianness::LittleEndian
            } else {
                sllz::Endianness::BigEndian
            };

            if let Ok(compressed) = sllz::compress(&file.data, options.compressor_version, endianness) {
                let diff = file.data.len() as i64 - compressed.len() as i64;
                if diff >= 0 && (file.data.len() < 2048 || diff >= 2048) {
                    file.data = compressed;
                    file.is_compressed = true;
                }
            }
        }
    }

    let header_size = 32 + (64 * folders.len()) + (64 * files.len());
    let folder_table_offset = header_size;
    let file_table_offset = folder_table_offset + (folders.len() * 32);
    let mut data_position = file_table_offset + (files.len() * 32);
    data_position = align(data_position as u64, 2048) as usize;

    let mut output = Vec::new();
    let mut writer = Cursor::new(&mut output);

    writer.write_all(b"PARC")?;
    writer.write_u8(archive.platform_id)?;
    writer.write_u8(archive.endianness)?;
    writer.write_u16::<BigEndian>(0)?; // extended size and relocated

    if archive.endianness == 0 {
        write_content::<LittleEndian>(&mut writer, archive, &folders, &files, folder_table_offset, file_table_offset, data_position as u64)?;
    } else {
        write_content::<BigEndian>(&mut writer, archive, &folders, &files, folder_table_offset, file_table_offset, data_position as u64)?;
    }

    Ok(output)
}

fn flatten_tree(
    dir: &ParDirectory,
    folders: &mut Vec<RawFolderWriteInfo>,
    files: &mut Vec<ParFile>,
    options: &WriterOptions,
) -> Result<()> {
    let current_folder_idx = folders.len();
    folders.push(RawFolderWriteInfo {
        name: dir.name.clone(),
        first_folder_index: 0,
        folder_count: 0,
        first_file_index: files.len() as i32,
        file_count: 0,
        attributes: dir.attributes,
    });

    let mut children_folders = Vec::new();
    let mut children_files = Vec::new();

    for child in &dir.children {
        match child {
            ParNode::Directory(d) => children_folders.push(d),
            ParNode::File(f) => children_files.push(f.clone()),
            ParNode::Archive(a) => {
                // Nested archive becomes a file in the parent
                let archive_data = write(a, options)?;
                children_files.push(ParFile {
                    name: a.root.name.clone(),
                    data: archive_data,
                    decompressed_size: 0, // Will be updated if compressed? No, usually not for nested.
                    is_compressed: false,
                    attributes: 0x00000020,
                    timestamp: 0, // Should probably take from somewhere
                });
            }
        }
    }

    folders[current_folder_idx].folder_count = children_folders.len() as i32;
    folders[current_folder_idx].first_folder_index = folders.len() as i32;
    folders[current_folder_idx].file_count = children_files.len() as i32;

    for f in children_files {
        files.push(f);
    }

    for d in children_folders {
        flatten_tree(d, folders, files, options)?;
    }

    Ok(())
}

struct RawFolderWriteInfo {
    name: String,
    first_folder_index: i32,
    folder_count: i32,
    first_file_index: i32,
    file_count: i32,
    attributes: i32,
}

fn write_content<E: byteorder::ByteOrder>(
    writer: &mut Cursor<&mut Vec<u8>>,
    archive: &ParArchive,
    folders: &[RawFolderWriteInfo],
    files: &[ParFile],
    folder_table_offset: usize,
    file_table_offset: usize,
    mut data_position: u64,
) -> Result<()> {
    writer.write_i32::<E>(archive.version)?;
    writer.write_i32::<E>(0)?; // data size (placeholder)

    writer.write_i32::<E>(folders.len() as i32)?;
    writer.write_i32::<E>(folder_table_offset as i32)?;
    writer.write_i32::<E>(files.len() as i32)?;
    writer.write_i32::<E>(file_table_offset as i32)?;

    // Name Tables
    for f in folders {
        writer.write_all(&encode_name(&f.name))?;
    }
    for f in files {
        writer.write_all(&encode_name(&f.name))?;
    }

    // Folder Info Table
    for f in folders {
        writer.write_i32::<E>(f.folder_count)?;
        writer.write_i32::<E>(f.first_folder_index)?;
        writer.write_i32::<E>(f.file_count)?;
        writer.write_i32::<E>(f.first_file_index)?;
        writer.write_i32::<E>(f.attributes)?;
        writer.write_i32::<E>(0)?;
        writer.write_i32::<E>(0)?;
        writer.write_i32::<E>(0)?;
    }

    // File Info Table & Data
    let file_info_start_pos = writer.position();
    // Pre-allocate space for file info
    writer.write_all(&vec![0u8; files.len() * 32])?;

    let mut file_infos = Vec::new();

    for file in files {
        if file.data.len() > 2048 {
            data_position = align(data_position, 2048);
        } else {
            // Check if it fits in current block? C# logic is a bit more complex.
            // Simplified: always align to 2048 if it's large or if we reached end of block.
            data_position = align(data_position, 2048);
        }

        file_infos.push(RawFileWriteInfo {
            is_compressed: file.is_compressed,
            decompressed_size: file.decompressed_size,
            compressed_size: file.data.len() as u32,
            offset: data_position,
            attributes: file.attributes,
            timestamp: file.timestamp,
        });

        let current_pos = writer.position();
        writer.set_position(data_position);
        writer.write_all(&file.data)?;
        data_position = writer.position();
        writer.set_position(current_pos);
    }

    // Write File Infos
    writer.set_position(file_info_start_pos);
    for info in file_infos {
        writer.write_u32::<E>(if info.is_compressed { 0x80000000 } else { 0 })?;
        writer.write_u32::<E>(info.decompressed_size)?;
        writer.write_u32::<E>(info.compressed_size)?;
        writer.write_u32::<E>(info.offset as u32)?;
        writer.write_i32::<E>(info.attributes)?;
        writer.write_u32::<E>((info.offset >> 32) as u32)?;
        writer.write_u64::<E>(info.timestamp)?;
    }

    // Padding at the end
    writer.set_position(data_position);
    let final_aligned = align(data_position, 2048);
    let padding = (final_aligned - data_position) as usize;
    writer.write_all(&vec![0u8; padding])?;

    Ok(())
}

struct RawFileWriteInfo {
    is_compressed: bool,
    decompressed_size: u32,
    compressed_size: u32,
    offset: u64,
    attributes: i32,
    timestamp: u64,
}

fn align(pos: u64, align: u64) -> u64 {
    if pos % align == 0 {
        pos
    } else {
        pos + (align - (pos % align))
    }
}

fn encode_name(name: &str) -> [u8; 64] {
    let mut bytes = [0u8; 64];
    let (encoded, _, _) = WINDOWS_1252.encode(name);
    let encoded = encoded.as_ref();
    let len = std::cmp::min(encoded.len(), 63);
    bytes[..len].copy_from_slice(&encoded[..len]);
    bytes
}
