use bfs::{backends, BackendOptions, Config};
use clap::Parser;
use eyre::Context;
use http::Uri;
use tokio::runtime::{self, Runtime};

#[derive(Debug, Parser)]
#[clap(version, about)]
struct Cli {}

fn main() -> eyre::Result<()> {
    env_logger::init();

    let cli = Cli::parse();

    let rt = runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .wrap_err("unable to build tokio runtime")?;

    run(cli, rt)
}

fn run(_cli: Cli, rt: Runtime) -> eyre::Result<()> {
    let cfg = Config {
        bucket_name: "bucket1".into(),
        mountpoint: "/tmp/fuse-s3".into(),
        backend: BackendOptions::Aws(backends::aws::Options {
            endpoint_uri: Uri::from_static("http://localhost:4566").into(),
        }),
    };

    bfs::run_app(cfg, rt)
}
