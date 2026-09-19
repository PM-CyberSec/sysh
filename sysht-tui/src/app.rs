use std::sync::mpsc;
use std::time::{Duration, Instant};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::health::{self, Snapshot};
use crate::theme;
use crate::tui::Tui;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum View {
    Overview,
    Cpu,
    Memory,
    DiskFs,
    DiskDirs,
    DiskFiles,
    Packages,
    Docker,
    Network,
    Processes,
    Services,
    Sensors,
    Virt,
}

impl View {
    fn name(self) -> &'static str {
        match self {
            View::Overview => "Overview",
            View::Cpu => "CPU",
            View::Memory => "Memory",
            View::DiskFs => "Disk:FS",
            View::DiskDirs => "Disk:Dirs",
            View::DiskFiles => "Disk:Files",
            View::Packages => "Packages",
            View::Docker => "Docker",
            View::Network => "Network",
            View::Processes => "Procs",
            View::Services => "Svcs/Logs",
            View::Sensors => "Sensors",
            View::Virt => "Virt-Thin",
        }
    }
    fn key(self) -> char {
        match self {
            View::Overview => '1',
            View::Cpu => '2',
            View::Memory => '3',
            View::DiskFs => '4',
            View::DiskDirs => '5',
            View::DiskFiles => '6',
            View::Packages => '7',
            View::Docker => '8',
            View::Network => '9',
            View::Processes => '0',
            View::Services => 's',
            View::Sensors => 't',
            View::Virt => 'v',
        }
    }
    fn section_match(self) -> &'static str {
        match self {
            View::Overview => "SYSTEM HEADER",
            View::Cpu => "CPU",
            View::Memory => "MEMORY",
            View::DiskFs => "DISK — Filesystems",
            View::DiskDirs => "DISK — Top",
            View::DiskFiles => "DISK — Top",
            View::Packages => "DISK — Top",
            View::Docker => "DISK — Docker",
            View::Network => "NETWORK",
            View::Processes => "PROCESSES",
            View::Services => "SERVICES",
            View::Sensors => "SENSORS",
            View::Virt => "VIRT — Thin",
        }
    }
    fn from_key(c: char) -> Option<Self> {
        match c {
            '1' => Some(View::Overview),
            '2' => Some(View::Cpu),
            '3' => Some(View::Memory),
            '4' => Some(View::DiskFs),
            '5' => Some(View::DiskDirs),
            '6' => Some(View::DiskFiles),
            '7' => Some(View::Packages),
            '8' => Some(View::Docker),
            '9' => Some(View::Network),
            '0' => Some(View::Processes),
            's' | 'S' => Some(View::Services),
            't' | 'T' => Some(View::Sensors),
            'v' | 'V' => Some(View::Virt),
            _ => None,
        }
    }
    fn from_f_key(code: KeyCode) -> Option<Self> {
        match code {
            KeyCode::F(1) => Some(View::Overview),
            KeyCode::F(2) => Some(View::Cpu),
            KeyCode::F(3) => Some(View::Memory),
            KeyCode::F(4) => Some(View::DiskFs),
            KeyCode::F(5) => Some(View::DiskDirs),
            KeyCode::F(6) => Some(View::DiskFiles),
            KeyCode::F(7) => Some(View::Packages),
            KeyCode::F(8) => Some(View::Docker),
            KeyCode::F(9) => Some(View::Network),
            KeyCode::F(10) => Some(View::Processes),
            KeyCode::F(11) => Some(View::Services),
            KeyCode::F(12) => Some(View::Sensors),
            KeyCode::F(13) => Some(View::Virt),
            // also allow Shift+F? use fallback
            _ => None,
        }
    }
}

const ALL_VIEWS: &[View] = &[
    View::Overview, View::Cpu, View::Memory, View::DiskFs, View::DiskDirs, View::DiskFiles,
    View::Packages, View::Docker, View::Network, View::Processes, View::Services, View::Sensors, View::Virt,
];

