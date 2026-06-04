use crate::parc::{ParArchive, ParDirectory, ParFile, ParNode};
use crate::sllz;
use anyhow::{Result, bail};
use byteorder::{ReadBytesExt, BigEndian, LittleEndian};
use encoding_rs::WINDOWS_1252;
use std::io::{Cursor, Read, Seek, SeekFrom};

pub struct ReaderOptions {
    pub recursive: bool,
}

pub fn read(input: &[u8], options: &ReaderOptions) -> Result<ParArchive> {
    let mut data = input;
    let decompressed_data;

    if data.len() >= 4 && &data[0..4] == b"SLLZ" {
        decompressed_data = sllz::decompress(data)?;
        data = &decompressed_data;
    }

    let mut reader = Cursor::new(data);
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic)?;
    if &magic != b"PARC" {
        bail!("PARC: Bad magic Id.");
    }

    let platform_id = reader.read_u8()?;
    let endianness = reader.read_u8()?;
    let _size_extended = reader.read_u8()?;
    let _relocated = reader.read_u8()?;

    if endianness == 0 {
        read_with_endianness::<LittleEndian>(reader, data, platform_id, endianness, options)
    } else {
        read_with_endianness::<BigEndian>(reader, data, platform_id, endianness, options)
    }
}

fn read_with_endianness<E: byteorder::ByteOrder>(
    mut reader: Cursor<&[u8]>,
    full_data: &[u8],
    platform_id: u8,
    endianness: u8,
    options: &ReaderOptions,
) -> Result<ParArchive> {
    let version = reader.read_i32::<E>()?;
    let _data_size = reader.read_i32::<E>()?;

    let total_folder_count = reader.read_i32::<E>()? as usize;
    let folder_info_offset = reader.read_i32::<E>()? as u64;
    let total_file_count = reader.read_i32::<E>()? as usize;
    let file_info_offset = reader.read_i32::<E>()? as u64;

    let mut folder_names = Vec::with_capacity(total_folder_count);
    for _ in 0..total_folder_count {
        let mut name_bytes = [0u8; 64];
        reader.read_exact(&mut name_bytes)?;
        folder_names.push(decode_name(&name_bytes));
    }

    let mut file_names = Vec::with_capacity(total_file_count);
    for _ in 0..total_file_count {
        let mut name_bytes = [0u8; 64];
        reader.read_exact(&mut name_bytes)?;
        file_names.push(decode_name(&name_bytes));
    }

    // Read Folder Info
    reader.seek(SeekFrom::Start(folder_info_offset))?;
    let mut raw_folders = Vec::with_capacity(total_folder_count);
    for i in 0..total_folder_count {
        raw_folders.push(RawFolderInfo {
            name: folder_names[i].clone(),
            folder_count: reader.read_i32::<E>()?,
            first_folder_index: reader.read_i32::<E>()?,
            file_count: reader.read_i32::<E>()?,
            first_file_index: reader.read_i32::<E>()?,
            attributes: reader.read_i32::<E>()?,
            unused: [
                reader.read_i32::<E>()?,
                reader.read_i32::<E>()?,
                reader.read_i32::<E>()?,
            ],
        });
    }

    // Read File Info
    reader.seek(SeekFrom::Start(file_info_offset))?;
    let mut raw_files = Vec::with_capacity(total_file_count);
    for i in 0..total_file_count {
        let compression_flag = reader.read_u32::<E>()?;
        let size = reader.read_u32::<E>()?;
        let compressed_size = reader.read_u32::<E>()?;
        let base_offset = reader.read_u32::<E>()?;
        let attributes = reader.read_i32::<E>()?;
        let extended_offset = reader.read_u32::<E>()?;
        let timestamp = reader.read_u64::<E>()?;

        let offset = ((extended_offset as u64) << 32) | (base_offset as u64);
        let offset = offset & 0x00FFFFFFFFFFFFFF;

        let is_compressed = compression_flag == 0x80000000;
        
        let start = offset as usize;
        let end = start + compressed_size as usize;
        let file_data = if end <= full_data.len() {
            full_data[start..end].to_vec()
        } else {
            Vec::new() // Should probably error
        };

        raw_files.push(ParFile {
            name: file_names[i].clone(),
            data: file_data,
            decompressed_size: size,
            is_compressed,
            attributes,
            timestamp,
        });
    }

    let root = build_tree(&raw_folders[0], &raw_folders, &raw_files, options)?;

    Ok(ParArchive {
        platform_id,
        endianness,
        version,
        root,
    })
}

struct RawFolderInfo {
    name: String,
    folder_count: i32,
    first_folder_index: i32,
    file_count: i32,
    first_file_index: i32,
    attributes: i32,
    #[allow(dead_code)]
    unused: [i32; 3],
}

fn build_tree(
    raw_folder: &RawFolderInfo,
    all_folders: &[RawFolderInfo],
    all_files: &[ParFile],
    options: &ReaderOptions,
) -> Result<ParDirectory> {
    let mut dir = ParDirectory {
        name: raw_folder.name.clone(),
        children: Vec::new(),
        attributes: raw_folder.attributes,
    };

    let folder_start = raw_folder.first_folder_index as usize;
    let folder_end = folder_start + raw_folder.folder_count as usize;
    for i in folder_start..folder_end {
        if i < all_folders.len() {
            let child_dir = build_tree(&all_folders[i], all_folders, all_files, options)?;
            dir.children.push(ParNode::Directory(child_dir));
        }
    }

    let file_start = raw_folder.first_file_index as usize;
    let file_end = file_start + raw_folder.file_count as usize;
    for i in file_start..file_end {
        if i < all_files.len() {
            let file = all_files[i].clone();
            
            if options.recursive && file.name.to_lowercase().ends_with(".par") {
                // Try to parse as archive
                let file_content = if file.is_compressed {
                    sllz::decompress(&file.data)?
                } else {
                    file.data.clone()
                };

                if let Ok(nested_archive) = read(&file_content, options) {
                    // Update the root name of nested archive to match the filename
                    let mut archive = nested_archive;
                    archive.root.name = file.name.clone();
                    dir.children.push(ParNode::Archive(archive));
                    continue;
                }
            }
            
            dir.children.push(ParNode::File(file));
        }
    }

    Ok(dir)
}

fn decode_name(bytes: &[u8; 64]) -> String {
    let len = bytes.iter().position(|&b| b == 0).unwrap_or(64);
    let (decoded, _, _) = WINDOWS_1252.decode(&bytes[..len]);
    let name = decoded.into_owned();
    if name.is_empty() {
        ".".to_string()
    } else {
        name
    }
}
