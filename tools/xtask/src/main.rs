use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

const DEFAULT_CRITERION_DIR: &str = "target/criterion";
const NEW_BASELINE_DIR: &str = "new";
const BENCH_TARGETS: &[BenchTarget] = &[
    BenchTarget {
        package: "zap-core",
        bench: "protocol",
        path: "crates/zap-core/benches/protocol.rs",
    },
    BenchTarget {
        package: "zap-crypto",
        bench: "signature",
        path: "crates/zap-crypto/benches/signature.rs",
    },
    BenchTarget {
        package: "zap-envelope",
        bench: "envelope",
        path: "crates/zap-envelope/benches/envelope.rs",
    },
    BenchTarget {
        package: "zap-net",
        bench: "round_trip",
        path: "crates/zap-net/benches/round_trip.rs",
    },
    BenchTarget {
        package: "zap-node",
        bench: "dispatch",
        path: "crates/zap-node/benches/dispatch.rs",
    },
    BenchTarget {
        package: "zap-runtime",
        bench: "runtime",
        path: "crates/zap-runtime/benches/runtime.rs",
    },
];

fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    match (args.next().as_deref(), args.next().as_deref()) {
        (None, _) | (Some("help" | "-h" | "--help"), _) => {
            print_usage();
            Ok(())
        }
        (Some("bench"), Some("help" | "-h" | "--help")) => {
            print_usage();
            Ok(())
        }
        (Some("bench"), Some("run")) => bench_run(RunOptions::parse(args)?),
        (Some("bench"), Some("collect")) => bench_collect(CollectOptions::parse(args)?),
        (Some("bench"), Some("compare")) => bench_compare(CompareOptions::parse(args)?),
        (Some("bench"), Some("site")) => bench_site(SiteOptions::parse(args)?),
        _ => {
            bail!("{}", usage());
        }
    }
}

fn print_usage() {
    println!("{}", usage());
}

fn usage() -> &'static str {
    "usage: xtask bench <run|collect|compare|site> [options]\n\
     run: [--sample-size <n>] [--warm-up-time <sec>] [--measurement-time <sec>] [--only <package/bench>]\n\
     collect: --out <path> [--input <criterion-dir>] [--label <label>] [--source-sha <sha>]\n\
     compare: --base <path> --head <path> --thresholds <path> [--out <markdown>]\n\
     site: --current <path> --out <dir> [--history-in <path>]"
}

#[derive(Debug, Clone, Copy)]
struct BenchTarget {
    package: &'static str,
    bench: &'static str,
    path: &'static str,
}

#[derive(Debug)]
struct RunOptions {
    sample_size: String,
    warm_up_time: String,
    measurement_time: String,
    only: Option<String>,
}

impl RunOptions {
    fn parse(args: impl Iterator<Item = String>) -> Result<Self> {
        let mut sample_size = "30".to_string();
        let mut warm_up_time = "3".to_string();
        let mut measurement_time = "5".to_string();
        let mut only = None;
        let mut args = args.peekable();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--sample-size" => sample_size = next_value(&mut args, "--sample-size")?,
                "--warm-up-time" => warm_up_time = next_value(&mut args, "--warm-up-time")?,
                "--measurement-time" => {
                    measurement_time = next_value(&mut args, "--measurement-time")?
                }
                "--only" => only = Some(next_value(&mut args, "--only")?),
                other => bail!("unknown run option `{other}`"),
            }
        }
        Ok(Self {
            sample_size,
            warm_up_time,
            measurement_time,
            only,
        })
    }
}

#[derive(Debug)]
struct CollectOptions {
    input: PathBuf,
    out: PathBuf,
    label: String,
    source_sha: Option<String>,
}

impl CollectOptions {
    fn parse(args: impl Iterator<Item = String>) -> Result<Self> {
        let mut input = PathBuf::from(DEFAULT_CRITERION_DIR);
        let mut out = None;
        let mut label = "current".to_string();
        let mut source_sha = None;
        let mut args = args.peekable();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--input" => input = next_path(&mut args, "--input")?,
                "--out" => out = Some(next_path(&mut args, "--out")?),
                "--label" => label = next_value(&mut args, "--label")?,
                "--source-sha" => source_sha = Some(next_value(&mut args, "--source-sha")?),
                other => bail!("unknown collect option `{other}`"),
            }
        }
        Ok(Self {
            input,
            out: out.context("missing --out")?,
            label,
            source_sha,
        })
    }
}

