//! A bevy [`AssetIo`](bevy::asset::AssetIo) implementation that allows reading from
//! tar files, that are zstd compressed.
//!
//! Use the crate bevy_assets_tar_zstd_bundler to bundle the assets in an folder at build time.
//!
//! Example:
//!
//! ```ignore
//! bevy_assets_tar_zstd_bundler::bundle_asset(bevy_assets_tar_zstd_bundler::Config::default());
//! ```
//!
//! This will read assets from the `assets` folder an write them into `./target/assets.bin`.

use bevy::asset::AssetIoError;
use bevy::{asset::AssetIo, prelude::*};
use std::io::prelude::*;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

#[derive(Clone, Debug)]
pub struct AssetsTarZstdConfig {
    /// The name of the asset directory (the resoulting .bin file without the extension)
    pub name: String,
}

impl Default for AssetsTarZstdConfig {
    fn default() -> Self {
        Self {
            name: "assets".into(),
        }
    }
}

#[derive(Debug)]
enum Message {
    /// Read file (hopefully)
    RequestFile(String, mpsc::Sender<Option<Vec<u8>>>),
    /// Read metadata of file or directory
    RequestMetadata(String, mpsc::Sender<Option<bevy::asset::Metadata>>),
    /// Read files in a directory
    RequestDirFiles(String, mpsc::Sender<Option<Vec<PathBuf>>>),
}

struct AssetsTarZstd {
    tx: Arc<Mutex<mpsc::Sender<Message>>>,
    task: thread::JoinHandle<()>,
}

fn find<'b, 'c, 'd>(
    archive: &'b mut tar::Archive<zstd::Decoder<'c, std::io::BufReader<std::fs::File>>>,
    path: &str,
) -> Option<tar::Entry<'b, zstd::Decoder<'c, std::io::BufReader<std::fs::File>>>> {
    archive
        .entries()
        .ok()?
        .into_iter()
        .filter_map(|e| e.ok())
        .find(|e| *e.path().unwrap().to_string_lossy() == *path)
}

fn read_bytes(
    archive: &mut tar::Archive<zstd::Decoder<std::io::BufReader<std::fs::File>>>,
    path: &str,
) -> Option<Vec<u8>> {
    let mut entry = find(archive, path)?;

    let mut buffer = Vec::new();
    entry.read_to_end(&mut buffer).ok()?;

    Some(buffer)
}

fn read_metadata(
    archive: &mut tar::Archive<zstd::Decoder<std::io::BufReader<std::fs::File>>>,
    path: &str,
) -> Option<bevy::asset::Metadata> {
    let entry = find(archive, path)?;

    let file_type = match entry.header().entry_type() {
        tar::EntryType::Regular => bevy::asset::FileType::File,
        tar::EntryType::Directory => bevy::asset::FileType::Directory,
        _ => return None,
    };

    Some(bevy::asset::Metadata::new(file_type))
}

fn read_dir_files(
    archive: &mut tar::Archive<zstd::Decoder<std::io::BufReader<std::fs::File>>>,
    path: &str,
) -> Option<Vec<PathBuf>> {
    let mut result: Vec<PathBuf> = archive
        .entries()
        .ok()?
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_ok())
        .filter(|e| e.path().unwrap().parent().is_some())
        .filter(|e| *e.path().unwrap().parent().unwrap().to_string_lossy() == *path)
        .map(|e| e.path().unwrap().into())
        .collect();

    result.sort();
    Some(result)
}

fn spawn_async(
    config: AssetsTarZstdConfig,
) -> (Arc<Mutex<mpsc::Sender<Message>>>, thread::JoinHandle<()>) {
    let (tx, rx) = mpsc::channel();
    let task = thread::spawn(move || {
        // Search the assets.bin file
        let paths = [
            std::env::current_exe()
                .unwrap()
                .join(format!("{}.bin", config.name)),
            std::env::current_exe()
                .unwrap()
                .parent()
                .expect("Expected parent path relative to executable")
                .join(format!("{}.bin", config.name)),
        ];

        let archive_path = paths
            .into_iter()
            .find(|p| p.is_file())
            .expect("Expected assets.bin file");

        info!("Assets archive path: {}", archive_path.to_string_lossy());

        // Open archive
        fn open_archive(
            path: &PathBuf,
        ) -> tar::Archive<zstd::Decoder<'static, std::io::BufReader<std::fs::File>>> {
            let file_reader = std::fs::File::open(path.clone())
                .expect(format!("Expected {} file", path.to_string_lossy().to_string()).as_str());
            let decoder = zstd::Decoder::new(file_reader).expect("Expected valid zstd encoding");
            return tar::Archive::new(decoder);
        }

        info!("Started asset loader");

        while let Ok(msg) = rx.recv() {
            match msg {
                Message::RequestFile(path, result) => {
                    debug!("Requested file {}", path);
                    let mut archive = open_archive(&archive_path);

                    result
                        .send(read_bytes(&mut archive, path.as_str()))
                        .unwrap_or_else(|err| error!("{}", err));
                }
                Message::RequestMetadata(path, result) => {
                    debug!("Requested metadata of file {}", path);
                    let mut archive = open_archive(&archive_path);
                    result
                        .send(read_metadata(&mut archive, path.as_str()))
                        .unwrap_or_else(|err| error!("{}", err));
                }
                Message::RequestDirFiles(path, result) => {
                    debug!("Requested files in directory {}", path);
                    let mut archive = open_archive(&archive_path);
                    result
                        .send(read_dir_files(&mut archive, path.as_str()))
                        .unwrap_or_else(|err| error!("{}", err));
                }
            }
        }
    });

    (Arc::new(Mutex::new(tx)), task)
}

