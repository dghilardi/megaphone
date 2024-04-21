use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use anyhow::Context;
use crate::docker::image::MEGAPHONE_IMAGE_NAME;

pub fn build_images(airgap_dir: &Path) -> anyhow::Result<()> {
    let out_file = airgap_dir.join("megaphone.tar");
    if !out_file.is_file() {
        Command::new("docker")
            .arg("build")
            .arg("-f")
            .arg("dockerfile/Dockerfile")
            .arg(".")
            .arg("-t")
            .arg(MEGAPHONE_IMAGE_NAME)
            .output()
            .context("Error building megaphone image")?;

        Command::new("docker")
            .arg("save")
            .arg(MEGAPHONE_IMAGE_NAME)
            .arg("-o")
            .arg(out_file.clone())
            .output()
            .context("Error saving megaphone image")?;

        Command::new("chmod")
            .arg("a+r")
            .arg(out_file)
            .output()
            .context("Error changing permissions")?;
    }

    println!("Airgap images:");
    for path in fs::read_dir(airgap_dir).context("Error reading dir")? {
        println!("- '{}'", path.context("Error in dir entry")?.path().file_name().unwrap_or_default().to_str().unwrap_or_default());
    }
    Ok(())
}