#[derive(Debug)]
struct CompareOptions {
    base: PathBuf,
    head: PathBuf,
    thresholds: PathBuf,
    out: Option<PathBuf>,
}

impl CompareOptions {
    fn parse(args: impl Iterator<Item = String>) -> Result<Self> {
        let mut base = None;
        let mut head = None;
        let mut thresholds = None;
        let mut out = None;
        let mut args = args.peekable();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--base" => base = Some(next_path(&mut args, "--base")?),
                "--head" => head = Some(next_path(&mut args, "--head")?),
                "--thresholds" => thresholds = Some(next_path(&mut args, "--thresholds")?),
                "--out" => out = Some(next_path(&mut args, "--out")?),
                other => bail!("unknown compare option `{other}`"),
            }
        }
        Ok(Self {
            base: base.context("missing --base")?,
            head: head.context("missing --head")?,
            thresholds: thresholds.context("missing --thresholds")?,
            out,
        })
    }
}

#[derive(Debug)]
struct SiteOptions {
    current: PathBuf,
    history_in: Option<PathBuf>,
    out: PathBuf,
}

impl SiteOptions {
    fn parse(args: impl Iterator<Item = String>) -> Result<Self> {
        let mut current = None;
        let mut history_in = None;
        let mut out = None;
        let mut args = args.peekable();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--current" => current = Some(next_path(&mut args, "--current")?),
                "--history-in" => history_in = Some(next_path(&mut args, "--history-in")?),
                "--out" => out = Some(next_path(&mut args, "--out")?),
                other => bail!("unknown site option `{other}`"),
            }
        }
        Ok(Self {
            current: current.context("missing --current")?,
            history_in,
            out: out.context("missing --out")?,
        })
    }
}

fn next_value(
    args: &mut std::iter::Peekable<impl Iterator<Item = String>>,
    option: &str,
) -> Result<String> {
    args.next()
        .with_context(|| format!("missing value for {option}"))
}

fn next_path(
    args: &mut std::iter::Peekable<impl Iterator<Item = String>>,
    option: &str,
) -> Result<PathBuf> {
    Ok(PathBuf::from(next_value(args, option)?))
}