impl AssetsTarZstd {
    fn new(config: AssetsTarZstdConfig) -> Self {
        let (tx, task) = spawn_async(config);
        Self { tx, task }
    }
}

impl AssetIo for AssetsTarZstd {
    fn is_dir(&self, path: &std::path::Path) -> bool {
        if let Ok(result) = self.get_metadata(path) {
            result.is_dir()
        } else {
            false
        }
    }

    fn is_file(&self, path: &std::path::Path) -> bool {
        if let Ok(result) = self.get_metadata(path) {
            result.is_file()
        } else {
            false
        }
    }

    fn load_path<'a>(
        &'a self,
        path: &'a std::path::Path,
    ) -> bevy::utils::BoxedFuture<'a, anyhow::Result<Vec<u8>, bevy::asset::AssetIoError>> {
        let tx = self.tx.clone();
        let path = path.to_string_lossy().to_string();
        Box::pin(async move {
            let (tx_file, rx_file) = mpsc::channel();
            if let Err(_) = tx
                .lock()
                .unwrap()
                .send(Message::RequestFile(path.clone(), tx_file))
            {
                Err(AssetIoError::NotFound(path.into()))
            } else {
                rx_file
                    .recv()
                    .unwrap()
                    .ok_or(bevy::asset::AssetIoError::NotFound(path.into()))
            }
        })
    }

    fn get_metadata(
        &self,
        path: &std::path::Path,
    ) -> anyhow::Result<bevy::asset::Metadata, bevy::asset::AssetIoError> {
        let (tx_file, rx_file) = mpsc::channel();
        let path = path.to_string_lossy().to_string();
        if let Err(_) = self
            .tx
            .lock()
            .unwrap()
            .send(Message::RequestMetadata(path.clone(), tx_file))
        {
            Err(AssetIoError::NotFound(path.into()))
        } else {
            rx_file
                .recv()
                .unwrap()
                .ok_or(bevy::asset::AssetIoError::NotFound(path.into()))
        }
    }

    fn read_directory(
        &self,
        path: &std::path::Path,
    ) -> anyhow::Result<Box<dyn Iterator<Item = std::path::PathBuf>>, bevy::asset::AssetIoError>
    {
        let (tx_file, rx_file) = mpsc::channel();
        let path = path.to_string_lossy().to_string();
        if let Err(_) = self
            .tx
            .lock()
            .unwrap()
            .send(Message::RequestDirFiles(path.clone(), tx_file))
        {
            Err(AssetIoError::NotFound(path.into()))
        } else {
            match rx_file
                .recv()
                .unwrap()
                .ok_or(bevy::asset::AssetIoError::NotFound(path.into()))
            {
                Ok(result) => Ok(Box::new(result.into_iter())),
                Err(err) => Err(err),
            }
        }
    }

    fn watch_for_changes(&self) -> anyhow::Result<(), bevy::asset::AssetIoError> {
        Ok(())
    }

    fn watch_path_for_changes(
        &self,
        path: &std::path::Path,
    ) -> anyhow::Result<(), bevy::asset::AssetIoError> {
        Ok(())
    }
}

//    bevy::asset::create_platform_default_asset_io

#[derive(Default)]
pub struct AssetsTarZstdPlugin {
    config: AssetsTarZstdConfig,
}

impl AssetsTarZstdPlugin {
    pub fn new(config: AssetsTarZstdConfig) -> Self {
        Self { config }
    }
}

impl Plugin for AssetsTarZstdPlugin {
    fn build(&self, app: &mut App) {
        #[cfg(info_assertions)]
        {
            warn!("Assets from tar.zstd file disabled in Debug build");
            return;
        }

        let io = AssetsTarZstd::new(self.config.clone());
        app.insert_resource(AssetServer::new(io));
    }
}
