use anyhow::Result;
use client_core::{parse_csv_line, Frame};
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

pub struct UdpConfig {
    pub host: String,
    pub port: u16,
}

pub async fn run_udp_listener(cfg: UdpConfig, tx: mpsc::Sender<Frame>) -> Result<()> {
    let bind_addr = format!("{}:{}", cfg.host, cfg.port);
    let sock = UdpSocket::bind(&bind_addr).await?;
    let mut buf = vec![0u8; 4096];
    loop {
        let (n, _peer) = sock.recv_from(&mut buf).await?;
        let payload = &buf[..n];
        if let Ok(text) = std::str::from_utf8(payload) {
            for line in text.lines() {
                if line.trim().is_empty() { continue; }
                if let Ok(frame) = parse_csv_line(line) {
                    let _ = tx.send(frame).await;
                }
            }
        }
    }
}

