use anyhow::{anyhow, Result, Context};
use std::collections::HashMap;

use crate::Frame;

pub struct DecodedSignal {
    pub name: String,
    pub value: f64,
    pub unit: Option<String>,
}

pub struct DbcRuntime { pub dbc: can_dbc::DBC, pub by_id: HashMap<u32, usize>, pub name_by_id: HashMap<u32, String> }

impl DbcRuntime {
    pub fn load(path: &str) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading DBC at {}", path))?;
        // Build ID list by scanning BO_ lines
        let mut ids: Vec<u32> = Vec::new();
        let mut names: Vec<String> = Vec::new();
        for line in text.lines() {
            let l = line.trim_start();
            if l.starts_with("BO_") {
                let parts: Vec<_> = l.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(id) = parts[1].parse::<u32>() { ids.push(id); }
                    // Name token may be at parts[2]
                    if parts.len() >= 3 { names.push(parts[2].to_string()); } else { names.push(String::new()); }
                }
            }
        }
        let dbc = can_dbc::DBC::from_str(&text).map_err(|e| anyhow!("DBC parse error: {:?}", e))?;
        let mut by_id = HashMap::new();
        let mut name_by_id = HashMap::new();
        for (idx, _m) in dbc.messages().iter().enumerate() {
            if let Some(id) = ids.get(idx) { by_id.insert(*id, idx); }
            if let (Some(id), Some(n)) = (ids.get(idx), names.get(idx)) { name_by_id.insert(*id, n.clone()); }
        }
        Ok(Self { dbc, by_id, name_by_id })
    }

    pub fn decode(&self, fr: &Frame) -> Option<Vec<DecodedSignal>> {
        let msg = self.by_id.get(&fr.id).and_then(|idx| self.dbc.messages().get(*idx))?;
        let mut out = Vec::new();
        for sig in msg.signals() {
            if let Some(v) = extract_signal(sig, &fr.data) {
                let phys = sig.factor() * v + sig.offset();
                out.push(DecodedSignal {
                    name: sig.name().to_string(),
                    value: phys,
                    unit: unit_opt(sig.unit()),
                });
            }
        }
        if out.is_empty() { None } else { Some(out) }
    }
}

fn extract_signal(sig: &can_dbc::Signal, data: &[u8;8]) -> Option<f64> {
    // Extract raw integer value from 64-bit buffer according to start bit/length/endianness.
    let start = *sig.start_bit() as usize;
    let len = *sig.signal_size() as usize;
    if len == 0 || len > 64 { return None; }
    let raw = match sig.byte_order() {
        can_dbc::ByteOrder::LittleEndian => extract_le_bits(data, start, len)?,
        can_dbc::ByteOrder::BigEndian => extract_be_bits(data, start, len)?,
    };
    let val = if matches!(sig.value_type(), can_dbc::ValueType::Signed) {
        // sign-extend from len
        let shift = 64 - len;
        ((raw << shift) as i64 >> shift) as f64
    } else {
        raw as f64
    };
    Some(val)
}

fn extract_le_bits(data: &[u8;8], start_bit: usize, len: usize) -> Option<u64> {
    // Little-endian (Intel) bit numbering: start_bit is LSB index from byte0 bit0.
    let mut acc: u64 = 0;
    for i in 0..8 { acc |= (data[i] as u64) << (i*8); }
    let mask = if len == 64 { u64::MAX } else { (1u64 << len) - 1 };
    Some((acc >> start_bit) & mask)
}

fn extract_be_bits(data: &[u8;8], start_bit: usize, len: usize) -> Option<u64> {
    // Big-endian (Motorola) bit numbering per DBC: start_bit points to the MSB of the signal
    // within a 64-bit big-endian view. Translate to a linear index over a big-endian bitstring.
    let mut acc: u64 = 0;
    for i in 0..8 { acc = (acc << 8) | data[i] as u64; }
    // In DBC, start_bit is counted from the start of the 64-bit message (byte0 bit7 = index 7).
    // Translate to a shift from the MSB side.
    let msb_index = 63 - start_bit;
    if len == 0 || msb_index + 1 < len { return None; }
    let shift = (msb_index + 1) - len;
    let mask = if len == 64 { u64::MAX } else { (1u64 << len) - 1 };
    Some((acc >> shift) & mask)
}

fn unit_opt(u: &String) -> Option<String> { if u.is_empty() { None } else { Some(u.clone()) } }

pub struct DecodedState {
    pub last_by_name: HashMap<String, f64>,
}

impl Default for DecodedState {
    fn default() -> Self { Self { last_by_name: HashMap::new() } }
}

impl DecodedState {
    pub fn apply(&mut self, signals: &[DecodedSignal]) {
        for s in signals { self.last_by_name.insert(s.name.clone(), s.value); }
    }
    pub fn get(&self, name: &str) -> Option<f64> { self.last_by_name.get(name).copied() }
}
