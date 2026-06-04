// -------------------------------------------------------
// © Kaplas. Licensed under MIT. See LICENSE for details.
// -------------------------------------------------------
use anyhow::{Result, Context};
use clap::{Parser, Subcommand};
use par_lib::parc::{self, reader, writer, ParArchive, ParNode};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Direct path to extract (shortcut for extract command)
    #[arg(index = 1)]
    input: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// List contents of a PAR archive
    List {
        #[arg(required = true)]
        archive: String,
    },
    /// Extract contents from a PAR archive
    Extract {
        #[arg(required = true)]
        archive: String,
        #[arg(required = true)]
        output: String,
        #[arg(short, long)]
        recursive: bool,
    },
    /// Create a PAR archive from a directory
    Create {
        #[arg(required = true)]
        input: String,
        #[arg(required = true)]
        archive: String,
        #[arg(short, long, default_value_t = 1)]
        compression: u8,
    },
    /// Add files to a PAR archive
    Add {
        #[arg(required = true)]
        archive: String,
        #[arg(required = true)]
        input: String,
    },
    /// Remove files from a PAR archive
    Remove {
        #[arg(required = true)]
        archive: String,
        #[arg(required = true)]
        path: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::List { archive }) => list(&archive),
        Some(Commands::Extract { archive, output, recursive }) => extract(&archive, &output, recursive),
        Some(Commands::Create { input, archive, compression }) => create(&input, &archive, compression),
        Some(Commands::Add { archive, input }) => add(&archive, &input),
        Some(Commands::Remove { archive, path }) => remove(&archive, &path),
        None => {
            if let Some(input) = cli.input {
                let path = Path::new(&input);
                if path.is_file() {
                    let output = format!("{}.unpack", input);
                    extract(&input, &output, false)
                } else if path.is_dir() {
                    let output = format!("{}.par", input.trim_end_matches('/'));
                    create(&input, &output, 1)
                } else {
                    println!("Error: Input path not found.");
                    Ok(())
                }
            } else {
                use clap::CommandFactory;
                Cli::command().print_help()?;
                Ok(())
            }
        }
    }
}

fn list(archive_path: &str) -> Result<()> {
    let data = std::fs::read(archive_path)?;
    let options = reader::ReaderOptions { recursive: true };
    let archive = reader::read(&data, &options)?;

    println!("Listing contents of {}:", archive_path);
    print_node(&archive.root.children, 0);
    Ok(())
}

fn print_node(nodes: &[ParNode], indent: usize) {
    let indent_str = "  ".repeat(indent);
    for node in nodes {
        match node {
            ParNode::File(f) => println!("{}{}", indent_str, f.name),
            ParNode::Directory(d) => {
                println!("{}{}/", indent_str, d.name);
                print_node(&d.children, indent + 1);
            }
            ParNode::Archive(a) => {
                println!("{}{}/ (Nested PAR)", indent_str, a.root.name);
                print_node(&a.root.children, indent + 1);
            }
        }
    }
}

fn extract(archive_path: &str, output_path: &str, recursive: bool) -> Result<()> {
    let path = Path::new(archive_path);

    if path.is_dir() {
        // New feature: extract all pars in directory
        println!("Extracting all PAR files in directory: {}", archive_path);
        for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() && entry.path().extension().and_then(|s| s.to_str()) == Some("par") {
                let relative = entry.path().strip_prefix(path)?;
                let item_output = Path::new(output_path).join(relative).with_extension("unpack");
                println!("Extracting {} to {}...", entry.path().display(), item_output.display());
                extract_single(entry.path().to_str().unwrap(), item_output.to_str().unwrap(), recursive)?;
            }
        }
    } else {
        extract_single(archive_path, output_path, recursive)?;
    }
    Ok(())
}

fn extract_single(archive_path: &str, output_path: &str, recursive: bool) -> Result<()> {
    let data = std::fs::read(archive_path)?;
    let options = reader::ReaderOptions { recursive };
    let archive = reader::read(&data, &options)?;

    std::fs::create_dir_all(output_path)?;
    extract_node(&archive.root.children, Path::new(output_path))?;
    Ok(())
}

fn extract_node(nodes: &[ParNode], base_path: &Path) -> Result<()> {
    for node in nodes {
        match node {
            ParNode::File(f) => {
                let path = base_path.join(&f.name);
                let file_data = if f.is_compressed {
                    par_lib::sllz::decompress(&f.data).context("Failed to decompress file")?
                } else {
                    f.data.clone()
                };
                
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&path, file_data)?;
                // Set attributes and timestamps if possible (omitted for brevity in first pass)
            }
            ParNode::Directory(d) => {
                let path = base_path.join(&d.name);
                std::fs::create_dir_all(&path)?;
                extract_node(&d.children, &path)?;
            }
            ParNode::Archive(a) => {
                let path = base_path.join(&a.root.name);
                std::fs::create_dir_all(&path)?;
                extract_node(&a.root.children, &path)?;
            }
        }
    }
    Ok(())
}

fn create(input_path: &str, archive_path: &str, compression: u8) -> Result<()> {
    // Basic implementation: walk directory and build archive
    let root_path = Path::new(input_path);
    let root_name = root_path.file_name().unwrap().to_str().unwrap().to_string();
    let root_dir = build_dir_recursive(root_path, root_name)?;

    let archive = ParArchive {
        platform_id: 2, // Default
        endianness: 1,  // Default Big Endian
        version: 0x00020001,
        root: root_dir,
    };

    let options = writer::WriterOptions { compressor_version: compression };
    let data = writer::write(&archive, &options)?;
    std::fs::write(archive_path, data)?;
    println!("Created archive: {}", archive_path);
    Ok(())
}

fn build_dir_recursive(path: &Path, name: String) -> Result<parc::ParDirectory> {
    let mut dir = parc::ParDirectory::new(name);
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let name = entry.file_name().to_str().unwrap().to_string();
        if file_type.is_dir() {
            dir.children.push(ParNode::Directory(build_dir_recursive(&entry.path(), name)?));
        } else {
            let data = std::fs::read(entry.path())?;
            dir.children.push(ParNode::File(parc::ParFile {
                name,
                data: data.clone(),
                decompressed_size: data.len() as u32,
                is_compressed: false,
                attributes: 0x20,
                timestamp: 0, // Should set actual timestamp
            }));
        }
    }
    Ok(dir)
}

fn add(_archive_path: &str, _input_path: &str) -> Result<()> {
    println!("Add command not yet fully implemented in this draft.");
    Ok(())
}

fn remove(_archive_path: &str, _path: &str) -> Result<()> {
    println!("Remove command not yet fully implemented in this draft.");
    Ok(())
}