fn bench_run(options: RunOptions) -> Result<()> {
    let targets = BENCH_TARGETS
        .iter()
        .copied()
        .filter(|target| {
            let target_id = format!("{}/{}", target.package, target.bench);
            options.only.as_deref().is_none_or(|only| only == target_id)
        })
        .collect::<Vec<_>>();
    if targets.is_empty() {
        bail!("no benchmark targets matched the requested filter");
    }

    let mut runnable_targets = Vec::new();
    for target in targets {
        let target_id = format!("{}/{}", target.package, target.bench);
        if !Path::new(target.path).exists() {
            println!("skipping missing benchmark target {target_id}");
            continue;
        }
        runnable_targets.push(target);
    }
    if runnable_targets.is_empty() {
        bail!("no benchmark target files were present for the requested filter");
    }

    let criterion_dir = Path::new(DEFAULT_CRITERION_DIR);
    if criterion_dir.exists() {
        fs::remove_dir_all(criterion_dir)
            .with_context(|| format!("failed to remove {}", criterion_dir.display()))?;
    }

    for target in runnable_targets {
        let target_id = format!("{}/{}", target.package, target.bench);
        println!("running benchmark target {target_id}");
        let status = Command::new("cargo")
            .args([
                "bench",
                "-p",
                target.package,
                "--locked",
                "--bench",
                target.bench,
                "--",
                "--noplot",
                "--sample-size",
                &options.sample_size,
                "--warm-up-time",
                &options.warm_up_time,
                "--measurement-time",
                &options.measurement_time,
                "--color",
                "never",
            ])
            .status()
            .with_context(|| format!("failed to launch benchmark target {target_id}"))?;
        if !status.success() {
            bail!("benchmark target {target_id} failed with status {status}");
        }
    }
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
struct BenchmarkReport {
    schema_version: u8,
    label: String,
    source_sha: Option<String>,
    generated_at_epoch_secs: u64,
    benchmarks: Vec<BenchmarkMetric>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BenchmarkMetric {
    id: String,
    samples: usize,
    mean_ns: f64,
    median_ns: f64,
    p95_ns: f64,
}

#[derive(Debug, Serialize, Deserialize)]
struct BenchmarkHistory {
    schema_version: u8,
    runs: Vec<BenchmarkReport>,
}

#[derive(Debug, Deserialize)]
struct ThresholdConfig {
    default: Threshold,
    #[serde(default)]
    benchmark: Vec<BenchmarkThreshold>,
}

#[derive(Debug, Clone, Deserialize)]
struct BenchmarkThreshold {
    pattern: String,
    #[serde(flatten)]
    threshold: Threshold,
}

#[derive(Debug, Clone, Deserialize)]
struct Threshold {
    relative_regression: f64,
    min_absolute_ns: f64,
    #[serde(default = "default_blocking")]
    blocking: bool,
}

fn default_blocking() -> bool {
    true
}

fn bench_collect(options: CollectOptions) -> Result<()> {
    let mut benchmark_files = Vec::new();
    collect_benchmark_files(&options.input, "raw.csv", &mut benchmark_files)?;
    if benchmark_files.is_empty() {
        collect_benchmark_files(&options.input, "sample.json", &mut benchmark_files)?;
    }
    benchmark_files.sort();
    let mut benchmarks = Vec::new();
    for benchmark_file in benchmark_files {
        let Some(id) = benchmark_id(&options.input, &benchmark_file) else {
            continue;
        };
        let samples = match benchmark_file.file_name().and_then(|name| name.to_str()) {
            Some("raw.csv") => parse_raw_csv(&benchmark_file),
            Some("sample.json") => parse_sample_json(&benchmark_file),
            _ => Ok(Vec::new()),
        }
        .with_context(|| format!("failed to parse {}", benchmark_file.display()))?;
        if samples.is_empty() {
            continue;
        }
        benchmarks.push(metric_from_samples(id, samples));
    }
    benchmarks.sort_by(|left, right| left.id.cmp(&right.id));
    if benchmarks.is_empty() {
        bail!(
            "no Criterion benchmark samples found under {}",
            options.input.display()
        );
    }
    let report = BenchmarkReport {
        schema_version: 1,
        label: options.label,
        source_sha: options.source_sha,
        generated_at_epoch_secs: now_epoch_secs()?,
        benchmarks,
    };
    write_json(&options.out, &report)?;
    println!("wrote {}", options.out.display());
    Ok(())
}

fn bench_compare(options: CompareOptions) -> Result<()> {
    let base: BenchmarkReport = read_json(&options.base)?;
    let head: BenchmarkReport = read_json(&options.head)?;
    let thresholds: ThresholdConfig = toml::from_str(
        &fs::read_to_string(&options.thresholds)
            .with_context(|| format!("failed to read {}", options.thresholds.display()))?,
    )?;

    let base_by_id: BTreeMap<_, _> = base
        .benchmarks
        .iter()
        .map(|metric| (metric.id.as_str(), metric))
        .collect();
    let mut regressions = Vec::new();
    let mut lines = vec![
        "# ZAP benchmark comparison".to_string(),
        String::new(),
        "| benchmark | base median | head median | delta | threshold | status |".to_string(),
        "| --- | ---: | ---: | ---: | ---: | --- |".to_string(),
    ];

    for head_metric in &head.benchmarks {
        let Some(base_metric) = base_by_id.get(head_metric.id.as_str()) else {
            lines.push(format!(
                "| `{}` | missing | {:.2} ns | n/a | n/a | new |",
                head_metric.id, head_metric.median_ns
            ));
            continue;
        };
        let threshold = threshold_for(&thresholds, &head_metric.id);
        let absolute = head_metric.median_ns - base_metric.median_ns;
        let relative = if base_metric.median_ns > 0.0 {
            absolute / base_metric.median_ns
        } else {
            0.0
        };
        let failed = threshold.blocking
            && absolute > threshold.min_absolute_ns
            && relative > threshold.relative_regression;
        let status = if failed { "regression" } else { "ok" };
        lines.push(format!(
            "| `{}` | {:.2} ns | {:.2} ns | {} | {} | {} |",
            head_metric.id,
            base_metric.median_ns,
            head_metric.median_ns,
            format_delta(relative, absolute),
            format_threshold(threshold),
            status
        ));
        if failed {
            regressions.push(head_metric.id.clone());
        }
    }

    let markdown = lines.join("\n") + "\n";
    if let Some(out) = options.out {
        write_text(&out, &markdown)?;
    }
    print!("{markdown}");
    if !regressions.is_empty() {
        bail!(
            "benchmark regressions exceeded thresholds: {}",
            regressions.join(", ")
        );
    }
    Ok(())
}

fn bench_site(options: SiteOptions) -> Result<()> {
    let current: BenchmarkReport = read_json(&options.current)?;
    let mut history = match options.history_in {
        Some(path) if path.exists() => read_json::<BenchmarkHistory>(&path)?,
        _ => BenchmarkHistory {
            schema_version: 1,
            runs: Vec::new(),
        },
    };
    history.runs.push(current);
    history
        .runs
        .sort_by_key(|report| report.generated_at_epoch_secs);
    if history.runs.len() > 200 {
        let drain = history.runs.len() - 200;
        history.runs.drain(0..drain);
    }
    let latest = history
        .runs
        .last()
        .context("history unexpectedly empty after inserting current run")?;

    fs::create_dir_all(&options.out)
        .with_context(|| format!("failed to create {}", options.out.display()))?;
    write_json(&options.out.join("latest.json"), latest)?;
    write_json(&options.out.join("history.json"), &history)?;
    write_text(
        &options.out.join("index.html"),
        &render_site(latest, &history),
    )?;
    write_text(&options.out.join("badge.svg"), &render_badge(latest))?;
    println!("wrote benchmark site {}", options.out.display());
    Ok(())
}

fn collect_benchmark_files(dir: &Path, file_name: &str, out: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_benchmark_files(&path, file_name, out)?;
        } else if path.file_name().and_then(|name| name.to_str()) == Some(file_name) {
            out.push(path);
        }
    }
    Ok(())
}

