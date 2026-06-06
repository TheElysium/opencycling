//! Seed N fake sessions into the local opencycling.db.
//!
//! Each session is built from a real .zwo workout fixture under
//! `src-tauri/tests/fixtures/` (chosen at random), parsed via the same
//! `parse_zwo` the app uses, flattened, then inserted through the real
//! `DbActorHandle` so the rows look exactly like a real run.
//!
//! Usage:
//!   cargo run --bin seed_history -- 15
//!   cargo run --bin seed_history -- 5 --ftp 230 --days-back 30 --seed 42
//!   cargo run --bin seed_history -- 5 --db "C:/path/to/opencycling.db"

use chrono::{Duration as CDuration, Utc};
use opencycling_lib::db::{DbActorHandle, Metric};
use opencycling_lib::workout::{parse_zwo, ParsedWorkout, WorkoutBlock};
use serde::Serialize;
use std::env;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_BUNDLE: &str = "com.ranchcorp.opencycling.app";
const DB_FILE: &str = "opencycling.db";
const DEFAULT_FTP: u16 = 200;
const DEFAULT_DAYS_BACK: f64 = 60.0;

#[derive(Serialize, Clone)]
struct FlatBlock {
    duration_s: u32,
    power_start_w: u16,
    power_end_w: u16,
    cadence_rpm: Option<u16>,
    label: String,
}

struct Args {
    count: u32,
    db: Option<PathBuf>,
    ftp: u16,
    days_back: f64,
    fixtures_dir: PathBuf,
    seed: Option<u64>,
}

fn print_help() {
    eprintln!(
        "Usage: cargo run --bin seed_history -- N \
         [--db PATH] [--ftp W] [--days-back D] [--fixtures-dir DIR] [--seed S]"
    );
}

fn parse_args() -> Result<Args, String> {
    let mut it = env::args().skip(1);
    let mut count: Option<u32> = None;
    let mut db: Option<PathBuf> = None;
    let mut ftp: u16 = DEFAULT_FTP;
    let mut days_back: f64 = DEFAULT_DAYS_BACK;
    let mut fixtures_dir: PathBuf =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut seed: Option<u64> = None;
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--db" => db = Some(PathBuf::from(it.next().ok_or("--db needs a value")?)),
            "--ftp" => {
                ftp = it
                    .next()
                    .ok_or("--ftp needs a value")?
                    .parse()
                    .map_err(|e: std::num::ParseIntError| e.to_string())?
            }
            "--days-back" => {
                days_back = it
                    .next()
                    .ok_or("--days-back needs a value")?
                    .parse()
                    .map_err(|e: std::num::ParseFloatError| e.to_string())?
            }
            "--fixtures-dir" => {
                fixtures_dir = PathBuf::from(it.next().ok_or("--fixtures-dir needs a value")?)
            }
            "--seed" => {
                seed = Some(
                    it.next()
                        .ok_or("--seed needs a value")?
                        .parse()
                        .map_err(|e: std::num::ParseIntError| e.to_string())?,
                )
            }
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            s if count.is_none() => {
                count = Some(
                    s.parse()
                        .map_err(|e: std::num::ParseIntError| e.to_string())?,
                )
            }
            other => return Err(format!("unexpected arg: {other}")),
        }
    }
    Ok(Args {
        count: count.ok_or("missing N (number of sessions)")?,
        db,
        ftp,
        days_back,
        fixtures_dir,
        seed,
    })
}

fn default_db_path() -> Result<PathBuf, String> {
    let appdata =
        env::var("APPDATA").map_err(|_| "APPDATA not set; pass --db explicitly".to_string())?;
    Ok(PathBuf::from(appdata).join(DEFAULT_BUNDLE).join(DB_FILE))
}

/// xorshift64 PRNG — no external dep needed, deterministic with a seed.
struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        Self(if seed == 0 {
            0xdead_beef_cafe_babe
        } else {
            seed
        })
    }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn uniform(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / ((1u64 << 53) as f64)
    }
    fn gauss(&mut self, mean: f64, std: f64) -> f64 {
        let u1 = self.uniform().max(1e-12);
        let u2 = self.uniform();
        let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
        mean + std * z
    }
    fn choice<'a, T>(&mut self, slice: &'a [T]) -> &'a T {
        &slice[(self.next_u64() as usize) % slice.len()]
    }
}

fn flatten_workout(parsed: ParsedWorkout, ftp: u16) -> Vec<FlatBlock> {
    let mut out = Vec::new();
    for block in parsed.workout_blocks {
        flatten_block(block, ftp, None, &mut out);
    }
    out
}

