use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use chrono::{DateTime, TimeDelta, Utc};
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

use ccsesh::discover;
use ccsesh::display;
use ccsesh::parse;
use ccsesh::types::SessionInfo;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a benchmark environment with `size` session files spread across 5
/// project directories. Returns the fake HOME path. Idempotent — reuses data
/// if the directory already exists.
fn setup_discover_env(size: usize) -> String {
    let root = std::env::temp_dir().join(format!("ccsesh_criterion_{}", size));
    let projects = root.join(".claude").join("projects");
    let marker = projects.join(".bench_ready");

    if marker.exists() {
        return root.to_str().unwrap().to_string();
    }

    let _ = fs::remove_dir_all(&root);

    for p in 0..5 {
        fs::create_dir_all(projects.join(format!("project-{}", p))).unwrap();
    }

    let now = SystemTime::now();

    for i in 0..size {
        let proj = projects.join(format!("project-{}", i % 5));
        let uuid = format!(
            "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
            i * 7 + 12345,
            i * 3 + 100,
            i * 5 + 200,
            i * 11 + 300,
            i * 13 + 40000
        );
        let content = format!(
            concat!(
                "{{\"type\":\"system\",\"cwd\":\"/home/user/dev/project-{p}\",",
                "\"sessionId\":\"{uuid}\",\"message\":{{\"content\":\"init\"}}}}\n",
                "{{\"type\":\"user\",\"cwd\":\"/home/user/dev/project-{p}\",",
                "\"sessionId\":\"{uuid}\",",
                "\"message\":{{\"content\":\"Implement feature {i} with tests\"}}}}\n",
                "{{\"type\":\"assistant\",\"slug\":\"slug-{i}\",",
                "\"message\":{{\"content\":\"Working on it.\"}}}}\n",
            ),
            p = i % 5,
            uuid = uuid,
            i = i,
        );

        let path = proj.join(format!("{}.jsonl", uuid));
        fs::write(&path, content).unwrap();

        let mtime = now - Duration::from_secs(i as u64 * 60);
        let times = fs::FileTimes::new().set_modified(mtime);
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(times)
            .unwrap();
    }

    fs::write(&marker, "ok").unwrap();
    root.to_str().unwrap().to_string()
}

/// Build a synthetic `SessionInfo` for display benchmarks.
fn make_session(
    index: usize,
    now: DateTime<Utc>,
    prompt: Option<&str>,
    slug: Option<&str>,
) -> SessionInfo {
    SessionInfo {
        session_id: format!("{:08x}-{:04x}-{:04x}-{:04x}-{:012x}", index, 1, 2, 3, 4),
        path: PathBuf::from(format!(
            "/home/user/.claude/projects/proj/{:08x}-{:04x}-{:04x}-{:04x}-{:012x}.jsonl",
            index, 1, 2, 3, 4
        )),
        project_dir: PathBuf::from(format!("/home/user/dev/project-{}", index % 5)),
        project_dir_display: format!("~/dev/project-{}", index % 5),
        last_active: now - TimeDelta::seconds(index as i64 * 137),
        first_prompt: prompt.map(String::from),
        slug: slug.map(String::from),
    }
}

// ---------------------------------------------------------------------------
// Benchmarks: discover
// ---------------------------------------------------------------------------

