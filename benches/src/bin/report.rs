//! Turns the latest Criterion run into a page of the site's Performance section.
//!
//! Criterion leaves one directory per benchmark under `target/criterion`, each with a
//! `new/benchmark.json` naming the group and the function and a `new/estimates.json` holding
//! the statistics of the last run. This binary reads them, pairs the `orm` and `raw` (or `sql`)
//! sides of every group, and writes `docs/performance/turso-<version>.md`, where the version is
//! the `turso` release resolved in `Cargo.lock`: the same version overwrites its page, a new one
//! adds a page, and the pages are the history. It then makes sure `zensical.toml` lists the page
//! in the Performance section, inserting the entry under the marker comment when the version is
//! new, and that its `extra.turso_version`, which the home page hero shows, names the same
//! version, so that a `turso` upgrade needs no edit of the site configuration by hand.
//!
//! The page records the machine and the toolchain next to the numbers. Absolute times are only
//! comparable on one machine; the ORM-to-raw ratio in the last column is comparable anywhere and
//! is what the Explain page asks the reader to look at.

#![allow(
    clippy::print_stdout,
    reason = "a command-line tool reports what it wrote"
)]

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

/// The comment of `zensical.toml` under which the version pages are listed, newest first.
const MARKER: &str = "# report: versions";
/// The key of `zensical.toml` that the home page hero shows as the engine version.
const VERSION_KEY: &str = "turso_version = ";
/// The order of the tables in a fragment: the first segment of a Criterion group id.
const SECTIONS: [&str; 6] = [
    "select",
    "insert",
    "update",
    "delete",
    "relations",
    "render",
];

/// One side of one Criterion group, in nanoseconds.
#[derive(Debug, Default)]
struct Row {
    /// The mean of the `orm` function, or of the single function of an unpaired group.
    orm: Option<f64>,
    /// The mean of the `raw` or `sql` function.
    raw: Option<f64>,
    /// The elements per iteration when the group declared a throughput.
    elements: Option<u64>,
}

/// What the header of a fragment says about the run.
#[derive(Debug)]
struct Context {
    /// The `turso` version resolved in `Cargo.lock`.
    turso: String,
    /// The `turso-orm` version resolved in `Cargo.lock`.
    orm: String,
    /// The date of the run, `YYYY-MM-DD` in UTC.
    date: String,
    /// The processor model and the number of logical cores.
    machine: String,
    /// The operating system and the architecture.
    platform: String,
    /// The `rustc --version` line.
    rustc: String,
}

/// Reads the run, writes the fragment and wires it into the page.
fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root();
    let lock = fs::read_to_string(root.join("Cargo.lock"))?;
    let context = Context {
        turso: locked_version(&lock, "turso").ok_or("turso is not in Cargo.lock")?,
        orm: locked_version(&lock, "turso-orm").ok_or("turso-orm is not in Cargo.lock")?,
        date: today(),
        machine: machine(),
        platform: format!("{}/{}", std::env::consts::OS, std::env::consts::ARCH),
        rustc: rustc_version(),
    };
    let rows = read_rows(&root.join("target").join("criterion"))?;
    if rows.is_empty() {
        return Err("no Criterion results under target/criterion; run `cargo bench` first".into());
    }
    let page = render(&rows, &context);
    let section = root.join("docs").join("performance");
    fs::create_dir_all(&section)?;
    let path = section.join(format!("turso-{}.md", context.turso));
    fs::write(&path, page)?;
    println!("wrote {}", path.display());

    let config = root.join("zensical.toml");
    if ensure_listed(&config, &context.turso)? {
        println!(
            "added Turso {} to the Performance section of {}",
            context.turso,
            config.display()
        );
    }
    if ensure_version(&config, &context.turso)? {
        println!(
            "set the hero's turso version to {} in {}",
            context.turso,
            config.display()
        );
    }
    Ok(())
}

/// The repository root: the parent of this crate's directory.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

/// The version of `name` in a `Cargo.lock`, read from the `version` line that follows its
/// `name` line.
fn locked_version(lock: &str, name: &str) -> Option<String> {
    let header = format!("name = \"{name}\"");
    let mut lines = lock.lines();
    while let Some(line) = lines.next() {
        if line.trim() == header {
            return lines
                .next()?
                .trim()
                .strip_prefix("version = \"")?
                .strip_suffix('"')
                .map(str::to_owned);
        }
    }
    None
}

/// Walks `target/criterion` and pairs the sides of every group.
fn read_rows(criterion: &Path) -> Result<BTreeMap<String, Row>, Box<dyn Error>> {
    let mut rows: BTreeMap<String, Row> = BTreeMap::new();
    let mut pending = vec![criterion.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries {
            let path = entry?.path();
            if !path.is_dir() {
                continue;
            }
            let benchmark = path.join("new").join("benchmark.json");
            if benchmark.is_file() {
                record(&mut rows, &benchmark)?;
            } else {
                pending.push(path);
            }
        }
    }
    Ok(rows)
}

/// Adds one benchmark directory to the rows.
fn record(rows: &mut BTreeMap<String, Row>, benchmark: &Path) -> Result<(), Box<dyn Error>> {
    let meta: Value = serde_json::from_str(&fs::read_to_string(benchmark)?)?;
    let estimates: Value = serde_json::from_str(&fs::read_to_string(
        benchmark.with_file_name("estimates.json"),
    )?)?;
    let group = meta["group_id"]
        .as_str()
        .ok_or("benchmark.json without group_id")?;
    let mean = estimates["mean"]["point_estimate"]
        .as_f64()
        .ok_or("estimates.json without a mean")?;
    let row = rows.entry(group.to_owned()).or_default();
    match meta["function_id"].as_str() {
        Some("raw" | "sql") => row.raw = Some(mean),
        _ => row.orm = Some(mean),
    }
    if let Some(elements) = meta["throughput"]["Elements"].as_u64() {
        row.elements = Some(elements);
    }
    Ok(())
}

