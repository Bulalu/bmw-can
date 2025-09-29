use anyhow::{Context, Result};
use clap::Parser;
use client_core::dbc_runtime::{DbcRuntime, DecodedSignal};
use client_core::obd::{decode_obd_single_frame, ObdState};
use serde::Serialize;
use client_core::{parse_csv_line, Frame};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::collections::{HashMap, HashSet};

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
    /// Optional OBD log path (defaults to --input if not set)
    #[arg(long)]
    obd_input: Option<String>,
    /// Dump decoded OBD Mode01 values to JSON (grouped by signal name)
    #[arg(long)]
    dump_obd_json: Option<String>,
    /// Create unified export (PT-CAN filtered + OBD) to a single JSON file
    #[arg(long)]
    unify_out: Option<String>,
    /// Session ID to embed in unified export
    #[arg(long, default_value = "session-unknown")]
    session_id: String,
    /// Dump decoded values keyed by CAN ID from a CSV log to JSON and exit
    #[arg(long)]
    dump_values_json: Option<String>,
    /// Filter map JSON: { "0xID": ["SignalA","SignalB"], ... } — when set, only those signals are included per ID
    #[arg(long)]
    filter_map: Option<String>,
    /// Downsample rate in Hz per ID (e.g., 2 for ~2 samples/sec). 0 disables downsampling.
    #[arg(long, default_value_t = 0.0)]
    rate_hz: f64,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let rt = DbcRuntime::load(&args.dbc).context("load DBC")?;
    if let Some(path) = &args.dump_dbc_json {
        dump_dbc_json(&rt, path)?;
        return Ok(());
    }

    if let Some(path) = &args.dump_obd_json {
        let obd_path = args.obd_input.clone().unwrap_or(args.input.clone());
        dump_obd_values(&obd_path, path)?;
        return Ok(());
    }

    if let Some(path) = &args.unify_out {
        let obd_path = args.obd_input.clone();
        unify_export(
            &args.session_id,
            &args.input,
            &args.dbc,
            args.filter_map.as_deref(),
            args.rate_hz,
            obd_path.as_deref(),
            path,
        )?;
        return Ok(());
    }
    let fp = File::open(&args.input).with_context(|| format!("open {}", &args.input))?;
    let reader = BufReader::new(fp);
    let filt = args.filter.as_ref().map(|s| s.to_lowercase());

    // Optional decoded dump accumulator
    let mut dump_acc: Option<std::collections::HashMap<u32, Vec<DumpEntry>>> =
        args.dump_values_json.as_ref().map(|_| std::collections::HashMap::new());
    // Optional filter map
    let filter_map = if let Some(path) = &args.filter_map {
        Some(load_filter_map(path)?)
    } else { None };
    // Downsample period in microseconds
    let period_us: u64 = if args.rate_hz > 0.0 { (1_000_000.0/args.rate_hz).round() as u64 } else { 0 };
    let mut last_emit: std::collections::HashMap<u32, u64> = std::collections::HashMap::new();

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
        if let Some(mut sigs) = rt.decode(&fr) {
            // Apply name filter for console printing
            if dump_acc.is_none() {
                print_decoded(&fr, &sigs, filt.as_deref());
            } else {
                // Build signal map and apply filter-map if present
                let mut m = std::collections::HashMap::new();
                for s in &sigs { m.insert(s.name.clone(), s.value); }
                if let Some(fm) = &filter_map {
                    if let Some(keep) = fm.get(&fr.id) {
                        m.retain(|k,_| keep.contains(k));
                    } else {
                        // If filter map provided but no entry for this ID, drop entirely
                        m.clear();
                    }
                }
                if !m.is_empty() {
                    // Rate limit per ID
                    if period_us > 0 {
                        let le = last_emit.get(&fr.id).copied().unwrap_or(0);
                        if fr.ts_us < le + period_us { continue; }
                        last_emit.insert(fr.id, fr.ts_us);
                    }
                    if let Some(ref mut acc) = dump_acc { acc.entry(fr.id).or_default().push(DumpEntry { ts_us: fr.ts_us, signals: m }); }
                }
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
struct ObdEntry { ts_us: u64, value: f64, unit: String, ecu_id: String, pid: String }

fn dump_obd_values(input: &str, out: &str) -> Result<()> {
    let fp = File::open(input).with_context(|| format!("open {}", input))?;
    let reader = BufReader::new(fp);
    let mut map: std::collections::BTreeMap<String, Vec<ObdEntry>> = std::collections::BTreeMap::new();
    for line in reader.lines() {
        let line = line?; if line.trim().is_empty() { continue; }
        let fr: Frame = match parse_csv_line(&line) { Ok(f)=>f, Err(_)=>continue };
        if let Some(s) = decode_obd_single_frame(&fr) {
            let key = s.name.to_string();
            let ecu = format!("0x{:03X}", fr.id);
            let pid = format!("0x{:02X}", s.pid);
            map.entry(key).or_default().push(ObdEntry { ts_us: s.ts_us, value: s.value, unit: s.unit.to_string(), ecu_id: ecu, pid });
        }
    }
    let json = serde_json::to_vec_pretty(&map)?;
    std::fs::write(out, json)?;
    eprintln!("Wrote OBD values to {}", out);
    Ok(())
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

fn parse_id_key(s: &str) -> Option<u32> {
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u32::from_str_radix(hex, 16).ok()
    } else {
        s.parse::<u32>().ok()
    }
}

fn load_filter_map(path: &str) -> Result<HashMap<u32, HashSet<String>>> {
    let text = std::fs::read_to_string(path)?;
    let v: serde_json::Value = serde_json::from_str(&text)?;
    let mut out: HashMap<u32, HashSet<String>> = HashMap::new();
    if let Some(obj) = v.as_object() {
        for (k, arr) in obj.iter() {
            if let Some(id) = parse_id_key(k) {
                let mut set = HashSet::new();
                if let Some(a) = arr.as_array() {
                    for itm in a {
                        if let Some(name) = itm.as_str() { set.insert(name.to_string()); }
                    }
                }
                out.insert(id, set);
            }
        }
    }
    Ok(out)
}

#[derive(Serialize)]
struct UnifiedValue {
    ts_us: u64,
    signal_key: String,
    value: f64,
    unit: String,
    source: String,
    bus_id: String,
    #[serde(skip_serializing_if = "Option::is_none")] can_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")] ecu_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] origin: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct UnifiedExport { session_id: String, values: Vec<UnifiedValue> }

fn unify_export(
    session_id: &str,
    ptcan_input: &str,
    dbc_path: &str,
    filter_map_path: Option<&str>,
    rate_hz: f64,
    obd_input: Option<&str>,
    out: &str,
) -> Result<()> {
    let dbc = DbcRuntime::load(dbc_path)?;
    let filter_map = if let Some(p) = filter_map_path { Some(load_filter_map(p)?) } else { None };
    let period_us: u64 = if rate_hz > 0.0 { (1_000_000.0/rate_hz).round() as u64 } else { 0 };
    let mut last_emit: HashMap<u32, u64> = HashMap::new();
    let mut out_vals: Vec<UnifiedValue> = Vec::new();

    // PT-CAN path
    let fp = File::open(ptcan_input).with_context(|| format!("open {}", ptcan_input))?;
    let reader = BufReader::new(fp);
    for line in reader.lines() {
        let line = line?; if line.trim().is_empty() { continue; }
        let fr: Frame = match parse_csv_line(&line) { Ok(f)=>f, Err(_)=>continue };
        if period_us > 0 {
            let le = last_emit.get(&fr.id).copied().unwrap_or(0);
            if fr.ts_us < le + period_us { continue; }
            last_emit.insert(fr.id, fr.ts_us);
        }
        if let Some(sigs) = dbc.decode(&fr) {
            for s in sigs {
                // Apply filter-map
                if let Some(fm) = &filter_map {
                    if let Some(keep) = fm.get(&fr.id) {
                        if !keep.contains(&s.name) { continue; }
                    } else { continue; }
                }
                let key = canonical_key_dbc(&s.name);
                let unit = s.unit.clone().unwrap_or_default();
                out_vals.push(UnifiedValue {
                    ts_us: fr.ts_us,
                    signal_key: key,
                    value: s.value,
                    unit,
                    source: "PTCAN".into(),
                    bus_id: "ptcan".into(),
                    can_id: Some(fr.id),
                    ecu_id: None,
                    origin: Some(serde_json::json!({"dbc_signal": s.name})),
                });
            }
        }
    }

    // OBD path (optional) — always decode but skip overlapping signals (rpm/speed)
    if let Some(obd_file) = obd_input {
        let fp = File::open(obd_file).with_context(|| format!("open {}", obd_file))?;
        let reader = BufReader::new(fp);
        for line in reader.lines() {
            let line = line?; if line.trim().is_empty() { continue; }
            let fr: Frame = match parse_csv_line(&line) { Ok(f)=>f, Err(_)=>continue };
            if let Some(s) = decode_obd_single_frame(&fr) {
                let key = canonical_key_obd(s.name);
                // Skip OBD rpm/speed to avoid duplicates
                if key == "engine_speed_rpm" || key == "vehicle_speed_kph" { continue; }
                out_vals.push(UnifiedValue {
                    ts_us: s.ts_us,
                    signal_key: key,
                    value: s.value,
                    unit: s.unit.into(),
                    source: "OBD".into(),
                    bus_id: "obd".into(),
                    can_id: None,
                    ecu_id: Some(format!("0x{:03X}", fr.id)),
                    origin: Some(serde_json::json!({"pid": format!("0x{:02X}", s.pid)})),
                });
            }
        }
    }

    let exp = UnifiedExport { session_id: session_id.to_string(), values: out_vals };
    std::fs::write(out, serde_json::to_vec_pretty(&exp)?)?;
    eprintln!("Wrote unified export to {}", out);
    Ok(())
}

fn canonical_key_dbc(sig: &str) -> String {
    match sig {
        "VehicleSpeed" => "vehicle_speed_kph".into(),
        "YawRate" => "yaw_rate_dps".into(),
        "LatlAcc" => "lat_acc_ms2".into(),
        "LongAcc" => "long_acc_ms2".into(),
        "Wheel_FL" => "wheel_speed_fl_kph".into(),
        "Wheel_FR" => "wheel_speed_fr_kph".into(),
        "Wheel_RL" => "wheel_speed_rl_kph".into(),
        "Wheel_RR" => "wheel_speed_rr_kph".into(),
        "EngineSpeed" => "engine_speed_rpm".into(),
        "AcceleratorPedalPercentage" => "accel_pedal_pct".into(),
        "TORQ_AVL_MAX" => "torque_available_max_nm".into(),
        "TORQ_AVL_MIN" => "torque_available_min_nm".into(),
        "GearRatio" => "gear_ratio".into(),
        "GearTar" => "gear_target".into(),
        "OutputShaftSpeed" => "output_shaft_speed_rpm".into(),
        "Shifting" => "shifting_state".into(),
        "TEMP_ENG" => "engine_temp_c".into(),
        _ => sig.to_lowercase(),
    }
}

fn canonical_key_obd(name: &str) -> String {
    match name {
        "rpm" => "engine_speed_rpm".into(),
        "vehicle_speed" => "vehicle_speed_kph".into(),
        "coolant_temp" => "coolant_temp_c".into(),
        "fuel_level" => "fuel_level_pct".into(),
        "module_voltage" => "module_voltage_v".into(),
        "engine_load" => "engine_load_pct".into(),
        "throttle" => "throttle_pct".into(),
        "intake_air_temp" => "intake_air_temp_c".into(),
        "stft_b1" => "stft_b1_pct".into(),
        "ltft_b1" => "ltft_b1_pct".into(),
        "baro" => "baro_kpa".into(),
        "run_time" => "run_time_s".into(),
        _ => name.to_string(),
    }
}
