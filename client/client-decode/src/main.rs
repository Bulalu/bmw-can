use anyhow::{Context, Result};
use clap::Parser;
use client_core::{parse_csv_line, Frame};
use client_core::dbc_runtime::{DbcRuntime, DecodedSignal};
use std::fs::File;
use std::io::{BufRead, BufReader};

#[derive(Debug, Parser)]
#[command(name="client-decode", about="Decode CSV log via DBC")] 
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
}

fn main() -> Result<()> {
    let args = Args::parse();
    let rt = DbcRuntime::load(&args.dbc).context("load DBC")?;
    let fp = File::open(&args.input).with_context(|| format!("open {}", &args.input))?;
    let reader = BufReader::new(fp);
    let filt = args.filter.as_ref().map(|s| s.to_lowercase());

    let mut id_seen: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() { continue; }
        let fr: Frame = match parse_csv_line(&line) { Ok(f)=>f, Err(_)=>continue };
        *id_seen.entry(fr.id).or_insert(0) += 1;
        if let Some(sigs) = rt.decode(&fr) {
            print_decoded(&fr, &sigs, filt.as_deref());
        }
    }
    if args.list_ids {
        println!("-- ID summary --");
        let mut v: Vec<(u32,usize)> = id_seen.into_iter().collect();
        v.sort_by_key(|e| e.0);
        for (id, cnt) in v {
            let known = rt.by_id.contains_key(&id);
            let name = rt.name_by_id.get(&id).cloned().unwrap_or_default();
            println!("0x{:03X} count={} known={} {}", id, cnt, known, name);
        }
    }
    Ok(())
}

fn print_decoded(fr: &Frame, sigs: &[DecodedSignal], filt: Option<&str>) {
    let id = fr.id;
    for s in sigs {
        if let Some(f) = filt { if !s.name.to_lowercase().contains(f) { continue; } }
        if let Some(unit) = &s.unit {
            println!("0x{:03X} {} = {:.3} {}", id, s.name, s.value, unit);
        } else {
            println!("0x{:03X} {} = {:.3}", id, s.name, s.value);
        }
    }
}