/// Renders the page: a title, a header paragraph, then one table per section.
fn render(rows: &BTreeMap<String, Row>, context: &Context) -> String {
    let mut out = format!(
        "# Turso {}\n\nMeasured on {} with turso-orm {} against turso {}, on {} ({}), {}. Each \
         cell is the Criterion mean of one benchmark; the last column divides the ORM's time by \
         the raw client's. The [Explain](explain.md) page says how to read these tables.\n",
        context.turso,
        context.date,
        context.orm,
        context.turso,
        context.machine,
        context.platform,
        context.rustc
    );
    let mut sections: Vec<&str> = SECTIONS.to_vec();
    for group in rows.keys() {
        let section = group.split('/').next().unwrap_or(group);
        if !sections.contains(&section) {
            sections.push(section);
        }
    }
    for section in sections {
        let in_section: Vec<(&String, &Row)> = rows
            .iter()
            .filter(|(group, _)| group.split('/').next() == Some(section))
            .collect();
        if in_section.is_empty() {
            continue;
        }
        let raw_label = if section == "render" {
            "builder"
        } else {
            "raw"
        };
        write!(
            out,
            "\n## `{section}`\n\n| Group | ORM | {raw_label} | Ratio |\n|---|---|---|---|\n"
        )
        .expect("writing to a String cannot fail");
        for (group, row) in in_section {
            let label = match row.elements {
                Some(n) => format!("`{group}` ({n} rows)"),
                None => format!("`{group}`"),
            };
            let ratio = match (row.orm, row.raw) {
                (Some(orm), Some(raw)) if raw > 0.0 => format!("{:.2}×", orm / raw),
                _ => "—".to_owned(),
            };
            writeln!(
                out,
                "| {label} | {} | {} | {ratio} |",
                row.orm.map_or_else(|| "—".to_owned(), duration),
                row.raw.map_or_else(|| "—".to_owned(), duration),
            )
            .expect("writing to a String cannot fail");
        }
    }
    out
}

/// Formats nanoseconds with the unit that keeps the number short.
fn duration(ns: f64) -> String {
    if ns < 1_000.0 {
        format!("{ns:.0} ns")
    } else if ns < 1_000_000.0 {
        format!("{:.2} µs", ns / 1_000.0)
    } else {
        format!("{:.2} ms", ns / 1_000_000.0)
    }
}

/// Inserts the nav entry of `version` under the marker comment of `zensical.toml` when it is
/// missing, with the indentation of the marker so that `taplo fmt` leaves the line alone.
///
/// Returns whether the configuration changed.
fn ensure_listed(config: &Path, version: &str) -> Result<bool, Box<dyn Error>> {
    let text = fs::read_to_string(config)?;
    let page = format!("performance/turso-{version}.md");
    if text.contains(&page) {
        return Ok(false);
    }
    let Some(line) = text.lines().find(|line| line.trim_start() == MARKER) else {
        return Err(format!("{} has no `{MARKER}` comment", config.display()).into());
    };
    let indent = &line[..line.len() - line.trim_start().len()];
    let entry = format!("{indent}{{ \"Turso {version}\" = \"{page}\" }},\n");
    let at = text.find(line).ok_or("marker vanished")? + line.len() + 1;
    let mut updated = String::with_capacity(text.len() + entry.len());
    updated.push_str(&text[..at]);
    updated.push_str(&entry);
    updated.push_str(&text[at..]);
    fs::write(config, updated)?;
    Ok(true)
}

/// Sets the `turso_version` key of `zensical.toml` to `version` when it differs.
///
/// Returns whether the configuration changed.
fn ensure_version(config: &Path, version: &str) -> Result<bool, Box<dyn Error>> {
    let text = fs::read_to_string(config)?;
    let wanted = format!("{VERSION_KEY}\"{version}\"");
    let Some(line) = text.lines().find(|line| line.starts_with(VERSION_KEY)) else {
        return Err(format!("{} has no `{VERSION_KEY}` key", config.display()).into());
    };
    if line == wanted {
        return Ok(false);
    }
    fs::write(config, text.replacen(line, &wanted, 1))?;
    Ok(true)
}

/// Today's date in UTC as `YYYY-MM-DD`, from the system clock alone.
fn today() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Converts days since 1970-01-01 to a proleptic Gregorian date (the classic era-based
/// algorithm, valid for every date this tool will ever see).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// The processor model from `/proc/cpuinfo` when there is one, and the logical core count.
fn machine() -> String {
    let model = fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|info| {
            info.lines()
                .find(|line| line.starts_with("model name"))
                .and_then(|line| line.split_once(':'))
                .map(|(_, model)| model.trim().to_owned())
        })
        .unwrap_or_else(|| "unknown processor".to_owned());
    let cores = std::thread::available_parallelism().map_or(0, std::num::NonZero::get);
    format!("{model}, {cores} logical cores")
}

/// The first line of `rustc --version`, or a placeholder when rustc is not on the path.
fn rustc_version() -> String {
    Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map_or_else(
            || "unknown rustc".to_owned(),
            |output| String::from_utf8_lossy(&output.stdout).trim().to_owned(),
        )
}
