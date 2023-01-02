//! Crate for supporting the `bevy_assets_tar_zstd` crate.
//!
//! This generates an `assets.bin` in the folder next-to or the parent directory of the generated
//! executable by cargo.
//!
//! Use this in your build scripts `build.rs` file:
//!
//! ```ignore
//! fn main() {
//!     bevy_assets_tar_zstd_bundler::bundle_asset(bevy_assets_tar_zstd_bundler::Config::default());
//! }
//! ```
//!
//! You have to add `bevy_assets_tar_zstd_bundler` as a build dependency in Cargo.toml
//!
//! ```toml
//! [build-dependencies]
//! bevy_assets_tar_zstd_bundler = { version = "0" }
//! ```
use std::fs::File;

/// The configuration for bundeling the assets
pub struct Config {
    /// Path to compress into target `name`.bin (default: assets)
    pub name: String,
    /// Path where the `name`.bin file should be written to. This path is relative to the OUR_DIR
    /// and tries by default to write into the same folder as the executable.
    pub target_dir: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            name: "assets".to_string(),
            target_dir: "../../..".to_string(),
        }
    }
}

/// This bundles the assets in the target folder (default: assets)
pub fn bundle_asset(config: Config) {
    let src_dir = format!("./{}", config.name);
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", src_dir);
    println!("cargo:rerun-if-env-changed=OUT_DIR");

    let src_dir_path = std::path::Path::new(&src_dir);
    let target_archive = format!(
        "{}/{}/{}.bin",
        std::env::var("OUT_DIR").expect("Cargo promised the OUT_DIR environment variable"),
        config.target_dir,
        config.name
    );
    let target_archive_path = std::path::Path::new(&target_archive);
    /*println!(
        "cargo:warning=Target path: {}",
        target_archive_path.to_string_lossy()
    );*/

    write_to_archive(target_archive_path, src_dir_path).unwrap();
}

fn write_to_archive(
    target_archive_path: &std::path::Path,
    src_dir_path: &std::path::Path,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(target_archive_path.parent().ok_or(anyhow::anyhow!(""))?).ok(); // Create the target path
    std::fs::remove_file(target_archive_path).ok();

    let file_writer = File::create(target_archive_path)?;
    let mut archive = tar::Builder::new(zstd::Encoder::new(file_writer, 12)?.auto_finish());
    archive.mode(tar::HeaderMode::Deterministic);
    archive.follow_symlinks(false);

    archive.append_dir_all(".", src_dir_path)?;

    Ok(())
}
