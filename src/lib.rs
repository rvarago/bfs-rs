//! A readonly FUSE where entries mirror objects stored in cloud buckets.

pub mod backends;
mod fs;

use backends::BucketConnection;
use eyre::{eyre, Context};
use fs::BucketFilesystem;
use fuser::MountOption;
use log::info;
use std::path::PathBuf;
use tokio::runtime::Runtime;

#[derive(Debug)]
pub struct Config {
    pub bucket_name: String,
    pub mountpoint: PathBuf,

    pub backend: BackendOptions,
}

#[derive(Debug)]
pub enum BackendOptions {
    Aws(backends::aws::Options),
}

// Starts the filesystem, mounting it at the specified location.
pub fn run_app(cfg: Config, rt: Runtime) -> eyre::Result<()> {
    let conn = new_backend_from(cfg.backend, rt);

    let fs = BucketFilesystem::new(cfg.bucket_name, conn)
        .wrap_err_with(|| eyre!("unable to construct bucket fs"))?;

    info!("starting bfs");

    let mount_opts = vec![MountOption::RO, MountOption::NoExec];
    fuser::mount2(fs, &cfg.mountpoint, &mount_opts).wrap_err_with(|| {
        eyre!(
            "unable to mount bucket fs at mountpoint={}",
            cfg.mountpoint.display()
        )
    })
}

fn new_backend_from(opts: BackendOptions, rt: Runtime) -> BucketConnection {
    let backend = match opts {
        BackendOptions::Aws(opts) => rt.block_on(backends::aws::AwsBucketService::new(opts)),
    };

    BucketConnection::new(backend, rt)
}
