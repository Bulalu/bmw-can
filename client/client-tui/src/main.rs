use anyhow::Result;
use clap::Parser;
use client_core::Frame;
use client_udp::{run_udp_listener, UdpConfig};
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::execute;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs, Chart, Axis, Dataset, GraphType, Table, Row, Cell},
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
    /// KCAN UDP port
    #[arg(long, default_value_t = 45454)]
    kcan_port: u16,
    /// PTCAN UDP port
    #[arg(long, default_value_t = 45455)]
    ptcan_port: u16,
    /// Path to DBC file (unused in MVP, reserved for next step)
    #[arg(long, default_value = "../dbc/bmw_e90.dbc")]
    dbc: String,
    /// Max raw frame lines to show
    #[arg(long, default_value_t = 200)]
    tail: usize,
    /// Demo mode: generate fake frames instead of UDP
    #[arg(long, default_value_t = false)]
    demo: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Panel {
    Dashboard,
    Raw,
    Signals,
    Stats,
    Logs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BusTab { All, KCAN, PTCAN }

struct AppState {
    frames_all: Vec<Frame>,
    frames_kcan: Vec<Frame>,
    frames_ptcan: Vec<Frame>,
    recv_count: u64,
    last_tick: Instant,
    pps: f32,
    total: u64,
    pps_k: f32,
    pps_p: f32,
    recv_k: u64,
    recv_p: u64,
    paused: bool,
    logging: bool,
    show_help: bool,
    input_mode: bool,
    input_buf: String,
    filter: String,
    panel: Panel,
    status_socket: String,
    demo: bool,
    // Chart state
    tick: u64,
    rpm_hist: Vec<f64>,
    speed_hist: Vec<f64>,
    thr_hist: Vec<f64>,
    bus_tab: BusTab,
}

impl AppState {
    fn new(status_socket: String) -> Self {
        Self {
            frames_all: Vec::with_capacity(1024),
            frames_kcan: Vec::with_capacity(1024),
            frames_ptcan: Vec::with_capacity(1024),
            recv_count: 0,
            last_tick: Instant::now(),
            pps: 0.0,
            total: 0,
            pps_k: 0.0,
            pps_p: 0.0,
            recv_k: 0,
            recv_p: 0,
            paused: false,
            logging: false,
            show_help: false,
            input_mode: false,
            input_buf: String::new(),
            filter: String::new(),
            panel: Panel::Raw,
            status_socket,
            demo: false,
            tick: 0,
            rpm_hist: Vec::with_capacity(256),
            speed_hist: Vec::with_capacity(256),
            thr_hist: Vec::with_capacity(256),
            bus_tab: BusTab::All,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let (tx, mut rx) = mpsc::channel::<(BusTab, Frame)>(8192);
    if args.demo {
        // Demo generator task
        let tx_demo = tx.clone();
        tokio::spawn(async move {
            let mut counter: u8 = 0;
            let mut ts: u64 = 0;
            let ids: [u32; 3] = [0x123, 0x1A6, 0x329];
            let mut idx = 0usize;
            loop {
                ts += 50_000; // ~20 Hz
                let mut data = [0u8; 8];
                for i in 0..8 { data[i] = counter.wrapping_add(i as u8); }
                let _ = tx_demo.send((BusTab::All, Frame { ts_us: ts, id: ids[idx], dlc: 8, data })).await;
                counter = counter.wrapping_add(1);
                idx = (idx + 1) % ids.len();
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        });
    } else {
        // KCAN listener
        let tx_k = tx.clone();
        let host_k = args.host.clone();
        let k_port = args.kcan_port;
        tokio::spawn(async move {
            let (inner_tx, mut inner_rx) = mpsc::channel::<Frame>(4096);
            let cfg = UdpConfig { host: host_k, port: k_port };
            tokio::spawn(async move { let _ = run_udp_listener(cfg, inner_tx).await; });
            while let Some(f) = inner_rx.recv().await {
                let _ = tx_k.send((BusTab::KCAN, f)).await;
            }
        });
        // PTCAN listener
        let tx_p = tx.clone();
        let host_p = args.host.clone();
        let p_port = args.ptcan_port;
        tokio::spawn(async move {
            let (inner_tx, mut inner_rx) = mpsc::channel::<Frame>(4096);
            let cfg = UdpConfig { host: host_p, port: p_port };
            tokio::spawn(async move { let _ = run_udp_listener(cfg, inner_tx).await; });
            while let Some(f) = inner_rx.recv().await {
                let _ = tx_p.send((BusTab::PTCAN, f)).await;
            }
        });
    }

    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    let mut term = init_terminal()?;
    term.clear()?;
    let status_socket = if args.demo {
        String::from("demo")
    } else {
        format!("{} [K:{} | P:{}]", args.host, args.kcan_port, args.ptcan_port)
    };
    let res = run_app(&mut term, &mut rx, args.tail, status_socket, args.demo).await;
    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
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
    rx: &mut mpsc::Receiver<(BusTab, Frame)>,
    tail_max: usize,
    status_socket: String,
    demo_mode: bool,
) -> Result<()> {
    let mut app = AppState::new(status_socket);
    app.demo = demo_mode;
    let mut ticker = time::interval(Duration::from_millis(100));
    loop {
        // Non-blocking drain of frames
        if !app.paused {
            while let Ok((bus, frame)) = rx.try_recv() {
                app.recv_count += 1;
                app.total += 1;
                app.frames_all.push(frame.clone());
                match bus {
                    BusTab::KCAN => { app.recv_k += 1; app.frames_kcan.push(frame); if app.frames_kcan.len() > tail_max { let ex = app.frames_kcan.len()-tail_max; app.frames_kcan.drain(0..ex); } },
                    BusTab::PTCAN => { app.recv_p += 1; app.frames_ptcan.push(frame); if app.frames_ptcan.len() > tail_max { let ex = app.frames_ptcan.len()-tail_max; app.frames_ptcan.drain(0..ex); } },
                    BusTab::All => { /* demo */ },
                }
                if app.frames_all.len() > tail_max { let ex = app.frames_all.len()-tail_max; app.frames_all.drain(0..ex); }
            }
        }

        // Update demo charts regularly (until real DBC wiring)
        app.tick = app.tick.wrapping_add(1);
        let t = app.tick as f64;
        let rpm = (1500.0 + (t * 0.10).sin() * 2000.0 + (t * 0.03).sin() * 500.0).clamp(0.0, 6000.0);
        let speed = (50.0 + (t * 0.07).sin() * 50.0 + (t * 0.011).cos() * 5.0).clamp(0.0, 200.0);
        let thr = (50.0 + (t * 0.13).sin() * 40.0).clamp(0.0, 100.0);
        push_hist(&mut app.rpm_hist, rpm, 200);
        push_hist(&mut app.speed_hist, speed, 200);
        push_hist(&mut app.thr_hist, thr, 200);

        // Update PPS every second
        if app.last_tick.elapsed() >= Duration::from_secs(1) {
            let secs = app.last_tick.elapsed().as_secs_f32();
            app.pps = app.recv_count as f32 / secs;
            app.pps_k = app.recv_k as f32 / secs;
            app.pps_p = app.recv_p as f32 / secs;
            app.recv_count = 0; app.recv_k = 0; app.recv_p = 0;
            app.last_tick = Instant::now();
        }

        // Render
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(1),
                    Constraint::Length(1),
                ])
                .split(f.size());

            // Status bar
            let status = format!(
                "BMW-CAN Client  |  {}  |  PPS: {:.1} (K:{:.1} P:{:.1})  |  Total: {}  |  Logging: {}  |  Filter: {}",
                if app.demo { String::from("MODE: demo") } else { format!("UDP: {}", app.status_socket) },
                app.pps, app.pps_k, app.pps_p,
                app.total,
                if app.logging {"on"} else {"off"},
                if app.filter.is_empty() {"(none)".to_string()} else {app.filter.clone()}
            );
            let header = Paragraph::new(status)
                .block(Block::default().borders(Borders::ALL).title("Status"))
                .style(Style::default());
            f.render_widget(header, chunks[0]);

            // Tabs and main content area
            let body_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(1)])
                .split(chunks[1]);

            let titles = ["Dashboard", "Raw", "Signals", "Stats", "Logs"]
                .iter()
                .map(|t| Line::from(Span::raw(*t)))
                .collect::<Vec<_>>();
            let mut tabs = Tabs::new(titles)
                .block(Block::default().borders(Borders::ALL).title("Panels"))
                .highlight_style(Style::default().add_modifier(Modifier::BOLD));
            let idx = match app.panel {
                Panel::Dashboard => 0,
                Panel::Raw => 1,
                Panel::Signals => 2,
                Panel::Stats => 3,
                Panel::Logs => 4,
            };
            tabs = tabs.select(idx);
            f.render_widget(tabs, body_chunks[0]);

            // Content render based on panel
            match app.panel {
                Panel::Raw => {
                    // Bus tabs inside Raw panel
                    let bus_titles = ["All", "K-CAN", "PT-CAN"].iter().map(|t| Line::from(Span::raw(*t))).collect::<Vec<_>>();
                    let mut btabs = Tabs::new(bus_titles)
                        .block(Block::default().borders(Borders::ALL).title("Bus"))
                        .highlight_style(Style::default().add_modifier(Modifier::BOLD));
                    let bi = match app.bus_tab { BusTab::All => 0, BusTab::KCAN => 1, BusTab::PTCAN => 2 };
                    btabs = btabs.select(bi);
                    let inner = Layout::default().direction(Direction::Vertical).constraints([Constraint::Length(3), Constraint::Min(1)]).split(body_chunks[1]);
                    f.render_widget(btabs, inner[0]);

                    let iter_src: Box<dyn Iterator<Item=&Frame>> = match app.bus_tab {
                        BusTab::All => Box::new(app.frames_all.iter().rev()),
                        BusTab::KCAN => Box::new(app.frames_kcan.iter().rev()),
                        BusTab::PTCAN => Box::new(app.frames_ptcan.iter().rev()),
                    };
                    let items: Vec<ListItem> = iter_src
                        .filter(|fr| match_filter(fr, &app.filter))
                        .take(500)
                        .map(|fr| ListItem::new(fr.to_csv_line()))
                        .collect();
                    let list = List::new(items)
                        .block(Block::default().borders(Borders::ALL).title("Raw Frames (latest first)"));
                    f.render_widget(list, inner[1]);
                }
                Panel::Dashboard => {
                    let rows = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                        .split(body_chunks[1]);

                    // RPM Chart
                    let rpm_pts: Vec<(f64,f64)> = app.rpm_hist.iter().enumerate().map(|(i,v)| (i as f64, *v)).collect();
                    let rpm_ds = vec![Dataset::default()
                        .name("RPM")
                        .marker(ratatui::symbols::Marker::Braille)
                        .style(Style::default().fg(Color::LightRed))
                        .graph_type(GraphType::Line)
                        .data(&rpm_pts)];
                    let rpm_chart = Chart::new(rpm_ds)
                        .block(Block::default().borders(Borders::ALL).title("RPM"))
                        .x_axis(Axis::default().bounds([0.0, app.rpm_hist.len().max(1) as f64]))
                        .y_axis(Axis::default().bounds([0.0, 6000.0]));
                    f.render_widget(rpm_chart, rows[0]);

                    // Speed & Throttle Chart
                    let sp_pts: Vec<(f64,f64)> = app.speed_hist.iter().enumerate().map(|(i,v)| (i as f64, *v)).collect();
                    let th_pts: Vec<(f64,f64)> = app.thr_hist.iter().enumerate().map(|(i,v)| (i as f64, *v)).collect();
                    let ds = vec![
                        Dataset::default()
                            .name("Speed km/h")
                            .marker(ratatui::symbols::Marker::Dot)
                            .style(Style::default().fg(Color::LightCyan))
                            .graph_type(GraphType::Line)
                            .data(&sp_pts),
                        Dataset::default()
                            .name("Throttle %")
                            .marker(ratatui::symbols::Marker::Dot)
                            .style(Style::default().fg(Color::LightGreen))
                            .graph_type(GraphType::Line)
                            .data(&th_pts),
                    ];
                    let chart = Chart::new(ds)
                        .block(Block::default().borders(Borders::ALL).title("Speed & Throttle"))
                        .x_axis(Axis::default().bounds([0.0, app.speed_hist.len().max(1) as f64]))
                        .y_axis(Axis::default().bounds([0.0, 200.0]));
                    f.render_widget(chart, rows[1]);
                }
                Panel::Signals => {
                    // Fake signals based on demo charts
                    let rpm = *app.rpm_hist.last().unwrap_or(&0.0);
                    let speed = *app.speed_hist.last().unwrap_or(&0.0);
                    let thr = *app.thr_hist.last().unwrap_or(&0.0);
                    let coolant = 70.0 + (app.tick as f64 * 0.05).sin() * 10.0;
                    let gears = ["P","R","N","D","3","2","1"]; 
                    let gear = gears[(app.total as usize / 50) % gears.len()];

                    let header = Row::new(vec![
                        Cell::from(Span::styled("Signal", Style::default().add_modifier(Modifier::BOLD))),
                        Cell::from(Span::styled("Value", Style::default().add_modifier(Modifier::BOLD))),
                        Cell::from(Span::styled("Unit", Style::default().add_modifier(Modifier::BOLD))),
                        Cell::from(Span::styled("Rate (Hz)", Style::default().add_modifier(Modifier::BOLD))),
                    ]);
                    let rows = vec![
                        Row::new(vec![
                            Cell::from("rpm"),
                            styled_val(format!("{:.0}", rpm), style_for_signal("rpm", rpm)),
                            Cell::from("rpm"),
                            Cell::from("20.0"),
                        ]),
                        Row::new(vec![
                            Cell::from("vehicle_speed"),
                            styled_val(format!("{:.1}", speed), style_for_signal("vehicle_speed", speed)),
                            Cell::from("km/h"),
                            Cell::from("20.0"),
                        ]),
                        Row::new(vec![
                            Cell::from("throttle"),
                            styled_val(format!("{:.0}", thr), style_for_signal("throttle", thr)),
                            Cell::from("%"),
                            Cell::from("20.0"),
                        ]),
                        Row::new(vec![
                            Cell::from("coolant_temp"),
                            styled_val(format!("{:.1}", coolant), style_for_signal("coolant_temp", coolant)),
                            Cell::from("°C"),
                            Cell::from("1.0"),
                        ]),
                        Row::new(vec![
                            Cell::from("gear"),
                            Cell::from(gear.to_string()),
                            Cell::from(""),
                            Cell::from("2.0"),
                        ]),
                    ];
                    let table = Table::new(
                        rows,
                        [
                            Constraint::Length(16),
                            Constraint::Length(12),
                            Constraint::Length(8),
                            Constraint::Length(10),
                        ],
                    )
                    .header(header)
                    .block(Block::default().borders(Borders::ALL).title("Signals (demo)"));
                    f.render_widget(table, body_chunks[1]);
                }
                Panel::Stats => {
                    let p = Paragraph::new(format!("PPS: {:.1}\nTotal frames: {}\nPaused: {}\nMode: {}",
                        app.pps, app.total, app.paused, if app.demo {"demo"} else {"udp"}))
                        .block(Block::default().borders(Borders::ALL).title("Stats"));
                    f.render_widget(p, body_chunks[1]);
                }
                Panel::Logs => {
                    let p = Paragraph::new("Logging controls coming soon")
                        .block(Block::default().borders(Borders::ALL).title("Logs"));
                    f.render_widget(p, body_chunks[1]);
                }
            }

            // Bottom help / input bar
            let bottom = if app.input_mode {
                format!("/{}", app.input_buf)
            } else if app.show_help {
                String::from("[q] quit  [Tab/1-5] panels  [/] filter  [c] clear  [p] pause  [l] log (stub)  [?] help")
            } else {
                String::from("Press ? for help. / filter  Tab/←/→ or 1-5 to switch panels. q to quit.")
            };
            let b = Paragraph::new(bottom).block(Block::default().borders(Borders::ALL));
            f.render_widget(b, chunks[2]);
        })?;

        // Handle input (non-blocking)
        if event::poll(std::time::Duration::from_millis(0))? {
            if let Event::Key(k) = event::read()? {
                if app.input_mode {
                    match k.code {
                        KeyCode::Esc => { app.input_mode = false; app.input_buf.clear(); }
                        KeyCode::Enter => { app.filter = app.input_buf.clone(); app.input_mode = false; }
                        KeyCode::Backspace => { app.input_buf.pop(); }
                        KeyCode::Char(ch) => { app.input_buf.push(ch); }
                        _ => {}
                    }
                } else {
                    match k.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Esc => { app.show_help = false; },
                        KeyCode::Char('?') => { app.show_help = !app.show_help; },
                        KeyCode::Char('/') => { app.input_mode = true; app.input_buf.clear(); },
                        KeyCode::Char('c') => { app.frames_all.clear(); app.frames_kcan.clear(); app.frames_ptcan.clear(); },
                        KeyCode::Char('p') => { app.paused = !app.paused; },
                        KeyCode::Char('l') => { app.logging = !app.logging; },
                        KeyCode::Tab | KeyCode::Char('\t') | KeyCode::Right => { app.panel = next_panel(app.panel); },
                        KeyCode::Left => { app.panel = prev_panel(app.panel); },
                        KeyCode::Char('b') => { app.bus_tab = next_bus(app.bus_tab); },
                        KeyCode::Char('1') => { app.panel = Panel::Dashboard; },
                        KeyCode::Char('2') => { app.panel = Panel::Raw; },
                        KeyCode::Char('3') => { app.panel = Panel::Signals; },
                        KeyCode::Char('4') => { app.panel = Panel::Stats; },
                        KeyCode::Char('5') => { app.panel = Panel::Logs; },
                        _ => {}
                    }
                }
            }
        }
        // Tick pacing
        ticker.tick().await;
    }
    Ok(())
}

