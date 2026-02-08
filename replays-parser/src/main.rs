use clap::Parser as ClapParser;
use rayon::prelude::*;
use replays_parser::Parser;
use std::collections::HashMap;
use std::path::PathBuf;
use std::fs;

#[derive(ClapParser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the .wotreplay file or directory containing replays
    #[arg(required = true)]
    input: PathBuf,

    /// Output to stdout as JSON lines
    #[arg(short, long, default_value_t = false)]
    json: bool,

    /// Print statistics about message types (for debugging/analysis)
    #[arg(short, long, default_value_t = false)]
    stats: bool,
}

fn main() {
    let args = Args::parse();

    let paths: Vec<PathBuf> = if args.input.is_dir() {
        fs::read_dir(&args.input)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.extension().map_or(false, |ext| ext == "wotreplay"))
            .collect()
    } else {
        vec![args.input.clone()]
    };

    // For --stats mode, we need to collect results from parallel iteration
    if args.stats {
        use std::sync::Mutex;
        // Key: (PacketType, SubType)
        let global_stats: Mutex<HashMap<(u32, Option<u32>), u64>> = Mutex::new(HashMap::new());
        let total_packets: Mutex<u64> = Mutex::new(0);
        let total_errors: Mutex<u64> = Mutex::new(0);

        paths.par_iter().for_each(|path| {
            match Parser::parse_file(path) {
                Ok(replay) => {
                    use std::io::Cursor;
                    use byteorder::{ReadBytesExt, LittleEndian};

                    // Load Definitions
                    // Try to normalize version string to match our IDs format
                    // e.g. "World of Tanks v.1.25.1.0 #1234" -> "wot_v1_25_1_0" or close to it
                    // For now, let's just use the build.rs logic: match exact or fallback
                    // Actually, build.rs keys are "wot_eu_v1_...", so we need to guess or user provides it?
                    // The internal replay version string is like "1.25.1.0".
                    // We might need a mapping function. 
                    // detailed matching is complex, for MVP let's just try to load *any* definition that matches version number.
                    // Or iterate all available definitions in definitions.rs? No public iterator.
                    
                    // Simple logic:
                    // 1. Try "wot_v{version_clean}"
                    // 2. Try "wot_eu_v{version_clean}"
                    
                    let raw_ver = &replay.battle_config.client_version_from_exe;
                    let clean_ver = raw_ver.replace('.', "_");
                    
                    // Hybrid Loading Strategy:
                    // 1. Try to load from "ids_wot_v{ver}.json" in current dir (Runtime override)
                    // 2. Try embedded "wot_v{ver}"
                    // 3. Try fallback variants
                    
                    let variants = [
                        format!("wot_v{}", clean_ver),
                        format!("wot_eu_v{}", clean_ver),
                        format!("wot_ru_v{}", clean_ver),
                        format!("wot_na_v{}", clean_ver),
                        format!("wot_asia_v{}", clean_ver),
                    ];
                    
                    let mut defs = None;
                    
                    // 1. Try Files
                    for variant in &variants {
                        let filename = format!("ids_{}.json", variant);
                        if let Ok(d) = replays_parser::definitions::Definitions::load_from_file(std::path::Path::new(&filename)) {
                            println!("  [Loaded Overrides from {}]", filename);
                            defs = Some(d);
                            break;
                        }
                    }
                    
                    // 2. Try Embedded
                    if defs.is_none() {
                        for variant in &variants {
                           if let Some(d) = replays_parser::definitions::Definitions::load_embedded(variant) {
                               defs = Some(d);
                               break;
                           }
                        }
                    }

                    let mut cursor = Cursor::new(replay.packets_buffer.clone());
                    let packet_stream = replays_parser::packet_stream::PacketStream::new(&mut cursor);

                    let mut local_stats: HashMap<(u32, Option<u32>), u64> = HashMap::new();
                    let mut local_count: u64 = 0;
                    let mut local_errors: u64 = 0;

                    for packet in packet_stream {
                        match packet {
                            Ok(p) => {
                                let mut sub_type = None;
                                
                                // Parse sub-type for known packet types
                                // 0x07 (Entity/Health), 0x08 (Tank Destruction/Damage)
                                // Structure: [EntityID (4)] [SubType (4)] ...
                                if (p.packet_type == 0x07 || p.packet_type == 0x08) && p.payload.len() >= 8 {
                                    let mut rdr = Cursor::new(&p.payload[4..8]);
                                    if let Ok(st) = rdr.read_u32::<LittleEndian>() {
                                        sub_type = Some(st);
                                    }
                                }

                                *local_stats.entry((p.packet_type, sub_type)).or_insert(0) += 1;
                                local_count += 1;
                            }
                            Err(_) => {
                                local_errors += 1;
                            }
                        }
                    }

                    // Merge into global stats
                    {
                        let mut stats = global_stats.lock().unwrap();
                        for (key, count) in local_stats {
                            *stats.entry(key).or_insert(0) += count;
                        }
                    }
                    *total_packets.lock().unwrap() += local_count;
                    *total_errors.lock().unwrap() += local_errors;
                }
                Err(e) => {
                    eprintln!("Error parsing {}: {}", path.display(), e);
                }
            }
        });

        // Print stats summary
        let stats = global_stats.into_inner().unwrap();
        let packets = *total_packets.lock().unwrap();
        let errors = *total_errors.lock().unwrap();

        println!("\n=== Message Type Statistics ===");
        println!("Total replays analyzed: {}", paths.len());
        println!("Total packets parsed: {}", packets);
        println!("Total packet errors: {}", errors);
        println!("\nPacket Type Distribution:");
        println!("{:>10} | {:>10} | {:>8} | {:<20}", "Type", "Count", "Percent", "SubTypes");
        println!("{:-<10}-+-{:-<10}-+-{:-<8}-+-{:-<20}", "", "", "", "");

        // Group by main type
        let mut grouped: HashMap<u32, u64> = HashMap::new();
        for ((ptype, _), count) in &stats {
            *grouped.entry(*ptype).or_insert(0) += count;
        }

        // Sort main types by total count descending
        let mut sorted_types: Vec<_> = grouped.iter().collect();
        sorted_types.sort_by(|a, b| b.1.cmp(a.1));

        for (ptype, total_count) in sorted_types {
            let pct = if packets > 0 { (*total_count as f64 / packets as f64) * 100.0 } else { 0.0 };
            
            // Try to find name for packet type
            // (We don't have reference to generic defs here, using hardcoded map from generate_ids for backup?)
            // Ideally we'd have a 'default' definition or use the one from the first replay.
            // For now just print Hex.
            
            println!("    0x{:02X}   | {:>10} | {:>7.2}% |", ptype, total_count, pct);

            // Print subtypes if any exist for this type
            let mut sub_types: Vec<_> = stats.iter()
                .filter(|((p, s), _)| *p == *ptype && s.is_some())
                .map(|((_, s), c)| (s.unwrap(), c))
                .collect();
            
            if !sub_types.is_empty() {
                sub_types.sort_by(|a, b| b.1.cmp(a.1)); // Sort subtypes by count
                for (stype, scount) in sub_types {
                     let spct = if *total_count > 0 { (*scount as f64 / *total_count as f64) * 100.0 } else { 0.0 };
                     println!("{:>10} | {:>10} | {:>8} |   -> Sub 0x{:02X}: {} ({:.1}%)", "", "", "", stype, scount, spct);
                }
            }
        }
        println!("\nUnique message types: {}", grouped.len());

    } else {
        // Original behavior
        paths.par_iter().for_each(|path| {
            match Parser::parse_file(path) {
                Ok(replay) => {
                    if args.json {
                        println!("{}", serde_json::to_string(&replay).unwrap());
                    } else {
                        println!("Successfully parsed: {}", path.display());
                        println!("  Magic: {:x}", replay.header.magic);
                        println!("  Block Count: {}", replay.header.block_count);
                        
                        println!("  Metadata:");
                        println!("    Player: {}", replay.battle_config.player_name);
                        println!("    Vehicle: {}", replay.battle_config.player_vehicle);
                        println!("    Map: {}", replay.battle_config.map_name);
                        println!("    Version: {}", replay.battle_config.client_version_from_exe);
                        println!("    Date: {}", replay.battle_config.date_time);

                        // Load Definitions
                        let raw_ver = &replay.battle_config.client_version_from_exe;
                        let clean_ver = raw_ver.replace('.', "_");
                        
                         let variants = [
                            format!("wot_v{}", clean_ver),
                            format!("wot_eu_v{}", clean_ver),
                            format!("wot_ru_v{}", clean_ver),
                            format!("wot_na_v{}", clean_ver),
                            format!("wot_asia_v{}", clean_ver),
                        ];
                        
                        let mut defs = None;
                        
                        // 1. Try Files
                        for variant in &variants {
                            let filename = format!("ids_{}.json", variant);
                            if let Ok(d) = replays_parser::definitions::Definitions::load_from_file(std::path::Path::new(&filename)) {
                                println!("  [Loaded Overrides from {}]", filename);
                                defs = Some(d);
                                break;
                            }
                        }
                        
                        // 2. Try Embedded
                        if defs.is_none() {
                            for variant in &variants {
                               if let Some(d) = replays_parser::definitions::Definitions::load_embedded(variant) {
                                   defs = Some(d);
                                   break;
                               }
                            }
                        }
                            
                        if let Some(_) = defs {
                             // Already printed loaded info for file override
                             if defs.is_some() {
                                 // println!("  [Definitions Loaded for v{}]", clean_ver); 
                             }
                        } else {
                             println!("  [No Definitions Found for v{}]", clean_ver);
                        }

                        println!("  Battle Results: {}", if replay.battle_results.is_some() { "present" } else { "missing" });
                        println!("  Packets Buffer: {} bytes", replay.packets_buffer.len());

                        // Verify packet stream
                        use std::io::Cursor;
                        use byteorder::{ReadBytesExt, LittleEndian};
                        
                        let mut cursor = Cursor::new(replay.packets_buffer.clone());
                        let packet_stream = replays_parser::packet_stream::PacketStream::new(&mut cursor);

                        println!("  First 20 packets:");
                        for (i, packet) in packet_stream.enumerate().take(20) {
                            match packet {
                                Ok(p) => {
                                    let mut desc = String::new();
                                    
                                    // Try to decode packet name
                                    if let Some(d) = &defs {
                                         let key = format!("0x{:02X}", p.packet_type);
                                         if let Some(val) = d.packet_types.get(&key) {
                                             if let Some(name) = val.as_str() {
                                                 desc = format!("({})", name);
                                             } else if let Some(obj) = val.as_object() {
                                                 if let Some(name) = obj.get("name").and_then(|n| n.as_str()) {
                                                     desc = format!("({})", name);
                                                 }
                                             }
                                         }
                                         
                                         // Entity Method Call (0x08) Logic
                                         if p.packet_type == 0x08 && p.payload.len() >= 8 {
                                             let mut rdr = Cursor::new(&p.payload);
                                             if let Ok(ent_id) = rdr.read_u32::<LittleEndian>() {
                                                 if let Ok(method_id) = rdr.read_u32::<LittleEndian>() {
                                                     // Lookup entity
                                                     if let Some(ent_def) = d.entities.get(&ent_id.to_string()) {
                                                         // Lookup Method
                                                         // Need to check client/cell/base? Usually ClientMethods for replay?
                                                         // Replays contain ClientMethods calls.
                                                         if let Some(m_def) = ent_def.client_methods.get(&method_id.to_string()) {
                                                             desc = format!("{} :: {}.{}", desc, ent_def.name, m_def.name);
                                                         } else {
                                                             desc = format!("{} :: {}.Method[{}]", desc, ent_def.name, method_id);
                                                         }
                                                     }
                                                 }
                                             }
                                         }
                                    }
                                    
                                    println!("    [{}] Time: {:.3}s, Type: 0x{:02X} {}, Size: {} bytes", i, p.time, p.packet_type, desc, p.length);
                                },
                                Err(e) => println!("    [{}] Error: {}", i, e),
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error parsing {}: {}", path.display(), e);
                }
            }
        });
    }
}
