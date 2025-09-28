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

pub mod obd {
    use super::Frame;

    #[derive(Debug, Clone, Copy)]
    pub struct ObdSample {
        pub ts_us: u64,
        pub pid: u8,
        pub name: &'static str,
        pub unit: &'static str,
        pub value: f64,
    }

    #[derive(Default, Debug, Clone)]
    pub struct ObdState {
        pub rpm: Option<f64>,
        pub speed_kph: Option<f64>,
        pub coolant_c: Option<f64>,
        pub fuel_level_pct: Option<f64>,
        pub module_v: Option<f64>,
        pub engine_load_pct: Option<f64>,
        pub throttle_pct: Option<f64>,
        pub iat_c: Option<f64>,
        pub stft_b1_pct: Option<f64>,
        pub ltft_b1_pct: Option<f64>,
        pub baro_kpa: Option<f64>,
        pub run_time_s: Option<f64>,
    }

    impl ObdState {
        pub fn apply(&mut self, s: &ObdSample) {
            match s.pid {
                0x0C => self.rpm = Some(s.value),
                0x0D => self.speed_kph = Some(s.value),
                0x05 => self.coolant_c = Some(s.value),
                0x2F => self.fuel_level_pct = Some(s.value),
                0x42 => self.module_v = Some(s.value),
                0x04 => self.engine_load_pct = Some(s.value),
                0x11 => self.throttle_pct = Some(s.value),
                0x0F => self.iat_c = Some(s.value),
                0x06 => self.stft_b1_pct = Some(s.value),
                0x07 => self.ltft_b1_pct = Some(s.value),
                0x33 => self.baro_kpa = Some(s.value),
                0x1F => self.run_time_s = Some(s.value),
                _ => {}
            }
        }
    }

    // Decode single-frame Mode 01 responses commonly seen as: len, 0x41, PID, A, B, ...
    pub fn decode_obd_single_frame(fr: &Frame) -> Option<ObdSample> {
        let data = &fr.data;
        if fr.dlc < 3 { return None; }
        let pci = data[0] & 0xF0;
        if pci != 0x00 { return None; } // only SF here
        if data[1] != 0x41 { return None; }
        let pid = data[2];
        let a = if fr.dlc > 3 { data[3] } else { 0 };
        let b = if fr.dlc > 4 { data[4] } else { 0 };
        match pid {
            0x0C => Some(sample(fr, pid, "rpm", "RPM", ((a as u16 as u32 * 256 + b as u32) as f64) / 4.0)),
            0x0D => Some(sample(fr, pid, "vehicle_speed", "km/h", a as f64)),
            0x05 => Some(sample(fr, pid, "coolant_temp", "°C", (a as i16 - 40) as f64)),
            0x2F => Some(sample(fr, pid, "fuel_level", "%", (a as f64) * 100.0 / 255.0)),
            0x42 => Some(sample(fr, pid, "module_voltage", "V", (((a as u16) << 8) | b as u16) as f64 / 1000.0)),
            0x04 => Some(sample(fr, pid, "engine_load", "%", (a as f64) * 100.0 / 255.0)),
            0x11 => Some(sample(fr, pid, "throttle", "%", (a as f64) * 100.0 / 255.0)),
            0x0F => Some(sample(fr, pid, "intake_air_temp", "°C", (a as i16 - 40) as f64)),
            0x06 => Some(sample(fr, pid, "stft_b1", "%", (a as f64 - 128.0) * 100.0 / 128.0)),
            0x07 => Some(sample(fr, pid, "ltft_b1", "%", (a as f64 - 128.0) * 100.0 / 128.0)),
            0x33 => Some(sample(fr, pid, "baro", "kPa", a as f64)),
            0x1F => Some(sample(fr, pid, "run_time", "s", ((a as u16 as u32 * 256 + b as u32) as f64))),
            _ => None,
        }
    }

    fn sample(fr: &Frame, pid: u8, name: &'static str, unit: &'static str, value: f64) -> ObdSample {
        ObdSample { ts_us: fr.ts_us, pid, name, unit, value }
    }
}

pub mod dbc_runtime;
