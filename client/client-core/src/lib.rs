use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frame {
    pub ts_us: u64,
    pub id: u32,
    pub dlc: u8,
    pub data: [u8; 8],
}

impl Frame {
    pub fn to_csv_line(&self) -> String {
        let mut s = format!("{},0x{:X},{}", self.ts_us, self.id, self.dlc);
        s.push(',');
        for i in 0..self.dlc.min(8) as usize {
            s.push_str(&format!("{:02X}", self.data[i]));
        }
        s
    }
}

// Parses "ts_us,id,dlc,data_hex" e.g. "1234567,0x1A6,8,1122334455667788"
pub fn parse_csv_line(line: &str) -> Result<Frame> {
    let parts: Vec<_> = line.trim().split(',').collect();
    if parts.len() != 4 {
        bail!("expected 4 comma-separated fields");
    }
    let ts_us: u64 = parts[0]
        .parse()
        .with_context(|| format!("invalid ts_us: {}", parts[0]))?;
    let id_str = parts[1];
    let id = if let Some(hex) = id_str.strip_prefix("0x").or_else(|| id_str.strip_prefix("0X")) {
        u32::from_str_radix(hex, 16)
            .with_context(|| format!("invalid hex id: {}", id_str))?
    } else {
        id_str
            .parse()
            .with_context(|| format!("invalid id: {}", id_str))?
    };
    let dlc: u8 = parts[2]
        .parse()
        .with_context(|| format!("invalid dlc: {}", parts[2]))?;
    let data_hex = parts[3].trim();
    if data_hex.len() % 2 != 0 || data_hex.len() > 16 {
        bail!("invalid data hex length");
    }
    let mut data = [0u8; 8];
    for i in (0..data_hex.len()).step_by(2) {
        let byte = u8::from_str_radix(&data_hex[i..i + 2], 16)
            .with_context(|| format!("invalid hex at {}", i))?;
        data[i / 2] = byte;
    }
    Ok(Frame { ts_us, id, dlc, data })
}

// Placeholder for future DBC integration.
pub mod dbc {
    pub struct Database;
    impl Database {
        pub fn load_path(_path: &str) -> anyhow::Result<Self> {
            // TODO: integrate a proper DBC parser (e.g., can-dbc)
            Ok(Database)
        }
    }
}

