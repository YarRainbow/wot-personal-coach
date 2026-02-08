use std::path::PathBuf;
use std::process::Command;

#[test]
fn test_parser_runs_on_replays() {
    // This integration test attempts to run the binary against the sample replays
    // It assumes `cargo build` has been run or we can run via `cargo run`

    let replay_dir = PathBuf::from("replays-data");
    if !replay_dir.exists() {
        eprintln!("replays-data directory not found, skipping integration test");
        return;
    }

    // Find a replay file
    let entries = std::fs::read_dir(&replay_dir).unwrap();
    let replay_file = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.extension().map_or(false, |ext| ext == "wotreplay"));

    if let Some(path) = replay_file {
        println!("Testing with replay: {:?}", path);

        // We assume the version for these replays is known or we can pick a valid one for testing.
        // Let's use "wot_eu_1_25_0" as a test case, or whatever we have definitions for.
        // Since we created message_codes/wot_eu/_default.json, using "wot_eu_0_0_0" should at least load defaults.
        
        let output = Command::new("cargo")
            .args(&[
                "run", 
                "--", 
                "--input", path.to_str().unwrap(), 
                "--version", "wot_eu_test_version" // Should load defaults from wot_eu
            ])
            .output()
            .expect("Failed to run cargo run");

        if !output.status.success() {
             eprintln!("STDOUT: {}", String::from_utf8_lossy(&output.stdout));
             eprintln!("STDERR: {}", String::from_utf8_lossy(&output.stderr));
             panic!("Parser failed execution");
        }
        
        // rudimentary check of output
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Magic: 11343212"));
        assert!(stdout.contains("[No Definitions Loaded]")); // Because we passed a fake version, identifying wot_eu but no ids_ file
        assert!(stdout.contains("Successfully parsed"));
    }
}
