use std::path::{Path, PathBuf};

pub fn deploy_uf2(
    deploy_path_args: String,
    uf2_path: PathBuf,
    deploy_retry_count: u32,
) -> anyhow::Result<()> {
    let bar = indicatif::ProgressBar::new(0)
        .with_style(
            indicatif::ProgressStyle::with_template(
                "{prefix} [{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
            )
            .unwrap(),
        )
        .with_position(0);

    let deploy_path = 'abandoned: {
        for i in 0..deploy_retry_count {
            if i > 0 {
                std::thread::sleep(std::time::Duration::from_millis(500));
            }

            bar.set_message(format!(
                "Deploying (attempt {}/{})",
                i + 1,
                deploy_retry_count
            ));

            let Ok(deploy_path) = get_uf2_deploy_path(deploy_path_args.clone(), &uf2_path) else {
                continue;
            };

            match fs_extra::file::copy_with_progress(
                &uf2_path,
                &deploy_path,
                &fs_extra::file::CopyOptions::new(),
                |p| {
                    bar.set_length(p.total_bytes);
                    bar.set_position(p.copied_bytes);
                },
            ) {
                Ok(_) => {
                    bar.finish();

                    eprintln!("Success");
                    break 'abandoned deploy_path;
                }
                Err(_e) => {}
            }
        }
        bar.abandon_with_message("Abandoned");
        anyhow::bail!("Failed to copy the uf2 file to the deploy directory");
    };

    eprintln!("Copied UF2 file to {}", deploy_path.display());

    Ok(())
}

fn get_uf2_deploy_path(deploy_path: String, uf2_path: &Path) -> anyhow::Result<PathBuf> {
    let deploy_dir = if deploy_path == "auto" {
        // search mount that have "INFO_UF2.txt" file

        let mut deploy_dir = None;
        for disk in sysinfo::Disks::new_with_refreshed_list().iter() {
            let path = disk.mount_point().to_path_buf();
            if path.join("INFO_UF2.TXT").exists() {
                deploy_dir = Some(path);
                break;
            }
        }
        if let Some(deploy_dir) = deploy_dir {
            deploy_dir
        } else {
            anyhow::bail!("No mount found that have INFO_UF2.TXT file");
        }
    } else {
        PathBuf::from(deploy_path)
    };
    Ok(deploy_dir.join(uf2_path.file_name().unwrap()))
}
