//! Standalone timing harness for end-to-end benchmarks.
//!
//! Compile:  rustc -O benches/timer.rs -o target/bench-timer
//! Usage:    bench-timer <iterations> <command> [args...]
//! Output:   min avg p50 p95 max   (all in microseconds)
//!
//! The parent process environment is inherited by the child command,
//! so set HOME (etc.) before invoking this binary.

use std::env;
use std::process::{Command, Stdio};
use std::time::Instant;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("Usage: bench-timer <iterations> <command> [args...]");
        eprintln!("Outputs: min avg p50 p95 max (microseconds)");
        std::process::exit(1);
    }

    let iters: usize = args[0]
        .parse()
        .expect("first argument must be an iteration count");
    let cmd = &args[1];
    let cmd_args = &args[2..];

    // Warmup
    for _ in 0..5 {
        Command::new(cmd)
            .args(cmd_args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .ok();
    }

    // Benchmark
    let mut times = Vec::with_capacity(iters);
    for _ in 0..iters {
        let start = Instant::now();
        Command::new(cmd)
            .args(cmd_args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        times.push(start.elapsed().as_micros() as u64);
    }

    times.sort();
    let n = times.len();
    let sum: u64 = times.iter().sum();

    // min avg p50 p95 max
    println!(
        "{} {} {} {} {}",
        times[0],
        sum / n as u64,
        times[n * 50 / 100],
        times[n * 95 / 100],
        times[n - 1],
    );
}
