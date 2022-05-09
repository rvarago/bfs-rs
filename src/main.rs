use clap::Parser;
use eyre::{eyre, Context};
use std::path::PathBuf;
use tokio::runtime::{self, Runtime};

#[derive(Debug, Parser)]
#[clap(version, about)]
struct Cli {
    #[clap(short, long, default_value = "config")]
    config: PathBuf,
}

fn main() -> eyre::Result<()> {
    env_logger::init();

    let cli = Cli::parse();

    let rt = runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .wrap_err("unable to build tokio runtime")?;

    run(cli, rt)
}

fn run(cli: Cli, rt: Runtime) -> eyre::Result<()> {
    let cfg = bfs::Config::load_from(&cli.config).wrap_err_with(|| {
        eyre!(
            "unable to load app config at path={}",
            cli.config.to_string_lossy()
        )
    })?;

    bfs::run(cfg, rt)
}
