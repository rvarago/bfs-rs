//! A readonly FUSE where entries mirror objects stored in cloud buckets.

pub mod backends;
mod fs;

use backends::BlockingConnection;
use eyre::{eyre, Context};
use fs::BucketFilesystem;
use fuser::MountOption;
use log::info;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use tokio::runtime::Runtime;

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(rename = "source")]
    pub source: SourceOptions,
    #[serde(rename = "filesystem")]
    pub filesystem: FilesystemOptions,
    #[serde(rename = "backend")]
    pub backend: BackendOptions,
}

#[derive(Debug, Deserialize)]
pub struct SourceOptions {
    #[serde(rename = "bucket")]
    pub bucket_name: String,
}

#[derive(Debug, Deserialize)]
pub struct FilesystemOptions {
    #[serde(rename = "mountpoint")]
    pub mountpoint: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "provider")]
pub enum BackendOptions {
    #[serde(rename = "aws")]
    Aws(backends::aws::Options),
}

impl Config {
    /// Loads the configuration at `path`.
    pub fn load_from(path: impl AsRef<Path>) -> eyre::Result<Self> {
        let cfg = config::ConfigBuilder::<config::builder::DefaultState>::default()
            .add_source(config::File::from(path.as_ref()))
            .build()
            .wrap_err("unable to load from source")?;

        cfg.try_deserialize().wrap_err("unable to deserialize")
    }
}

/// Starts the filesystem, mounting it at the specified location.
pub fn run_app(cfg: Config, rt: Runtime) -> eyre::Result<()> {
    let conn = new_connection_from(cfg.backend, rt);

    let fs = BucketFilesystem::new(cfg.source.bucket_name, conn)
        .wrap_err("unable to construct bucket fs")?;

    info!("starting bfs");

    start_fs(cfg.filesystem, fs)
}

fn start_fs(opts: FilesystemOptions, fs: BucketFilesystem) -> eyre::Result<()> {
    let mount_opts = vec![MountOption::RO, MountOption::NoExec];
    fuser::mount2(fs, &opts.mountpoint, &mount_opts).wrap_err_with(|| {
        eyre!(
            "unable to mount bucket fs at mountpoint={}",
            opts.mountpoint.display()
        )
    })
}

fn new_connection_from(opts: BackendOptions, rt: Runtime) -> BlockingConnection {
    let backend = match opts {
        BackendOptions::Aws(opts) => rt.block_on(backends::aws::AwsProvider::new(opts)),
    };

    BlockingConnection::new(backend, rt)
}
