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
        // Build ID/name list by scanning BO_ lines from raw text
        let mut ids: Vec<u32> = Vec::new();
        let mut names: Vec<String> = Vec::new();
        for line in text.lines() {
            let l = line.trim_start();
            if l.starts_with("BO_") {
                let parts: Vec<_> = l.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(id) = parts[1].parse::<u32>() { ids.push(id); }
                    // Name token may be at parts[2]
                    if parts.len() >= 3 {
                        let mut nm = parts[2].to_string();
                        if nm.ends_with(':') { nm.pop(); }
                        names.push(nm);
                    } else { names.push(String::new()); }
                }
            }
        }
        let dbc = can_dbc::DBC::from_str(&text).map_err(|e| anyhow!("DBC parse error: {:?}", e))?;
        // Build message-name -> index map from parsed DBC
        let mut name_to_idx: HashMap<String, usize> = HashMap::new();
        for (idx, m) in dbc.messages().iter().enumerate() {
            let nm = m.message_name().to_string();
            name_to_idx.insert(nm, idx);
        }
        let mut by_id = HashMap::new();
        let mut name_by_id = HashMap::new();
        for (i, id) in ids.iter().enumerate() {
            if let Some(nm) = names.get(i) {
                if let Some(idx) = name_to_idx.get(nm) {
                    by_id.insert(*id, *idx);
                    name_by_id.insert(*id, nm.clone());
                } else if let Some(msg) = dbc.messages().get(i) {
                    // Fallback: assume same ordering between BO_ and parser
                    by_id.insert(*id, i);
                    name_by_id.insert(*id, msg.message_name().to_string());
                }
            }
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
    // Big-endian (Motorola) per DBC: within each byte, bits are numbered from MSB(7) to LSB(0).
    // The start_bit points to the MSB of the signal in this numbering.
    // Build a BE 64-bit value, then compute the absolute position of the signal's MSB.
    let mut acc: u64 = 0;
    for i in 0..8 { acc = (acc << 8) | data[i] as u64; }
    if len == 0 || len > 64 { return None; }
    let byte = start_bit / 8;
    let bit_in_byte = start_bit % 8; // 0..7, where 7 is MSB in DBC Motorola notation
    // Position from the MSB side of the 64-bit word (0..63), where 0 is MSB of acc.
    let pos_from_msb = byte * 8 + (7 - bit_in_byte);
    if pos_from_msb + 1 < len { return None; }
    let shift = 63usize.saturating_sub(pos_from_msb) + (len - 1);
    let mask = if len == 64 { u64::MAX } else { (1u64 << len) - 1 };
    Some((acc >> (63 - pos_from_msb - (len - 1))) & mask)
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
