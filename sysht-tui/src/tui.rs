use std::io::stdout;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

pub struct Tui {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
}

impl Tui {
    pub fn new() -> anyhow::Result<Self> {
        let backend = CrosstermBackend::new(stdout());
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }
    pub fn enter(&mut self) -> anyhow::Result<()> {
        terminal::enable_raw_mode()?;
        stdout().execute(EnterAlternateScreen)?;
        disable_flow_control();
        Ok(())
    }
    pub fn exit(&mut self) -> anyhow::Result<()> {
        enable_flow_control();
        stdout().execute(LeaveAlternateScreen)?;
        terminal::disable_raw_mode()?;
        Ok(())
    }
    pub fn draw(&mut self, f: impl FnOnce(&mut ratatui::Frame)) -> anyhow::Result<()> {
        self.terminal.draw(f)?;
        Ok(())
    }
}

#[cfg(unix)]
fn disable_flow_control() {
    let _ = std::process::Command::new("stty").args(["-ixon", "-ixoff"]).status();
}
#[cfg(unix)]
fn enable_flow_control() {
    let _ = std::process::Command::new("stty").args(["ixon", "ixoff"]).status();
}
#[cfg(not(unix))]
fn disable_flow_control() {}
#[cfg(not(unix))]
fn enable_flow_control() {}
