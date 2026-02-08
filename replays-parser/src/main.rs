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
    #[arg(long, required = true)]
    input: PathBuf,

    /// Game version (e.g. "1_25_0" or "wot_eu_1_25_0")
    /// Required to load the correct entity definitions.
    #[arg(long, required = true)]
    version: String,

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

    // Load Definitions once
    // We expect the version string to be safe (e.g. "wot_eu_v1_25_1_0" or just "1_25_1_0" if we construct it)
    // The user provided version string is passed directly.
    
    // We need a way to construct the "variant" name if the user passes just "1.25.1.0". 
    // But the user said "do not trying to detect it".
    // So we assume args.version is the full ID or we try to load it directly.
    
    let defs = match replays_parser::definitions::Definitions::load(&args.version) {
        Ok(d) => Some(d),
        Err(e) => {
            eprintln!("Warning: Failed to load definitions for version '{}': {}", args.version, e);
            None
        }
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
            
            let mut name_desc = String::new();
            if let Some(d) = &defs {
                 let key = format!("0x{:02X}", ptype);
                 if let Some(val) = d.packet_types.get(&key) {
                     if let Some(id) = val.as_object().and_then(|o| o.get("id")).and_then(|s| s.as_str()) {
                         name_desc = format!("({})", id);
                     } else if let Some(s) = val.as_str() {
                         name_desc = format!("({})", s);
                     }
                 }
            }
            
            println!("    0x{:02X}   | {:>10} | {:>7.2}% | {}", ptype, total_count, pct, name_desc);

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

                        println!("    Version: {}", replay.battle_config.client_version_from_exe);
                        println!("    Date: {}", replay.battle_config.date_time);

                        if defs.is_some() {
                             // println!("  [Definitions Loaded]"); 
                        } else {
                             println!("  [No Definitions Loaded]");
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
                                                 // Legacy support
                                                 desc = format!("({})", name);
                                             } else if let Some(obj) = val.as_object() {
                                                 if let Some(id) = obj.get("id").and_then(|n| n.as_str()) {
                                                     desc = format!("({})", id);
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