pub struct App {
    sysh_path: std::path::PathBuf,
    top_n: usize,
    timeout: u64,
    do_virt: bool,
    current_view: View,
    snapshot: Option<Snapshot>,
    scroll: usize,
    should_quit: bool,
    show_help: bool,
    last_refresh: Instant,
    auto_refresh: bool,
    refresh_interval: Duration,
    msg: Option<String>,
    error: Option<String>,
    loading: bool,
    rx: Option<mpsc::Receiver<anyhow::Result<Snapshot>>>,
    tx: mpsc::Sender<anyhow::Result<Snapshot>>,
}

impl App {
    pub fn new(sysh_path: std::path::PathBuf, top_n: usize, timeout: u64, do_virt: bool) -> Self {
        let (real_tx, real_rx) = mpsc::channel();
        Self {
            sysh_path,
            top_n,
            timeout,
            do_virt,
            current_view: View::Overview,
            snapshot: None,
            scroll: 0,
            should_quit: false,
            show_help: false,
            last_refresh: Instant::now() - Duration::from_secs(100),
            auto_refresh: true,
            refresh_interval: Duration::from_secs(30),
            msg: Some(format!("press r to refresh, v virt:{}, ? help, q quit", if do_virt {"on"} else {"off"})),
            error: None,
            loading: false,
            rx: Some(real_rx),
            tx: real_tx,
        }
    }

    pub fn run(&mut self) -> anyhow::Result<()> {
        let mut tui = Tui::new()?;
        tui.enter()?;
        self.refresh_async();
        tui.draw(|f| self.render(f))?;

        while !self.should_quit {
            // check for finished collection without blocking
            self.poll_refresh();
            if self.auto_refresh && !self.loading && self.last_refresh.elapsed() >= self.refresh_interval {
                self.refresh_async();
            }
            if event::poll(Duration::from_millis(120))? {
                let ev = event::read()?;
                self.handle_input(ev);
                tui.draw(|f| self.render(f))?;
            } else {
                // periodic redraw to show loading spinner / clock
                tui.draw(|f| self.render(f))?;
            }
        }
        tui.exit()?;
        Ok(())
    }

