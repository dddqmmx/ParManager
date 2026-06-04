# ParManager (Rust Implementation)

Tools for Yakuza PAR archives, rewritten in Rust.

Original project by [Kaplas](https://github.com/Kaplas80/ParManager).

## Features
- Read and write Yakuza PAR archives.
- Support for ***SLLZ*** compression (including ***SLLZ V2*** used in Yakuza Kiwami 2).
- Faster and cross-platform.

## Usage

### ParTool

#### List mode
`par_tool list <archive.par> [-r]`

Reads a PAR archive and shows its contents.
`-r` parameter enables *recursive* mode and shows the contents of nested PAR archives.

#### Extraction mode
`par_tool extract <archive.par> <output_directory> [-r]`

Extracts the PAR archive contents to the specified directory.
`-r` parameter enables *recursive* mode and extracts the contents of nested PAR archives.

#### Creation mode
`par_tool create <input_directory> <archive.par> [-c compression_mode] [--alternative-mode]`

Creates a new PAR archive with the contents of the specified directory.
`-c` parameter sets the SLLZ compression version to use:
- `0`: No compression.
- `1`: Default (supported in all Yakuza games).
- `2`: Supported in Yakuza Kiwami 2.

Set `--alternative-mode` for Yakuza 3, 4, 5, or Kenzan archives.

## License
MIT License. See [LICENSE](LICENSE) for details.
Original work © 2019 Kaplas80.
