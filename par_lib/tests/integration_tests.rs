// -------------------------------------------------------
// © Kaplas. Licensed under MIT. See LICENSE for details.
// -------------------------------------------------------
use par_lib::parc::{reader, writer, ParArchive, ParDirectory, ParFile, ParNode};
use std::fs;

#[test]
fn test_parc_roundtrip() -> anyhow::Result<()> {
    // 1. Setup a dummy directory structure
    let test_dir = "test_roundtrip_data";
    fs::create_dir_all(format!("{}/subdir", test_dir))?;
    fs::write(format!("{}/file1.txt", test_dir), b"Hello World")?;
    fs::write(format!("{}/subdir/file2.bin", test_dir), vec![0xDE, 0xAD, 0xBE, 0xEF])?;

    // 2. Build ParArchive structure manually for creation
    let mut root = ParDirectory::new("root".to_string());
    
    root.children.push(ParNode::File(ParFile {
        name: "file1.txt".to_string(),
        data: b"Hello World".to_vec(),
        decompressed_size: 11,
        is_compressed: false,
        attributes: 0x20,
        timestamp: 123456789,
    }));

    let mut subdir = ParDirectory::new("subdir".to_string());
    subdir.children.push(ParNode::File(ParFile {
        name: "file2.bin".to_string(),
        data: vec![0xDE, 0xAD, 0xBE, 0xEF],
        decompressed_size: 4,
        is_compressed: false,
        attributes: 0x20,
        timestamp: 123456789,
    }));
    root.children.push(ParNode::Directory(subdir));

    let archive = ParArchive {
        platform_id: 2,
        endianness: 1, // Big Endian
        version: 0x00020001,
        root,
    };

    // 3. Write to binary
    let options = writer::WriterOptions { compressor_version: 1 };
    let par_data = writer::write(&archive, &options)?;

    // 4. Read back
    let read_options = reader::ReaderOptions { recursive: false };
    let read_archive = reader::read(&par_data, &read_options)?;

    // 5. Verify structure (Note: Writer flattens directories then files)
    assert_eq!(read_archive.root.children.len(), 2);
    
    let d1 = match &read_archive.root.children[0] {
        ParNode::Directory(d) => d,
        _ => panic!("Expected directory at index 0"),
    };
    assert_eq!(d1.name, "subdir");
    assert_eq!(d1.children.len(), 1);

    let f1 = match &read_archive.root.children[1] {
        ParNode::File(f) => f,
        _ => panic!("Expected file at index 1"),
    };
    assert_eq!(f1.name, "file1.txt");
    assert_eq!(f1.data, b"Hello World");

    // Cleanup
    fs::remove_dir_all(test_dir)?;

    Ok(())
}

#[test]
fn test_sllz_v1_large() -> anyhow::Result<()> {
    // Test with larger data to ensure sliding window works correctly
    let mut input = Vec::new();
    for i in 0..10000 {
        input.push((i % 256) as u8);
    }
    // Add some repeating patterns
    for _ in 0..10 {
        input.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    }

    let compressed = par_lib::sllz::compress(&input, 1, par_lib::sllz::Endianness::BigEndian)?;
    let decompressed = par_lib::sllz::decompress(&compressed)?;

    assert_eq!(input, decompressed);
    Ok(())
}
