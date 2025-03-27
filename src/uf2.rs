use std::{
    fs::File,
    io::Read as _,
    path::{Path, PathBuf},
};

use anyhow::Context as _;
use goblin::elf::Elf;

use crate::get_bytes;

pub fn elf2uf2(
    elf_path: &std::path::Path,
    family_id: u32,
    base_addr: Option<u32>,
) -> anyhow::Result<PathBuf> {
    let base_addr = if let Some(base_addr) = base_addr {
        base_addr
    } else {
        get_base_addr_of_elf(elf_path)?
    };
    eprintln!(
        "Generating UF2. Family: 0x{:08x}, Base Address: 0x{:08x}",
        family_id, base_addr
    );

    let artifact_dir = elf_path.parent().context("No parent dir in output file")?;
    let artifact_name = elf_path
        .file_stem()
        .context("No file stem in output file")?
        .to_string_lossy();

    // convert elf to bin
    let bin_path = artifact_dir.join(format!("{}.bin", artifact_name));
    duct::cmd!("rust-objcopy", "-O", "binary", &elf_path, &bin_path)
        .run()
        .context("Failed to convert to bin. Is cargo-binutils and llvm-tools installed?")?;
    eprintln!(
        "Bin file is generated at: {} ({})",
        bin_path.display(),
        get_bytes(&bin_path)
    );

    let uf2_path = artifact_dir.join(format!("{}.uf2", artifact_name));
    let uf2_data = uf2::bin_to_uf2(&std::fs::read(bin_path)?, family_id, base_addr)?;
    std::fs::write(&uf2_path, uf2_data).context("Failed to write uf2 file")?;
    eprintln!(
        "Uf2 file is generated at: {} ({})",
        uf2_path.display(),
        get_bytes(&uf2_path)
    );

    Ok(uf2_path)
}

fn get_base_addr_of_elf(path: &Path) -> anyhow::Result<u32> {
    let mut file = File::open(path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    // ELF を解析
    let elf = Elf::parse(&buffer).expect("Failed to parse ELF");

    // LOAD セグメントの最小の仮想アドレスを取得
    let base_address = elf
        .program_headers
        .iter()
        .filter(|ph| ph.p_type == goblin::elf::program_header::PT_LOAD) // LOAD セグメントのみ
        .map(|ph| ph.p_vaddr)
        .min()
        .unwrap_or_else(|| {
            eprintln!("WARN: No PT_LOAD segment found in ELF. Using 0 as base address.");
            0
        });

    Ok(base_address as u32)
}
