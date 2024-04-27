use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use anyhow::Context;
use crate::docker::image::MEGAPHONE_IMAGE_NAME;

fn run_command(command: &mut Command) -> anyhow::Result<()> {
    let out = command.output()?;
    if out.stdout.len() > 0 {
        match String::from_utf8(out.stdout) {
            Ok(msg) => println!("[{}] {msg}", command.get_program().to_str().unwrap_or_default()),
            Err(err) => eprintln!("Could not parse stderr - {err}"),
        }

    }
    if out.stderr.len() > 0 {
        match String::from_utf8(out.stderr) {
            Ok(msg) => eprintln!("[{}] {msg}", command.get_program().to_str().unwrap_or_default()),
            Err(err) => eprintln!("Could not parse stderr - {err}"),
        }
    }
    if !out.status.success() {
        anyhow::bail!("Command execution failed with code {:?}", out.status);
    }
    Ok(())
}

pub fn build_images(airgap_dir: &Path) -> anyhow::Result<()> {
    let out_file = airgap_dir.join("megaphone.tar");
    if !out_file.is_file() {
        let dockerfile_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("docker")
            .join("Dockerfile");

        run_command(Command::new("docker")
            .arg("build")
            .arg("-f")
            .arg(dockerfile_path)
            .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")))
            .arg("-t")
            .arg(MEGAPHONE_IMAGE_NAME)
        )?;

        run_command(Command::new("docker")
            .arg("save")
            .arg(MEGAPHONE_IMAGE_NAME)
            .arg("-o")
            .arg(out_file.clone())
        )?;
    }

    println!("Airgap images:");
    for path in fs::read_dir(airgap_dir).context("Error reading dir")? {
        println!("- '{}'", path.context("Error in dir entry")?.path().file_name().unwrap_or_default().to_str().unwrap_or_default());
    }
    Ok(())
}