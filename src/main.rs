use clap::{Parser, Subcommand};
use cli_table::{
    Cell as _, Table as _,
    format::{HorizontalLine, Separator, VerticalLine},
};
use preset::UF2_PRESETS;
use std::path::PathBuf;

mod deploy;
mod preset;
mod uf2;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    commands: Commands,
}

#[derive(Clone, Debug, Subcommand)]
pub enum Commands {
    /// Generate UF2 file from ELF file and optionally deploy it.
    Deploy {
        /// UF2 family name or hex.
        /// To see available families, run `list-families` subcommand.
        #[arg(long, short)]
        family: String,

        /// Base address of the binary. Usually you don't need to specify this as this is
        /// automatically read from the ELF file.
        ///
        /// By specifying this, you can override the base address.
        #[arg(long, short)]
        base_addr: Option<String>,

        /// Path to deploy uf2.
        /// If not specified, uf2 will be generated but not deployed.
        /// If 'auto' is specified, it will be deployed to the first connected device.
        #[arg(long, short, group = "deploy")]
        path: Option<String>,

        /// Retry count for deploying the binary.
        #[arg(long, default_value_t = 40)]
        deploy_retry_count: u32,

        /// Path of elf file. Usually passed by `cargo run`.
        elf_path: String,
    },
    /// Show available UF2 families.
    ListFamilies,
}

pub fn main() -> anyhow::Result<()> {
    let args = Cli::parse();

    match args.commands {
        Commands::Deploy {
            family,
            base_addr,
            path,
            deploy_retry_count,
            elf_path,
        } => {
            let family = if let Some(preset) = UF2_PRESETS.get(&family) {
                preset.id
            } else {
                parse_int(&family)?
            };
            let base_addr = base_addr.map(|s| parse_int(&s)).transpose()?;

            eprintln!(
                "ELF file is generated at: {} ({})",
                elf_path,
                get_bytes(&elf_path)
            );

            let elf_path = dunce::canonicalize(PathBuf::from(elf_path))?;

            let uf2_path = uf2::elf2uf2(&elf_path, family, base_addr)?;

            if let Some(deploy_path) = path {
                deploy::deploy_uf2(deploy_path, uf2_path, deploy_retry_count)?;
            } else {
                eprintln!("Path is not specified. Skipping deploy.",);
            }
        }
        Commands::ListFamilies => {
            let table = UF2_PRESETS
                .iter()
                .map(|(k, v)| {
                    let id = format!("{:#010x}", v.id);
                    vec![k.cell(), id.cell(), v.description.as_str().cell()]
                })
                .collect::<Vec<_>>()
                .table()
                .title(vec!["Name".cell(), "Address".cell(), "Description".cell()])
                .separator(
                    Separator::builder()
                        .row(None)
                        .column(Some(VerticalLine::default()))
                        .title(Some(HorizontalLine::default()))
                        .build(),
                );

            cli_table::print_stdout(table)?;
        }
    }

    Ok(())
}

fn get_bytes(p: impl AsRef<std::path::Path>) -> String {
    human_bytes::human_bytes(std::fs::metadata(p).map(|m| m.len()).unwrap_or(0) as f64)
}

fn parse_int(s: &str) -> std::result::Result<u32, std::num::ParseIntError> {
    if let Some(s) = s.strip_prefix("0x") {
        u32::from_str_radix(s, 16)
    } else if let Some(s) = s.strip_prefix("0o") {
        u32::from_str_radix(s, 8)
    } else if let Some(s) = s.strip_prefix("0b") {
        u32::from_str_radix(s, 2)
    } else {
        s.parse::<u32>()
    }
}
