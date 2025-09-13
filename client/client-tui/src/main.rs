use anyhow::Result;
use clap::Parser;
use client_core::Frame;
use client_udp::{run_udp_listener, UdpConfig};
use crossterm::event::{self, Event, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Style},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Terminal,
};
use std::io::{stdout, Stdout};
use tokio::sync::mpsc;
use tokio::time::{self, Duration, Instant};

#[derive(Debug, Parser)]
#[command(name = "client-tui", version, about = "BMW-CAN UDP → TUI client")]
struct Args {
    #[arg(long, default_value = "0.0.0.0")] 
    host: String,
    #[arg(long, default_value_t = 45454)]
    port: u16,
    /// Path to DBC file (unused in MVP, reserved for next step)
    #[arg(long, default_value = "../dbc/bmw_e90.dbc")]
    dbc: String,
    /// Max raw frame lines to show
    #[arg(long, default_value_t = 200)]
    tail: usize,
}

struct AppState {
    frames: Vec<Frame>,
    recv_count: u64,
    last_tick: Instant,
    pps: f32,
}

impl AppState {
    fn new() -> Self {
        Self { frames: Vec::with_capacity(1024), recv_count: 0, last_tick: Instant::now(), pps: 0.0 }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let (tx, mut rx) = mpsc::channel::<Frame>(4096);
    let udp_cfg = UdpConfig { host: args.host.clone(), port: args.port };
    tokio::spawn(async move {
        let _ = run_udp_listener(udp_cfg, tx).await;
    });

    enable_raw_mode()?;
    let mut term = init_terminal()?;
    let res = run_app(&mut term, &mut rx, args.tail).await;
    disable_raw_mode()?;
    term.show_cursor()?;
    if let Err(e) = res { eprintln!("{e:?}"); }
    Ok(())
}

fn init_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    let backend = CrosstermBackend::new(stdout());
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    rx: &mut mpsc::Receiver<Frame>,
    tail_max: usize,
) -> Result<()> {
    let mut app = AppState::new();
    let mut ticker = time::interval(Duration::from_millis(100));
    loop {
        // Non-blocking drain of frames
        while let Ok(frame) = rx.try_recv() {
            app.recv_count += 1;
            app.frames.push(frame);
            if app.frames.len() > tail_max {
                let excess = app.frames.len() - tail_max;
                app.frames.drain(0..excess);
            }
        }

        // Update PPS every second
        if app.last_tick.elapsed() >= Duration::from_secs(1) {
            app.pps = app.recv_count as f32 / app.last_tick.elapsed().as_secs_f32();
            app.recv_count = 0;
            app.last_tick = Instant::now();
        }

        // Render
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(1),
                ])
                .split(f.size());

            let header = Paragraph::new(format!(
                "BMW-CAN Client  |  UDP: listening  |  PPS: {:.1}",
                app.pps
            ))
            .block(Block::default().borders(Borders::ALL).title("Status"))
            .style(Style::default());
            f.render_widget(header, chunks[0]);

            let items: Vec<ListItem> = app
                .frames
                .iter()
                .rev()
                .take(100)
                .map(|fr| ListItem::new(fr.to_csv_line()))
                .collect();
            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title("Raw Frames (latest first)"));
            f.render_widget(list, chunks[1]);
        })?;

        // Handle input (non-blocking)
        if event::poll(std::time::Duration::from_millis(0))? {
            if let Event::Key(k) = event::read()? {
                match k.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    _ => {}
                }
            }
        }
        // Tick pacing
        ticker.tick().await;
    }
    Ok(())
}
