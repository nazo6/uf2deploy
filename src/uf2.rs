use std::{
    cmp::{max, min},
    fs::File,
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
};

use anyhow::{Context, bail};
use goblin::{
    Object,
    elf::{Elf, ProgramHeader, program_header::PT_LOAD},
};

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

    // elf to bin
    let bin_path = artifact_dir.join(format!("{}.bin", artifact_name));
    elf2bin(elf_path, &bin_path)?;
    eprintln!(
        "Bin file is generated at: {} ({})",
        bin_path.display(),
        get_bytes(&bin_path)
    );

    // bin to uf2
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

// base_addr is the minimum virtual address of PT_LOAD segments.
fn get_base_addr_of_elf(path: &Path) -> anyhow::Result<u32> {
    let mut file = File::open(path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    let elf = Elf::parse(&buffer).expect("Failed to parse ELF");

    let base_address = elf
        .program_headers
        .iter()
        .filter(|ph| ph.p_type == goblin::elf::program_header::PT_LOAD)
        .map(|ph| ph.p_vaddr)
        .min()
        .unwrap_or_else(|| {
            eprintln!("WARN: No PT_LOAD segment found in ELF. Using 0 as base address.");
            0
        });

    Ok(base_address as u32)
}

// Example elf header (by readelf)
//
// Program Headers:
//   Type           Offset   VirtAddr   PhysAddr   FileSiz MemSiz  Flg Align
//                           ↓ min vaddr is used when genearting uf2 (= base addr)
//   LOAD           0x000114 0x00026000 0x00026000 0x00100 0x00100 R   0x4      ← start is located to 0
//                                      ↑ min_lma (global offset)
//   LOAD           0x000214 0x00026100 0x00026100 0x1edf0 0x1edf0 R E 0x4      ← start is located to p_addr - min_lma
//   LOAD           0x01f008 0x00044ef0 0x00044ef0 0x02c4c 0x02c4c R   0x8      ← same as above
//   LOAD           0x021c58 0x20033e10 0x00047b40 0x0001c 0x0001c RW  0x8      ← same as above
//                                      ↑ p_addr+file_sz is max_lma_end
//   LOAD           0x021c80 0x20033e30 0x20033e30 0x00000 0x0c1cc RW  0x8      ← Ignored because of p_filesz == 0
fn elf2bin(elf_path: &Path, bin_path: &Path) -> anyhow::Result<()> {
    let mut elf_file = File::open(elf_path)?;
    let mut elf_data = Vec::new();
    elf_file.read_to_end(&mut elf_data)?;

    let elf = match Object::parse(&elf_data) {
        Ok(Object::Elf(elf)) => elf,
        Ok(_) => {
            bail!("The input file is not an ELF file.");
        }
        Err(e) => {
            bail!("Failed to parse ELF file: {}", e);
        }
    };

    // Extract PT_LOAD segments with file size > 0
    let loadable_segments: Vec<&ProgramHeader> = elf
        .program_headers
        .iter()
        .filter(|phdr| phdr.p_type == PT_LOAD && phdr.p_filesz > 0)
        .collect();

    if loadable_segments.is_empty() {
        bail!("No valid PT_LOAD segments with p_filesz > 0 found in the ELF file.");
    }

    let mut min_lma = u64::MAX;
    let mut max_lma_end = 0u64;

    for phdr in &loadable_segments {
        min_lma = min(min_lma, phdr.p_paddr);
        max_lma_end = max(max_lma_end, phdr.p_paddr.saturating_add(phdr.p_filesz));
    }

    if min_lma == u64::MAX {
        bail!("Could not determine valid LMA range",);
    }

    let output_size = if max_lma_end > min_lma {
        (max_lma_end - min_lma) as usize
    } else {
        bail!("Calculated output size based on LMA is zero. Output file will be empty.",);
    };

    let mut output_buffer = vec![0u8; output_size];
    // Copy segment data into the buffer based on LMA
    for phdr in &loadable_segments {
        // p_filesz > 0 is guaranteed by the filter.
        let read_size = phdr.p_filesz as usize;
        let file_offset = phdr.p_offset as usize;
        // The starting position in the buffer is the relative offset from the overall minimum LMA.
        let buffer_offset = (phdr.p_paddr - min_lma) as usize;

        // Check if the segment data range (in the ELF file) is valid.
        if file_offset
            .checked_add(read_size)
            .is_none_or(|end| end > elf_data.len())
        {
            bail!(
                "Segment data range (offset=0x{:x}, filesz=0x{:x}) exceeds ELF file size ({} bytes).",
                phdr.p_offset,
                phdr.p_filesz,
                elf_data.len()
            );
        }

        if buffer_offset
            .checked_add(read_size)
            .is_none_or(|end| end > output_buffer.len())
        {
            bail!(
                "Segment write range (LMA=0x{:x}, FileSz=0x{:x}, buffer_offset={}) exceeds output buffer size ({} bytes). min_lma=0x{:x}, max_lma_end=0x{:x}",
                phdr.p_paddr,
                phdr.p_filesz,
                buffer_offset,
                output_buffer.len(),
                min_lma,
                max_lma_end
            );
        }

        let data_to_copy = &elf_data[file_offset..file_offset + read_size];
        output_buffer[buffer_offset..buffer_offset + read_size].copy_from_slice(data_to_copy);
    }

    let mut bin_file = File::create(bin_path).context("Could not create bin file")?;
    bin_file.write_all(&output_buffer)?;

    Ok(())
}