fn benchmark_id(root: &Path, raw_file: &Path) -> Option<String> {
    let parent = raw_file.parent()?;
    let baseline = parent.file_name()?.to_str()?;
    if baseline != NEW_BASELINE_DIR {
        return None;
    }
    let bench_dir = parent.parent()?;
    let relative = bench_dir.strip_prefix(root).ok()?;
    let parts = relative
        .iter()
        .filter_map(|part| part.to_str())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

fn parse_raw_csv(path: &Path) -> Result<Vec<f64>> {
    let input = fs::read_to_string(path)?;
    let mut lines = input.lines();
    let header = lines.next().context("missing raw.csv header")?;
    let columns = split_csv_line(header);
    let measured_idx = columns
        .iter()
        .position(|column| *column == "sample_measured_value")
        .or_else(|| columns.iter().position(|column| *column == "sample_time"))
        .context("raw.csv is missing sample measured value column")?;
    let iterations_idx = columns
        .iter()
        .position(|column| *column == "iteration_count")
        .or_else(|| columns.iter().position(|column| *column == "iterations"));
    let unit_idx = columns.iter().position(|column| *column == "unit");

    let mut samples = Vec::new();
    for line in lines.filter(|line| !line.trim().is_empty()) {
        let fields = split_csv_line(line);
        let measured = parse_f64(fields.get(measured_idx), "sample measured value")?;
        let iterations = match iterations_idx.and_then(|idx| fields.get(idx)) {
            Some(value) => parse_f64(Some(value), "iteration count")?.max(1.0),
            None => 1.0,
        };
        let unit = unit_idx
            .and_then(|idx| fields.get(idx))
            .copied()
            .unwrap_or("ns");
        samples.push(to_nanoseconds(measured, unit) / iterations);
    }
    Ok(samples)
}

#[derive(Debug, Deserialize)]
struct CriterionSample {
    iters: Vec<f64>,
    times: Vec<f64>,
}

fn parse_sample_json(path: &Path) -> Result<Vec<f64>> {
    let sample: CriterionSample = read_json(path)?;
    if sample.iters.len() != sample.times.len() {
        bail!(
            "sample.json length mismatch: {} iteration counts for {} times",
            sample.iters.len(),
            sample.times.len()
        );
    }
    Ok(sample
        .times
        .iter()
        .zip(sample.iters.iter())
        .map(|(time, iterations)| time / iterations.max(1.0))
        .collect())
}

fn split_csv_line(line: &str) -> Vec<&str> {
    line.split(',').map(str::trim).collect()
}

fn parse_f64(value: Option<&&str>, label: &str) -> Result<f64> {
    value
        .copied()
        .context(format!("missing {label}"))?
        .parse::<f64>()
        .with_context(|| format!("invalid {label}"))
}

fn to_nanoseconds(value: f64, unit: &str) -> f64 {
    match unit {
        "s" | "sec" | "second" | "seconds" => value * 1_000_000_000.0,
        "ms" | "millisecond" | "milliseconds" => value * 1_000_000.0,
        "us" | "µs" | "microsecond" | "microseconds" => value * 1_000.0,
        _ => value,
    }
}

fn metric_from_samples(id: String, mut samples: Vec<f64>) -> BenchmarkMetric {
    samples.sort_by(|left, right| left.partial_cmp(right).unwrap_or(Ordering::Equal));
    let samples_len = samples.len();
    let mean_ns = samples.iter().sum::<f64>() / samples_len as f64;
    BenchmarkMetric {
        id,
        samples: samples_len,
        mean_ns,
        median_ns: percentile(&samples, 0.50),
        p95_ns: percentile(&samples, 0.95),
    }
}

fn percentile(sorted: &[f64], percentile: f64) -> f64 {
    let idx = ((sorted.len().saturating_sub(1)) as f64 * percentile).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn threshold_for<'a>(config: &'a ThresholdConfig, id: &str) -> &'a Threshold {
    config
        .benchmark
        .iter()
        .find(|entry| id.contains(&entry.pattern))
        .map(|entry| &entry.threshold)
        .unwrap_or(&config.default)
}

