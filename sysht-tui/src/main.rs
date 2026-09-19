mod app;
mod health;
mod theme;
mod tui;

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name="sysht-tui", about="TUI display for sysh — System Health (rust + ratatui)")]
struct Cli {
    /// Path to sysh bash script
    #[arg(long, default_value="/home/omen212/projects/active/sysh/sysh")]
    sysh: PathBuf,

    /// Top N entries for tables
    #[arg(long, short='n', default_value_t=10)]
    top: usize,

    /// Timeout for collection (seconds)
    #[arg(long, default_value_t=60)]
    timeout: u64,

    /// Enable thin-provisioned virt scan
    #[arg(long, default_value_t=false)]
    virt: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    if !cli.sysh.exists() {
        eprintln!("sysh not found at {:?}, use --sysh PATH", cli.sysh);
        std::process::exit(2);
    }
    let mut app = app::App::new(cli.sysh, cli.top, cli.timeout, cli.virt);
    app.run()
}
