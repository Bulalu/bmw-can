use anyhow::{Context, Result};
use clap::Parser;
use client_core::dbc_runtime::{DbcRuntime, DecodedSignal};
use serde::Serialize;
use client_core::{parse_csv_line, Frame};
use std::fs::File;
use std::io::{BufRead, BufReader};

#[derive(Debug, Parser)]
#[command(name = "client-decode", about = "Decode CSV log via DBC")]
struct Args {
    /// Path to CSV log produced by TUI (ts_us,id,dlc,data_hex)
    #[arg(long)]
    input: String,
    /// Path to DBC file
    #[arg(long, default_value = "../dbc/bmw_e90.dbc")]
    dbc: String,
    /// Only show signals whose names contain this substring (case-insensitive)
    #[arg(long)]
    filter: Option<String>,
    /// List unique IDs in the log and whether they exist in DBC
    #[arg(long, default_value_t = false)]
    list_ids: bool,
    /// Dump DBC mapping (ID -> message + signals) to a JSON file and exit
    #[arg(long)]
    dump_dbc_json: Option<String>,
    /// Dump decoded values keyed by CAN ID from a CSV log to JSON and exit
    #[arg(long)]
    dump_values_json: Option<String>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let rt = DbcRuntime::load(&args.dbc).context("load DBC")?;
    if let Some(path) = &args.dump_dbc_json {
        dump_dbc_json(&rt, path)?;
        return Ok(());
    }
    let fp = File::open(&args.input).with_context(|| format!("open {}", &args.input))?;
    let reader = BufReader::new(fp);
    let filt = args.filter.as_ref().map(|s| s.to_lowercase());

    // Optional decoded dump accumulator
    let mut dump_acc: Option<std::collections::HashMap<u32, Vec<DumpEntry>>> =
        args.dump_values_json.as_ref().map(|_| std::collections::HashMap::new());

    let mut id_seen: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let fr: Frame = match parse_csv_line(&line) {
            Ok(f) => f,
            Err(_) => continue,
        };
        *id_seen.entry(fr.id).or_insert(0) += 1;
        if let Some(sigs) = rt.decode(&fr) {
            if let Some(ref mut acc) = dump_acc {
                let mut m = std::collections::HashMap::new();
                for s in &sigs { m.insert(s.name.clone(), s.value); }
                acc.entry(fr.id).or_default().push(DumpEntry { ts_us: fr.ts_us, signals: m });
            } else {
                print_decoded(&fr, &sigs, filt.as_deref());
            }
        }
    }
    if let Some(path) = args.dump_values_json.as_ref() {
        write_values_json(&rt, dump_acc.unwrap_or_default(), id_seen, path)?;
        return Ok(());
    }
    if args.list_ids {
        println!("-- ID summary --");
        let mut v: Vec<(u32, usize)> = id_seen.into_iter().collect();
        v.sort_by_key(|e| e.0);
        for (id, cnt) in v {
            let known = rt.by_id.contains_key(&id);
            let name = rt.name_by_id.get(&id).cloned().unwrap_or_default();
            println!("0x{:03X} count={} known={} {}", id, cnt, known, name);
        }
    }
    Ok(())
}

#[derive(Serialize)]
struct JsonSignal {
    name: String,
    start_bit: u64,
    length: u64,
    byte_order: String,
    signed: bool,
    factor: f64,
    offset: f64,
    unit: Option<String>,
}

#[derive(Serialize)]
struct JsonMessage { name: String, id_dec: u32, id_hex: String, signals: Vec<JsonSignal> }

fn dump_dbc_json(rt: &DbcRuntime, path: &str) -> Result<()> {
    // Build messages array in the order of DBC messages
    let mut msgs: Vec<JsonMessage> = Vec::new();
    for (idx, m) in rt.dbc.messages().iter().enumerate() {
        // Resolve id via by_id reverse map; if missing, skip
        let id = rt.by_id.iter().find_map(|(k,v)| if *v == idx { Some(*k) } else { None });
        let id = if let Some(id) = id { id } else { continue };
        let name = rt.name_by_id.get(&id).cloned().unwrap_or_default();
        let mut signals = Vec::new();
        for s in m.signals() {
            let js = JsonSignal {
                name: s.name().to_string(),
                start_bit: *s.start_bit(),
                length: *s.signal_size(),
                byte_order: match s.byte_order() { can_dbc::ByteOrder::LittleEndian => "intel".into(), can_dbc::ByteOrder::BigEndian => "motorola".into() },
                signed: matches!(s.value_type(), can_dbc::ValueType::Signed),
                factor: *s.factor(),
                offset: *s.offset(),
                unit: if s.unit().is_empty() { None } else { Some(s.unit().clone()) },
            };
            signals.push(js);
        }
        let jm = JsonMessage { name, id_dec: id, id_hex: format!("0x{:03X}", id), signals };
        msgs.push(jm);
    }
    let root = serde_json::json!({ "messages": msgs });
    std::fs::write(path, serde_json::to_vec_pretty(&root)?)?;
    eprintln!("Wrote DBC JSON to {}", path);
    Ok(())
}

fn print_decoded(fr: &Frame, sigs: &[DecodedSignal], filt: Option<&str>) {
    let id = fr.id;
    for s in sigs {
        if let Some(f) = filt {
            if !s.name.to_lowercase().contains(f) {
                continue;
            }
        }
        if let Some(unit) = &s.unit {
            println!("0x{:03X} {} = {:.3} {}", id, s.name, s.value, unit);
        } else {
            println!("0x{:03X} {} = {:.3}", id, s.name, s.value);
        }
    }
}

#[derive(Serialize)]
struct DumpEntry {
    ts_us: u64,
    signals: std::collections::HashMap<String, f64>,
}

fn write_values_json(
    rt: &DbcRuntime,
    acc: std::collections::HashMap<u32, Vec<DumpEntry>>,
    id_seen: std::collections::HashMap<u32, usize>,
    path: &str,
) -> Result<()> {
    // Convert to hex-keyed map; include only IDs that produced decoded values
    use serde_json::json;
    let mut root = serde_json::Map::new();
    for (id, list) in acc {
        if list.is_empty() { continue; }
        let key = format!("0x{:03X}", id);
        let name = rt.name_by_id.get(&id).cloned().unwrap_or_default();
        let count = id_seen.get(&id).copied().unwrap_or(0);
        let arr = serde_json::to_value(&list)?;
        root.insert(key, json!({ "name": name, "id_dec": id, "count": count, "values": arr }));
    }
    std::fs::write(path, serde_json::to_vec_pretty(&serde_json::Value::Object(root))?)?;
    eprintln!("Wrote decoded values to {}", path);
    Ok(())
}