fn bench_discover(c: &mut Criterion) {
    let mut group = c.benchmark_group("discover");

    for &size in &[5, 50, 100, 500, 1000] {
        let home = setup_discover_env(size);
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| discover::discover_sessions(&home, 5).unwrap());
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmarks: parse
// ---------------------------------------------------------------------------

fn bench_parse(c: &mut Criterion) {
    let home = setup_discover_env(50);
    let candidates = discover::discover_sessions(&home, 5).unwrap();
    let candidate = &candidates[0];

    c.bench_function("parse_session", |b| {
        b.iter(|| parse::parse_session(candidate, &home).unwrap());
    });
}

fn bench_strip_xml_tags(c: &mut Criterion) {
    let inputs = [
        ("no_tags", "Hello world, this is a simple prompt"),
        (
            "simple_tags",
            "<system-reminder>context loaded</system-reminder> Fix the CSS",
        ),
        (
            "nested_tags",
            "<a><b>hello</b></a> <c>world</c> <d>test prompt here</d>",
        ),
        (
            "newline_bailout",
            "< 10 mins remaining\nPlease wrap up the current task",
        ),
    ];

    let mut group = c.benchmark_group("strip_xml_tags");
    for (name, input) in &inputs {
        group.bench_with_input(BenchmarkId::new("input", name), input, |b, s| {
            b.iter(|| parse::strip_xml_tags(s));
        });
    }
    group.finish();
}

fn bench_extract_text(c: &mut Criterion) {
    let string_val = serde_json::json!("Hello world prompt text");
    let array_val = serde_json::json!([
        {"type": "tool_result", "content": "some result"},
        {"type": "image", "source": {"type": "base64", "data": "abc123"}},
        {"type": "text", "text": "The actual user prompt here"}
    ]);

    let mut group = c.benchmark_group("extract_text");
    group.bench_function("string", |b| {
        b.iter(|| parse::extract_text_from_content(&string_val));
    });
    group.bench_function("array", |b| {
        b.iter(|| parse::extract_text_from_content(&array_val));
    });
    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmarks: display
// ---------------------------------------------------------------------------

fn bench_display(c: &mut Criterion) {
    let now: DateTime<Utc> = DateTime::parse_from_rfc3339("2026-02-18T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);

    let sessions_5: Vec<SessionInfo> = (0..5)
        .map(|i| {
            make_session(
                i,
                now,
                Some("Implement feature with comprehensive error handling and unit tests"),
                Some("woolly-conjuring-journal"),
            )
        })
        .collect();

    let sessions_20: Vec<SessionInfo> = (0..20)
        .map(|i| {
            make_session(
                i,
                now,
                Some("Implement feature with comprehensive error handling and unit tests"),
                Some("woolly-conjuring-journal"),
            )
        })
        .collect();

    let mut group = c.benchmark_group("display");

    group.bench_function("format_default_5", |b| {
        b.iter(|| display::format_default(&sessions_5, now));
    });
    group.bench_function("format_short_5", |b| {
        b.iter(|| display::format_short(&sessions_5, now));
    });
    group.bench_function("format_json_5", |b| {
        b.iter(|| display::format_json(&sessions_5, now));
    });
    group.bench_function("format_default_20", |b| {
        b.iter(|| display::format_default(&sessions_20, now));
    });
    group.bench_function("format_json_20", |b| {
        b.iter(|| display::format_json(&sessions_20, now));
    });

    group.finish();
}

fn bench_truncate_prompt(c: &mut Criterion) {
    let short = "Fix the bug";
    let long = "Fix the pagination bug in the users endpoint for the API server module \
                that is causing issues with offset calculations when filtering by role";

    let mut group = c.benchmark_group("truncate_prompt");
    group.bench_function("short_within_limit", |b| {
        b.iter(|| display::truncate_prompt(short, 72));
    });
    group.bench_function("long_needs_truncation", |b| {
        b.iter(|| display::truncate_prompt(long, 72));
    });
    group.finish();
}

fn bench_relative_time(c: &mut Criterion) {
    let durations = [
        ("30s", TimeDelta::seconds(30)),
        ("5m", TimeDelta::seconds(300)),
        ("2h", TimeDelta::seconds(7200)),
        ("3d", TimeDelta::seconds(259200)),
    ];

    let mut group = c.benchmark_group("relative_time");
    for (name, dur) in &durations {
        group.bench_with_input(BenchmarkId::new("format", name), dur, |b, d| {
            b.iter(|| display::format_relative_time(*d));
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Criterion groups
// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    bench_discover,
    bench_parse,
    bench_strip_xml_tags,
    bench_extract_text,
    bench_display,
    bench_truncate_prompt,
    bench_relative_time,
);
criterion_main!(benches);
