//! build.rs: Generates sandbox fixture .rec files under fixtures/sandbox/.
//!
//! Runs at cargo build time. Uses only std + bincode + serde (build-deps).
//! Re-runs only when scenario parameters change (OUT_DIR is a fixed path).

use std::io::Write;
use std::path::PathBuf;

fn main() {
    // Tell Cargo to re-run this script only if it changes.
    println!("cargo:rerun-if-changed=build.rs");

    // Locate workspace root (two levels up from crates/fsr-dshae/).
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let workspace_root = PathBuf::from(&manifest).parent().unwrap().parent().unwrap().to_path_buf();
    let fixtures_dir = workspace_root.join("fixtures").join("sandbox");

    // Create directory.
    std::fs::create_dir_all(&fixtures_dir).expect("failed to create fixtures/sandbox/");

    // Generate each scenario file if it doesn't already exist.
    let scenarios: &[(&str, u64)] = &[
        ("scenario_calm", 500),
        ("scenario_arb_single", 500),
        ("scenario_arb_recurring", 1000),
        ("scenario_noisy", 1000),
        ("scenario_regime_shift", 1000),
        ("scenario_correlation", 2000),
        ("scenario_lattice", 2000),
    ];

    for &(name, ticks) in scenarios {
        let path = fixtures_dir.join(format!("{}_000.rec", name));
        if !path.exists() {
            let bytes = generate_scenario(name, ticks);
            std::fs::write(&path, &bytes)
                .unwrap_or_else(|e| eprintln!("Warning: failed to write {}: {}", path.display(), e));
        }
    }
}

/// Simplified .rec file generator (mirrors recorder.rs format).
/// Format: [ u32_le frame_len | bincode(frame) ] ...
fn generate_scenario(name: &str, ticks: u64) -> Vec<u8> {
    let mut out = Vec::new();

    for tick in 0..ticks {
        let mids = tick_mids(name, tick);
        // Build a simple serializable frame.
        let frame = SimpleFrame { tick, mids };
        let encoded = simple_encode(&frame);
        let len = encoded.len() as u32;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&encoded);
    }

    out
}

fn tick_mids(scenario: &str, tick: u64) -> Vec<(u64, u64, i64)> {
    // Baseline no-arb rates for 4-currency basket (pairs indexed 0..5).
    let base: [(u64, u64, i64); 6] = [
        (0, 1, 9200),  // r_01 = 0.92
        (0, 2, 7912),  // r_02 = 0.7912
        (0, 3, 7516),  // r_03 = 0.7516
        (1, 2, 8600),  // r_12 = 0.86
        (1, 3, 8170),  // r_13 = 0.817
        (2, 3, 9500),  // r_23 = 0.95
    ];

    let mut mids: Vec<(u64, u64, i64)> = base.iter().map(|&(a, b, m)| (a, b, m)).collect();

    match scenario {
        "scenario_calm" => {} // no modification
        "scenario_arb_single" => {
            if tick >= 250 && tick < 300 {
                mids[1].2 = 7912 + 8; // ~10bp arb on r_02
            }
        }
        "scenario_arb_recurring" => {
            let arbs: [(u64, u64, i64); 5] =
                [(100, 130, 3), (250, 280, 5), (400, 430, 8), (600, 630, 12), (800, 830, 15)];
            for (start, end, bp) in arbs {
                if tick >= start && tick < end {
                    mids[1].2 = 7912 + 7912 * bp / 10000;
                }
            }
        }
        "scenario_noisy" => {
            // Sub-2bp pseudo-noise.
            let noise =
                (tick.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) % 3)
                    as i64;
            mids[1].2 = 7912 + noise;
        }
        "scenario_regime_shift" => {
            if tick > 200 && tick <= 700 {
                if tick >= 250 && tick < 280 {
                    mids[1].2 = 7912 + 8; // 10bp
                } else if tick >= 550 && tick < 580 {
                    mids[1].2 = 7912 + 8; // 10bp
                }
            }
        }
        "scenario_correlation" => {
            if tick < 1500 {
                // Correlated phase: synchronized noise (all rates move together).
                let sync_noise = ((tick.wrapping_mul(2654435761)) % 3) as i64 - 1; // {-1,0,1}
                for pair in mids.iter_mut() {
                    let bp_delta = sync_noise * pair.2 / 10000;
                    pair.2 += bp_delta;
                }
            } else {
                // Decorrelated phase: independent noise per pair.
                for (idx, pair) in mids.iter_mut().enumerate() {
                    let noise_seed = (tick.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407)
                        ^ (idx as u64 * 1000)) as i64;
                    let noise = ((noise_seed % 7) - 3).abs() % 4;
                    pair.2 += noise * pair.2 / 10000;
                }
            }
        }
        "scenario_lattice" => {
            // Stable ratio constraint with slow drift, inject arb at ticks 1000-1100.
            if tick >= 1000 && tick < 1100 {
                mids[1].2 = 7912 + 7912 * 8 / 10000; // 8bp arb
            } else {
                let drift = ((tick / 100) % 3) as i64; // 0,1,2 cycle
                mids[0].2 = base[0].2 + drift * base[0].2 / 1000;
                mids[3].2 = base[3].2 + drift * base[3].2 / 1000;
            }
        }
        _ => {}
    }

    mids
}

/// Very simple frame structure for build.rs (no external crates for serialization).
struct SimpleFrame {
    tick: u64,
    mids: Vec<(u64, u64, i64)>,
}

/// Manual simple encoding: tick (u64 le) + count (u32 le) + (u64 + u64 + i64) * count.
fn simple_encode(frame: &SimpleFrame) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&frame.tick.to_le_bytes());
    buf.extend_from_slice(&(frame.mids.len() as u32).to_le_bytes());
    for &(a, b, m) in &frame.mids {
        buf.extend_from_slice(&a.to_le_bytes());
        buf.extend_from_slice(&b.to_le_bytes());
        buf.extend_from_slice(&m.to_le_bytes());
    }
    buf
}