fn format_delta(relative: f64, absolute_ns: f64) -> String {
    format!("{:+.2}% ({:+.2} ns)", relative * 100.0, absolute_ns)
}

fn format_threshold(threshold: &Threshold) -> String {
    format!(
        "{:.2}% and {:.2} ns",
        threshold.relative_regression * 100.0,
        threshold.min_absolute_ns
    )
}

fn render_site(latest: &BenchmarkReport, history: &BenchmarkHistory) -> String {
    let mut rows = String::new();
    for metric in &latest.benchmarks {
        rows.push_str(&format!(
            "<tr><td><code>{}</code></td><td>{:.2}</td><td>{:.2}</td><td>{:.2}</td><td>{}</td></tr>\n",
            escape_html(&metric.id),
            metric.median_ns,
            metric.mean_ns,
            metric.p95_ns,
            metric.samples
        ));
    }
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>ZAP Benchmarks</title>
  <style>
    :root {{ color-scheme: light dark; font-family: Inter, ui-sans-serif, system-ui, sans-serif; }}
    body {{ margin: 0; padding: 32px; background: Canvas; color: CanvasText; }}
    main {{ max-width: 1120px; margin: 0 auto; }}
    h1 {{ font-size: 32px; margin: 0 0 8px; }}
    p {{ color: color-mix(in srgb, CanvasText 72%, transparent); }}
    table {{ width: 100%; border-collapse: collapse; margin-top: 24px; }}
    th, td {{ border-bottom: 1px solid color-mix(in srgb, CanvasText 16%, transparent); padding: 10px 8px; text-align: right; }}
    th:first-child, td:first-child {{ text-align: left; }}
    code {{ font-family: ui-monospace, SFMono-Regular, Consolas, monospace; }}
  </style>
</head>
<body>
<main>
  <h1>ZAP Benchmarks</h1>
  <p>Latest run: <code>{label}</code> at epoch <code>{generated}</code>. Stored runs: {runs}.</p>
  <p><a href="./latest.json">latest.json</a> · <a href="./history.json">history.json</a> · <a href="./badge.svg">badge.svg</a></p>
  <table>
    <thead><tr><th>Benchmark</th><th>Median ns</th><th>Mean ns</th><th>P95 ns</th><th>Samples</th></tr></thead>
    <tbody>
{rows}
    </tbody>
  </table>
</main>
</body>
</html>
"#,
        label = escape_html(&latest.label),
        generated = latest.generated_at_epoch_secs,
        runs = history.runs.len(),
        rows = rows
    )
}

fn render_badge(latest: &BenchmarkReport) -> String {
    let label = format!("{} benches", latest.benchmarks.len());
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="150" height="20" role="img" aria-label="benchmarks: {label}">
<rect width="75" height="20" fill="#555"/>
<rect x="75" width="75" height="20" fill="#2f855a"/>
<text x="37" y="14" fill="#fff" font-family="Verdana,sans-serif" font-size="11" text-anchor="middle">benchmarks</text>
<text x="112" y="14" fill="#fff" font-family="Verdana,sans-serif" font-size="11" text-anchor="middle">{label}</text>
</svg>
"##,
        label = escape_html(&label)
    )
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn now_epoch_secs() -> Result<u64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    serde_json::from_str(
        &fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?,
    )
    .with_context(|| format!("failed to parse {}", path.display()))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    write_text(path, &(json + "\n"))
}

fn write_text(path: &Path, text: &str) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(path, text).with_context(|| format!("failed to write {}", path.display()))
}