    fn refresh_async(&mut self) {
        if self.loading {
            return;
        }
        self.loading = true;
        self.last_refresh = Instant::now();
        self.msg = Some(format!("refreshing (top {} virt:{} )…", self.top_n, if self.do_virt {"on"} else {"off"}));
        let sysh = self.sysh_path.clone();
        let top = self.top_n;
        let timeout = if self.timeout == 0 { 55 } else { self.timeout };
        let do_virt = self.do_virt;
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let res = health::collect_with_virt(&sysh, top, timeout, do_virt);
            let _ = tx.send(res);
        });
    }

    fn poll_refresh(&mut self) {
        if let Some(rx) = &self.rx {
            match rx.try_recv() {
                Ok(Ok(snap)) => {
                    self.loading = false;
                    let n = snap.order.len();
                    let when = snap.generated_at.clone();
                    self.snapshot = Some(snap);
                    self.error = None;
                    self.msg = Some(format!("refreshed at {} — {} sections", when, n));
                }
                Ok(Err(e)) => {
                    self.loading = false;
                    self.error = Some(format!("collect failed: {e}"));
                    self.msg = Some("refresh failed".to_string());
                }
                Err(mpsc::TryRecvError::Empty) => {},
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.loading = false;
                }
            }
        }
    }

    fn refresh(&mut self) {
        // legacy sync wrapper kept for manual R key — now delegates to async
        self.refresh_async();
    }

    fn handle_input(&mut self, ev: Event) {
        let Event::Key(k) = ev else { return };
        if let Some(view) = View::from_f_key(k.code) {
            self.current_view = view;
            self.scroll = 0;
            return;
        }
        // Ctrl+V toggles virt scan on/off (since 'v' is view switch)
        if k.modifiers == KeyModifiers::CONTROL && (k.code == KeyCode::Char('v') || k.code == KeyCode::Char('V')) {
            self.do_virt = !self.do_virt;
            self.msg = Some(format!("virt scan {} (next refresh)", if self.do_virt {"ON"} else {"OFF"}));
            return;
        }
        match k.code {
            KeyCode::Char('q') if k.modifiers != KeyModifiers::CONTROL => self.should_quit = true,
            KeyCode::Char('?') => self.show_help = !self.show_help,
            KeyCode::Char('r') | KeyCode::Char('R') => { self.refresh(); self.scroll = 0; },
            KeyCode::Char('a') => { self.auto_refresh = !self.auto_refresh; self.msg = Some(format!("auto-refresh {}", if self.auto_refresh {"on"} else {"off"})); },
            KeyCode::Char('j') | KeyCode::Down => self.scroll = self.scroll.saturating_add(1),
            KeyCode::Char('k') | KeyCode::Up => self.scroll = self.scroll.saturating_sub(1),
            KeyCode::Char('h') if k.modifiers == KeyModifiers::CONTROL => self.scroll = self.scroll.saturating_sub(1),
            KeyCode::Char('d') if k.modifiers == KeyModifiers::CONTROL => self.scroll = self.scroll.saturating_add(1),
            KeyCode::PageDown => self.scroll = self.scroll.saturating_add(15),
            KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(15),
            KeyCode::Home => self.scroll = 0,
            KeyCode::End => self.scroll = 10_000,
            KeyCode::Char(c) => {
                if let Some(v) = View::from_key(c) {
                    self.current_view = v;
                    self.scroll = 0;
                } else if c == 'n' {
                    self.current_view = next_view(self.current_view);
                    self.scroll = 0;
                } else if c == 'p' {
                    self.current_view = prev_view(self.current_view);
                    self.scroll = 0;
                }
            }
            KeyCode::Tab => {
                self.current_view = next_view(self.current_view);
                self.scroll = 0;
            }
            KeyCode::BackTab => {
                self.current_view = prev_view(self.current_view);
                self.scroll = 0;
            }
            KeyCode::Esc => {
                if self.show_help { self.show_help = false; } else { self.scroll = 0; }
            }
            _ => {}
        }
    }

    fn render(&mut self, f: &mut Frame) {
        let footer_h: u16 = if self.show_help { 7 } else { 1 };
        let [header, header_border, main, footer] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(footer_h),
        ]).areas(f.area());

        self.render_header(f, header);
        self.render_header_border(f, header_border);
        self.render_main(f, main);
        self.render_footer(f, footer);
    }

    fn render_header(&self, f: &mut Frame, area: Rect) {
        let title = format!(" sysht — TUI Health  • {} • {} ", self.current_view.name(), self.snapshot.as_ref().map(|s| s.generated_at.clone()).unwrap_or_else(|| "loading…".to_string()));
        let mut spans = vec![Span::styled(title, Style::default().fg(theme::BRAND).add_modifier(Modifier::BOLD))];
        let right = if self.auto_refresh { " auto:ON " } else { " auto:OFF " };
        let pad = area.width.saturating_sub((spans.iter().map(|s| s.content.len()).sum::<usize>() + right.len()) as u16);
        if pad > 0 { spans.push(Span::raw(" ".repeat(pad as usize))); }
        spans.push(Span::styled(right, Style::default().fg(theme::TEXT_MUTED)));

        let p = Paragraph::new(Line::from(spans)).style(Style::default().bg(theme::BG_DARK));
        f.render_widget(p, area);
        // status line below header_border? handled there
    }

    fn render_header_border(&self, f: &mut Frame, area: Rect) {
        let block = Block::default().borders(Borders::BOTTOM);
        f.render_widget(block, area);
    }

    fn render_main(&mut self, f: &mut Frame, area: Rect) {
        if let Some(ref err) = self.error {
            let block = Block::default().title(" Error ").borders(Borders::ALL).border_style(Style::default().fg(theme::ERROR));
            let inner = block.inner(area);
            f.render_widget(block, area);
            let p = Paragraph::new(Text::from(vec![Line::from(Span::styled(err.clone(), Style::default().fg(theme::ERROR)))]));
            f.render_widget(p, inner);
            return;
        }
        let Some(ref snap) = self.snapshot else {
            let block = Block::default().title(" Loading… ").borders(Borders::ALL).border_style(Style::default().fg(theme::BRAND));
            let inner = block.inner(area);
            f.render_widget(block, area);
            let p = Paragraph::new("Collecting system health via sysh…\nThis may take ~10-15s for disk scans (du/find).\nPress q to quit, r to retry.")
                .style(Style::default().fg(theme::TEXT_MUTED)).wrap(Wrap{ trim:false});
            f.render_widget(p, inner);
            return;
        };

        // Determine content for current view
        let content = self.view_content(snap);
        let title = format!(" {} ", self.current_view.name());
        let block = Block::default().title(title).borders(Borders::ALL).border_style(Style::default().fg(theme::BRAND));
        let inner = block.inner(area);
        f.render_widget(block, area);

        // Build paragraph with line numbers / highlighting?
        let lines: Vec<Line> = content.lines().map(|l| {
            // Simple highlight for WARN/CRIT
            if l.contains("✘") || l.contains("CRIT") { Line::from(Span::styled(l.to_string(), Style::default().fg(theme::ERROR).add_modifier(Modifier::BOLD))) }
            else if l.contains("⚠") || l.contains("WARN") { Line::from(Span::styled(l.to_string(), Style::default().fg(theme::WARNING))) }
            else if l.contains("✔") { Line::from(Span::styled(l.to_string(), Style::default().fg(theme::SUCCESS))) }
            else if l.starts_with("━━━") { Line::from(Span::styled(l.to_string(), Style::default().fg(theme::BRAND).add_modifier(Modifier::BOLD))) }
            else { Line::from(Span::styled(l.to_string(), Style::default().fg(theme::TEXT))) }
        }).collect();

        let p = Paragraph::new(Text::from(lines))
            .wrap(Wrap{ trim: false })
            .scroll((self.scroll as u16, 0));
        f.render_widget(p, inner);
    }

    fn view_content(&self, snap: &Snapshot) -> String {
        match self.current_view {
            View::Overview => {
                // Combine header + uptime + load + quick summary
                let mut out = String::new();
                if let Some(ref j) = snap.json_overview {
                    out.push_str(&format!("JSON Overview:\n{}\n\n", serde_json::to_string_pretty(j).unwrap_or_default()));
                }
                // Append raw header sections
                for key in &snap.order {
                    if key.contains("HEADER") || key.contains("SYSTEM HEADER") {
                        out.push_str(&snap.section_text(key));
                        out.push_str("\n");
                    }
                }
                // Also include ALERTS summary
                for key in &snap.order {
                    if key.contains("ALERTS") {
                        out.push_str("\n");
                        out.push_str(&snap.section_text(key));
                    }
                }
                out
            }
            View::Cpu => find_section(snap, &["CPU"]),
            View::Memory => find_section(snap, &["MEMORY"]),
            View::DiskFs => find_sections_combined(snap, &["DISK — Filesystems", "DISK — Inodes", "DISK — Block Devices"]),
            View::DiskDirs => find_sections_combined(snap, &["DISK — Top", "Largest Directories"]),
            View::DiskFiles => find_sections_combined(snap, &["Largest Files"]),
            View::Packages => find_sections_combined(snap, &["Programs/Packages", "DISK — Top"]),
            View::Docker => find_section(snap, &["DISK — Docker"]),
            View::Network => find_section(snap, &["NETWORK"]),
            View::Processes => find_section(snap, &["PROCESSES"]),
            View::Services => find_sections_combined(snap, &["SERVICES", "LOGS"]),
            View::Sensors => find_section(snap, &["SENSORS"]),
            View::Virt => find_section(snap, &["VIRT — Thin"]),
        }
    }

    fn render_footer(&self, f: &mut Frame, area: Rect) {
        let view_keys: Vec<String> = ALL_VIEWS.iter().map(|v| format!("{}:{}", v.key(), v.name())).collect();
        let mut spans = Vec::new();
        for (i, &v) in ALL_VIEWS.iter().enumerate() {
            let is_active = v == self.current_view;
            let key_style = if is_active { Style::default().fg(theme::BG_DARK).bg(theme::TEXT).add_modifier(Modifier::BOLD) } else { Style::default().fg(theme::BG_DARK).bg(theme::TEXT_MUTED) };
            let label_style = if is_active { Style::default().fg(theme::TEXT).bg(theme::BG_DARK).add_modifier(Modifier::BOLD) } else { Style::default().fg(theme::TEXT_MUTED).bg(theme::BG_DARK) };
            spans.push(Span::styled(format!(" {} ", v.key()), key_style));
            spans.push(Span::styled(format!(" {} ", v.name()), label_style));
            if i < ALL_VIEWS.len()-1 { spans.push(Span::raw(" ")); }
        }
        let help_keys = [
            ("q","Quit"), ("r","Refresh"), ("a","Auto"), ("Ctrl+v","Virt"), ("n/p","Next/Prev"), ("j/k","Scroll"), ("?","Help")
        ];
        let mut second: Vec<Span> = Vec::new();
        for (k,l) in help_keys {
            second.push(Span::styled(format!(" {} ",k), Style::default().fg(theme::BG_DARK).bg(theme::TEXT_MUTED)));
            second.push(Span::styled(format!(" {} ",l), Style::default().fg(theme::TEXT_MUTED).bg(theme::BG_DARK)));
        }
        let msg = self.msg.clone().unwrap_or_default();
        let mut lines = vec![Line::from(spans)];
        // Wrap view keys if too long: second line is help keys + msg
        let mut second_line = vec![];
        second_line.extend(second);
        second_line.push(Span::raw(" "));
        second_line.push(Span::styled(msg, Style::default().fg(theme::TEXT_DIM)));
        lines.push(Line::from(second_line));

        if self.show_help {
            lines.push(Line::from(Span::styled(" Help: F1-F13/v switch views (1-9/0/s/t/v), Tab next, j/k scroll, g/Home top, G/End bottom, r refresh, a auto, Ctrl+v toggle virt, q quit ", Style::default().fg(theme::TEXT_MUTED))));
            lines.push(Line::from(Span::styled(" Disk categorization: FS, Dirs, Files, Packages, Docker, I/O. Virt: qcow2/vdi/vmdk/vhd + LVM thin + calc actual_left = host_free - total_remaining. ", Style::default().fg(theme::TEXT_DIM))));
            lines.push(Line::from(Span::styled(" sysh bash collects (du/find/qemu-img/lvs), rust TUI displays. Data from /tmp/sysht-raw-*.txt and sysh --json-only. Virt needs --virt.", Style::default().fg(theme::TEXT_DIM))));
            lines.push(Line::from(Span::styled(" Press ? to close help.", Style::default().fg(theme::BRAND))));
        }

        let p = Paragraph::new(Text::from(lines));
        f.render_widget(p, area);
    }
}

fn next_view(cur: View) -> View {
    let idx = ALL_VIEWS.iter().position(|&v| v == cur).unwrap_or(0);
    ALL_VIEWS[(idx + 1) % ALL_VIEWS.len()]
}
fn prev_view(cur: View) -> View {
    let idx = ALL_VIEWS.iter().position(|&v| v == cur).unwrap_or(0);
    if idx == 0 { *ALL_VIEWS.last().unwrap() } else { ALL_VIEWS[idx-1] }
}

fn find_section(snap: &Snapshot, keywords: &[&str]) -> String {
    for (k, lines) in &snap.sections {
        for kw in keywords {
            if k.contains(kw) {
                return lines.join("\n");
            }
        }
    }
    // fallback: raw
    snap.raw_lines.join("\n")
}

fn find_sections_combined(snap: &Snapshot, keywords: &[&str]) -> String {
    let mut out = Vec::new();
    for kw in keywords {
        for (k, lines) in &snap.sections {
            if k.contains(kw) {
                out.push(lines.join("\n"));
            }
        }
    }
    if out.is_empty() { snap.raw_lines.join("\n") } else { out.join("\n\n") }
}