fn flatten_block(
    block: WorkoutBlock,
    ftp: u16,
    override_label: Option<String>,
    out: &mut Vec<FlatBlock>,
) {
    match block {
        WorkoutBlock::SteadyState {
            duration_s,
            power_pct,
            cadence_rpm,
            label,
        } => {
            let w = (power_pct * ftp as f32).round() as u16;
            let resolved = override_label
                .or(label)
                .unwrap_or_else(|| "Steady".to_string());
            out.push(FlatBlock {
                duration_s,
                power_start_w: w,
                power_end_w: w,
                cadence_rpm,
                label: resolved,
            });
        }
        WorkoutBlock::Ramp {
            duration_s,
            power_start_pct,
            power_end_pct,
            cadence_rpm,
            label,
        } => {
            let s = (power_start_pct * ftp as f32).round() as u16;
            let e = (power_end_pct * ftp as f32).round() as u16;
            let resolved = override_label
                .or(label)
                .unwrap_or_else(|| "Ramp".to_string());
            out.push(FlatBlock {
                duration_s,
                power_start_w: s,
                power_end_w: e,
                cadence_rpm,
                label: resolved,
            });
        }
        WorkoutBlock::IntervalsT { repeat, on, off } => {
            for i in 0..repeat {
                let on_label = format!("Interval {}/{} ON", i + 1, repeat);
                let off_label = format!("Interval {}/{} OFF", i + 1, repeat);
                flatten_block(*on.clone(), ftp, Some(on_label), out);
                flatten_block(*off.clone(), ftp, Some(off_label), out);
            }
        }
    }
}

fn load_fixtures(dir: &PathBuf, ftp: u16) -> Result<Vec<(String, Vec<FlatBlock>)>, String> {
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("read fixtures dir {}: {e}", dir.display()))?;
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("zwo"))
        .collect();
    paths.sort();

    let mut fixtures = Vec::new();
    for path in paths {
        let content =
            std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let parsed = match parse_zwo(&content) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("  skip {}: {e}", path.display());
                continue;
            }
        };
        let name = parsed
            .name
            .clone()
            .unwrap_or_else(|| path.file_stem().unwrap().to_string_lossy().into_owned());
        let blocks = flatten_workout(parsed, ftp);
        if !blocks.is_empty() {
            fixtures.push((name, blocks));
        }
    }
    Ok(fixtures)
}

fn generate_metrics(blocks: &[FlatBlock], rng: &mut Rng) -> Vec<Metric> {
    let mut out = Vec::new();
    let mut t: u32 = 0;
    let mut hr: f64 = 95.0;
    for b in blocks {
        let dur = b.duration_s;
        let p_start = b.power_start_w as f64;
        let p_end = b.power_end_w as f64;
        let cad_target = b.cadence_rpm.unwrap_or(88) as f64;
        for i in 0..dur {
            let ratio = if dur > 0 { i as f64 / dur as f64 } else { 0.0 };
            let base = p_start + (p_end - p_start) * ratio;
            let noise_std = (base * 0.04).max(5.0);
            let power = (base + rng.gauss(0.0, noise_std)).max(0.0) as u16;
            let hr_target = 80.0 + (power as f64 / 250.0) * 90.0;
            hr += (hr_target - hr) * 0.02 + rng.gauss(0.0, 0.5);
            let cadence = (cad_target + rng.gauss(0.0, 3.0)).max(0.0) as u16;
            out.push(Metric {
                t_offset_s: t,
                power_w: Some(power),
                hr_bpm: Some(hr as u16),
                cadence_rpm: Some(cadence),
            });
            t += 1;
        }
    }
    out
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}");
            print_help();
            std::process::exit(2);
        }
    };

    let db_path = match args.db.clone() {
        Some(p) => p,
        None => default_db_path()?,
    };
    if !db_path.exists() {
        return Err(format!(
            "DB not found: {}\nLaunch the app once to create it, or pass --db.",
            db_path.display()
        )
        .into());
    }

    let fixtures = load_fixtures(&args.fixtures_dir, args.ftp)?;
    if fixtures.is_empty() {
        return Err(format!("no usable .zwo fixtures in {}", args.fixtures_dir.display()).into());
    }
    println!(
        "Loaded {} fixture(s): {}",
        fixtures.len(),
        fixtures
            .iter()
            .map(|(n, _)| n.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );

    let seed = args.seed.unwrap_or_else(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64
    });
    let mut rng = Rng::new(seed);

    let handle = DbActorHandle::spawn(db_path.to_string_lossy().to_string()).await?;
    let now = Utc::now();

    for i in 0..args.count {
        let (name, blocks) = rng.choice(&fixtures).clone();
        let offset_s = rng.uniform() * args.days_back * 86_400.0 + rng.uniform() * 12.0 * 3_600.0;
        let started_at_dt = now - CDuration::seconds(offset_s as i64);
        let started_at = started_at_dt.to_rfc3339();
        let duration_s: u32 = blocks.iter().map(|b| b.duration_s).sum();
        let flat_blocks_json = serde_json::to_string(&blocks)?;
        let metrics = generate_metrics(&blocks, &mut rng);

        let sid = handle
            .insert_session(name.clone(), started_at.clone(), args.ftp, flat_blocks_json)
            .await?;
        for m in metrics {
            handle.insert_metric(sid, m).await;
        }
        let ended_at = (started_at_dt + CDuration::seconds(duration_s as i64)).to_rfc3339();
        handle.finalize_session(sid, ended_at, duration_s).await;
        println!(
            "[{:>3}/{}] id={sid:<4} {name:<40} @ {started_at}",
            i + 1,
            args.count
        );
    }

    // Barrier: ListSessions has a reply, so awaiting it guarantees every
    // prior fire-and-forget InsertMetric/FinalizeSession has been processed
    // (mpsc is strictly FIFO).
    let _ = handle.list_sessions().await?;

    println!("Done. DB: {}", db_path.display());
    Ok(())
}
