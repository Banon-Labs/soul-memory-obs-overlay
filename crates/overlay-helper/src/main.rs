use clap::Parser;
use overlay_helper::{debug_probe, load_config, run_loop, run_pipe_server, ProcessMemoryReader};
use std::io;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "overlay-helper")]
#[command(about = "Read game memory and emit overlay messages")]
struct Cli {
    #[arg(long, default_value = "config/overlay.toml")]
    config: PathBuf,
    #[arg(long, default_value_t = false)]
    once: bool,
    #[arg(long, default_value_t = false)]
    stdout: bool,
    #[arg(long, default_value_t = false)]
    debug_probe: bool,
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();
    let cfg = match load_config(&cli.config) {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("{err}");
            return Ok(());
        }
    };

    if cli.debug_probe {
        for line in debug_probe(&cfg) {
            eprintln!("{line}");
        }
    }

    let mut reader = ProcessMemoryReader;
    if cli.stdout {
        run_loop(&mut reader, &cfg, &mut io::stdout(), cli.once)
    } else {
        run_pipe_server(&mut reader, &cfg, cli.once)
    }
}