fn next_panel(p: Panel) -> Panel {
    match p {
        Panel::Dashboard => Panel::Raw,
        Panel::Raw => Panel::Signals,
        Panel::Signals => Panel::Stats,
        Panel::Stats => Panel::Logs,
        Panel::Logs => Panel::Dashboard,
    }
}

fn prev_panel(p: Panel) -> Panel {
    match p {
        Panel::Dashboard => Panel::Logs,
        Panel::Raw => Panel::Dashboard,
        Panel::Signals => Panel::Raw,
        Panel::Stats => Panel::Signals,
        Panel::Logs => Panel::Stats,
    }
}

fn next_bus(b: BusTab) -> BusTab {
    match b {
        BusTab::All => BusTab::KCAN,
        BusTab::KCAN => BusTab::PTCAN,
        BusTab::PTCAN => BusTab::All,
    }
}

fn match_filter(fr: &Frame, filter: &str) -> bool {
    if filter.is_empty() { return true; }
    // Try numeric match first
    if let Some(hex) = filter.strip_prefix("0x").or_else(|| filter.strip_prefix("0X")) {
        if u32::from_str_radix(hex, 16).ok().map_or(false, |id| id == fr.id) { return true; }
    }
    if let Ok(id) = filter.parse::<u32>() { if id == fr.id { return true; } }
    // Fallback substring on CSV line
    fr.to_csv_line().to_lowercase().contains(&filter.to_lowercase())
}

fn push_hist(v: &mut Vec<f64>, value: f64, max_len: usize) {
    v.push(value);
    if v.len() > max_len {
        let excess = v.len() - max_len;
        v.drain(0..excess);
    }
}

fn style_for_signal(name: &str, v: f64) -> Style {
    match name {
        "rpm" => {
            if v >= 5500.0 { Style::default().fg(Color::Red) }
            else if v >= 4000.0 { Style::default().fg(Color::Yellow) }
            else { Style::default() }
        }
        "vehicle_speed" => {
            if v >= 160.0 { Style::default().fg(Color::Red) }
            else if v >= 120.0 { Style::default().fg(Color::Yellow) }
            else { Style::default() }
        }
        "throttle" => {
            if v >= 85.0 { Style::default().fg(Color::Yellow) }
            else { Style::default().fg(Color::LightGreen) }
        }
        "coolant_temp" => {
            if v >= 105.0 { Style::default().fg(Color::Red) }
            else if v >= 95.0 { Style::default().fg(Color::Yellow) }
            else { Style::default() }
        }
        _ => Style::default(),
    }
}

fn styled_val(s: String, style: Style) -> Cell<'static> {
    Cell::from(Line::from(Span::styled(s, style)))
}
