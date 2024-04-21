use std::path::{Path, PathBuf};
use std::process::Command;
use crate::docker::image::MEGAPHONE_IMAGE_NAME;

pub fn build_images(airgap_dir: &Path) {
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
            .expect("Error building megaphone image");

        Command::new("docker")
            .arg("save")
            .arg(MEGAPHONE_IMAGE_NAME)
            .arg("-o")
            .arg(out_file)
            .output()
            .expect("Error saving megaphone image");
    }
}