#!/usr/bin/env bash
#
# ccsesh end-to-end benchmark suite
# Portable: Linux + macOS
#
# Usage:
#   ./benches/e2e.sh              # full run (100 iterations)
#   ./benches/e2e.sh --quick      # fast run  (30 iterations)
#   ./benches/e2e.sh --clean      # remove benchmark data and exit
#   ./benches/e2e.sh --help
#
# On macOS, install coreutils for nanosecond-precision date:
#   brew install coreutils
# Without it, the script uses a compiled Rust timer (always accurate).

set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BINARY="$PROJECT_DIR/target/release/ccsesh"
TIMER="$PROJECT_DIR/target/bench-timer"
BENCH_ROOT="${TMPDIR:-/tmp}/ccsesh_bench"
OS="$(uname -s)"

ITERS=100
SIZES=(5 10 50 100 500 1000)
LARGE_SIZES=(50 500 5000 50000)

# ---------------------------------------------------------------------------
# Argument parsing
# ---------------------------------------------------------------------------

for arg in "$@"; do
    case "$arg" in
        --quick)  ITERS=30; SIZES=(5 50 500 1000) ;;
        --clean)
            echo "Removing $BENCH_ROOT ..."
            rm -rf "$BENCH_ROOT"
            echo "Done."
            exit 0
            ;;
        --help|-h)
            echo "Usage: $0 [--quick] [--clean] [--help]"
            echo ""
            echo "  --quick   Run with fewer iterations (30 vs 100)"
            echo "  --clean   Remove benchmark data and exit"
            echo "  --help    Show this help"
            exit 0
            ;;
        *)
            echo "Unknown argument: $arg" >&2
            exit 1
            ;;
    esac
done

# ---------------------------------------------------------------------------
# Platform helpers
# ---------------------------------------------------------------------------

sysinfo() {
    local cpus mem kernel
    kernel="$(uname -r)"
    if [[ "$OS" == "Darwin" ]]; then
        cpus="$(sysctl -n hw.ncpu)"
        mem="$(( $(sysctl -n hw.memsize) / 1073741824 )) GB"
    else
        cpus="$(nproc)"
        mem="$(awk '/MemTotal/{printf "%.1f GB", $2/1048576}' /proc/meminfo)"
    fi
    echo "$OS $kernel | ${cpus} CPUs | $mem RAM"
}

# Peak RSS in KB. Argument: the full command to run.
measure_peak_rss() {
    if [[ "$OS" == "Darwin" ]]; then
        # macOS: /usr/bin/time -l reports bytes on the first numeric field
        # of the "maximum resident set size" line.
        local out
        out=$(/usr/bin/time -l "$@" 2>&1 >/dev/null) || true
        echo "$out" | grep -i "maximum resident" | awk '{for(i=1;i<=NF;i++) if($i+0==$i){print int($i/1024); exit}}'
    else
        # Linux: /usr/bin/time -v reports KB directly.
        local out
        out=$(/usr/bin/time -v "$@" 2>&1 >/dev/null) || true
        echo "$out" | grep "Maximum resident" | awk '{print $NF}'
    fi
}

# Portable "set mtime to N minutes ago".
set_mtime_mins_ago() {
    local file="$1" mins="$2"
    if [[ "$OS" == "Darwin" ]]; then
        touch -t "$(date -v-${mins}M +%Y%m%d%H%M.%S)" "$file"
    else
        touch -d "now - ${mins} minutes" "$file"
    fi
}

# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------

build() {
    echo "Building release binary ..."
    (cd "$PROJECT_DIR" && cargo build --release 2>&1 | tail -1)

    echo "Building bench-timer ..."
    rustc -O "$SCRIPT_DIR/timer.rs" -o "$TIMER"

    echo "  Binary:  $(ls -lh "$BINARY" | awk '{print $5}')"
    echo "  Timer:   $TIMER"
    echo ""
}

# ---------------------------------------------------------------------------
# Data generation
# ---------------------------------------------------------------------------

generate_session_file() {
    local uuid="$1" cwd="$2" slug="$3" prompt="$4" outfile="$5" lines="${6:-3}"

    cat > "$outfile" <<-JSONL
{"type":"system","cwd":"$cwd","sessionId":"$uuid","message":{"content":"init"}}
{"type":"user","cwd":"$cwd","sessionId":"$uuid","message":{"content":"$prompt"}}
{"type":"assistant","slug":"$slug","message":{"content":"Working on it."}}
JSONL

    # Pad to requested line count
    local i
    for (( i=4; i<=lines; i++ )); do
        echo "{\"type\":\"assistant\",\"message\":{\"content\":\"Line $i of response with realistic content and explanations.\"}}" >> "$outfile"
    done
}

generate_data() {
    echo "Generating benchmark data in $BENCH_ROOT ..."

    rm -rf "$BENCH_ROOT"
    mkdir -p "$BENCH_ROOT"

    # --- Varying session counts ---
    for size in "${SIZES[@]}"; do
        local home="$BENCH_ROOT/home_${size}"
        local projects="$home/.claude/projects"

        for p in $(seq 0 4); do
            mkdir -p "$projects/project-$p"
        done

        for (( i=1; i<=size; i++ )); do
            local proj_num=$(( i % 5 ))
            local proj_dir="$projects/project-$proj_num"
            local uuid
            uuid=$(printf '%08x-%04x-%04x-%04x-%012x' \
                $((i * 7 + 12345)) $((i * 3 + 100)) $((i * 5 + 200)) \
                $((i * 11 + 300))  $((i * 13 + 40000)))

            generate_session_file \
                "$uuid" \
                "/home/user/dev/project-$proj_num" \
                "session-slug-$i" \
                "Implement feature number $i with comprehensive error handling and unit tests" \
                "$proj_dir/${uuid}.jsonl"

            set_mtime_mins_ago "$proj_dir/${uuid}.jsonl" "$i"
        done
    done

    # --- Large files (varying line counts) ---
    local large_home="$BENCH_ROOT/home_large"
    local large_proj="$large_home/.claude/projects/big-project"
    mkdir -p "$large_proj"

    for lines in "${LARGE_SIZES[@]}"; do
        local uuid
        uuid=$(printf '%08x-%04x-%04x-%04x-%012x' \
            "$lines" 1234 5678 9012 345600000000)

        generate_session_file \
            "$uuid" "/home/user/dev/big" "big-slug" \
            "Big session with $lines lines" \
            "$large_proj/${uuid}.jsonl" "$lines"
    done

    # --- Summary ---
    echo ""
    for size in "${SIZES[@]}"; do
        local count
        count=$(find "$BENCH_ROOT/home_${size}/.claude/projects" -name "*.jsonl" | wc -l | tr -d ' ')
        local total
        total=$(du -sh "$BENCH_ROOT/home_${size}/.claude" 2>/dev/null | cut -f1)
        echo "  home_${size}: ${count} files, ${total}"
    done

    echo "  home_large: $(find "$large_proj" -name "*.jsonl" | wc -l | tr -d ' ') files"
    for f in "$large_proj"/*.jsonl; do
        local lc
        lc=$(wc -l < "$f" | tr -d ' ')
        local sz
        sz=$(du -h "$f" | cut -f1)
        echo "    $lc lines: $sz"
    done
    echo ""
}

# ---------------------------------------------------------------------------
# Benchmark runner
# ---------------------------------------------------------------------------

# Run the timer and parse output into the 5 stat variables.
# Usage: run_timer <home_dir> <iters> <binary_args...>
# Sets: _min _avg _p50 _p95 _max
run_timer() {
    local home="$1" iters="$2"
    shift 2
    local result
    result=$(HOME="$home" "$TIMER" "$iters" "$BINARY" "$@")
    read -r _min _avg _p50 _p95 _max <<< "$result"
}

# Like run_timer but for arbitrary commands (no HOME override).
# Usage: run_timer_raw <iters> <command> [args...]
run_timer_raw() {
    local result
    result=$("$TIMER" "$@")
    read -r _min _avg _p50 _p95 _max <<< "$result"
}

fmt_us() {
    # Format microseconds with comma separators for readability.
    printf "%'d" "$1" 2>/dev/null || printf "%d" "$1"
}

# ---------------------------------------------------------------------------
# Benchmark: end-to-end latency
# ---------------------------------------------------------------------------

bench_latency() {
    echo "================================================================"
    echo "1. END-TO-END LATENCY (default format, --limit 5)"
    echo "   ($ITERS iterations per data point, warm cache)"
    echo "================================================================"
    echo ""
    printf "  %-10s  %8s  %8s  %8s  %8s  %8s\n" \
        "Sessions" "Min" "Avg" "P50" "P95" "Max"
    printf "  %-10s  %8s  %8s  %8s  %8s  %8s\n" \
        "--------" "------" "------" "------" "------" "------"

    for size in "${SIZES[@]}"; do
        run_timer "$BENCH_ROOT/home_${size}" "$ITERS" --limit 5
        printf "  %-10s  %6sus  %6sus  %6sus  %6sus  %6sus\n" \
            "$size" \
            "$(fmt_us "$_min")" "$(fmt_us "$_avg")" "$(fmt_us "$_p50")" \
            "$(fmt_us "$_p95")" "$(fmt_us "$_max")"
    done
    echo ""
}

# ---------------------------------------------------------------------------
# Benchmark: format comparison
# ---------------------------------------------------------------------------

bench_formats() {
    local home="$BENCH_ROOT/home_1000"

    echo "================================================================"
    echo "2. FORMAT COMPARISON (1000 sessions, --limit 5)"
    echo "================================================================"
    echo ""
    printf "  %-22s  %8s  %8s  %8s\n" "Format" "Min" "Avg" "P50"
    printf "  %-22s  %8s  %8s  %8s\n" "---------------------" "------" "------" "------"

    local labels=("default"         "short"                    "json"            "limit=1"  "limit=10" "limit=50" "limit=0 (no I/O)")
    local argsets=("--limit 5"      "--limit 5 --format short" "--limit 5 --json" "--limit 1" "--limit 10" "--limit 50" "--limit 0")

    for i in "${!labels[@]}"; do
        # shellcheck disable=SC2086
        run_timer "$home" "$ITERS" ${argsets[$i]}
        printf "  %-22s  %6sus  %6sus  %6sus\n" \
            "${labels[$i]}" \
            "$(fmt_us "$_min")" "$(fmt_us "$_avg")" "$(fmt_us "$_p50")"
    done
    echo ""
}

# ---------------------------------------------------------------------------
# Benchmark: startup overhead
# ---------------------------------------------------------------------------

bench_startup() {
    echo "================================================================"
    echo "3. STARTUP OVERHEAD BREAKDOWN"
    echo "================================================================"
    echo ""
    printf "  %-30s  %8s  %8s\n" "Command" "Min" "Avg"
    printf "  %-30s  %8s  %8s\n" "-----------------------------" "------" "------"

    run_timer_raw "$ITERS" /bin/true
    printf "  %-30s  %6sus  %6sus\n" "/bin/true (baseline)" \
        "$(fmt_us "$_min")" "$(fmt_us "$_avg")"

    run_timer_raw "$ITERS" "$BINARY" --version
    printf "  %-30s  %6sus  %6sus\n" "ccsesh --version" \
        "$(fmt_us "$_min")" "$(fmt_us "$_avg")"

    run_timer_raw "$ITERS" "$BINARY" --help
    printf "  %-30s  %6sus  %6sus\n" "ccsesh --help" \
        "$(fmt_us "$_min")" "$(fmt_us "$_avg")"

    run_timer "$BENCH_ROOT/home_1000" "$ITERS" --limit 0
    printf "  %-30s  %6sus  %6sus\n" "ccsesh --limit 0 (no I/O)" \
        "$(fmt_us "$_min")" "$(fmt_us "$_avg")"

    run_timer "$BENCH_ROOT/home_5" "$ITERS" --limit 5
    printf "  %-30s  %6sus  %6sus\n" "ccsesh (5 sessions)" \
        "$(fmt_us "$_min")" "$(fmt_us "$_avg")"

    run_timer "$BENCH_ROOT/home_1000" "$ITERS" --limit 5
    printf "  %-30s  %6sus  %6sus\n" "ccsesh (1000 sessions)" \
        "$(fmt_us "$_min")" "$(fmt_us "$_avg")"

    echo ""
}

# ---------------------------------------------------------------------------
# Benchmark: large files
# ---------------------------------------------------------------------------

bench_large_files() {
    local home="$BENCH_ROOT/home_large"

    echo "================================================================"
    echo "4. LARGE SESSION FILES (4 files, varying line counts)"
    echo "   ccsesh reads max 50 lines per file — size shouldn't matter"
    echo "================================================================"
    echo ""

    run_timer "$home" "$ITERS" --limit 4
    printf "  Mixed (50–50k lines):  min=%sus  avg=%sus  p50=%sus\n" \
        "$(fmt_us "$_min")" "$(fmt_us "$_avg")" "$(fmt_us "$_p50")"
    echo ""
}

# ---------------------------------------------------------------------------
# Benchmark: memory
# ---------------------------------------------------------------------------

bench_memory() {
    echo "================================================================"
    echo "5. MEMORY USAGE (Peak RSS)"
    echo "================================================================"
    echo ""
    printf "  %-35s  %10s\n" "Scenario" "Peak RSS"
    printf "  %-35s  %10s\n" "-----------------------------------" "--------"

    for size in "${SIZES[0]}" "${SIZES[-1]}"; do
        [[ -d "$BENCH_ROOT/home_${size}" ]] || continue
        local rss
        rss=$(measure_peak_rss env HOME="$BENCH_ROOT/home_${size}" "$BINARY" --limit 5)
        printf "  %-35s  %6s KB\n" "${size} sessions, limit=5" "$rss"
    done

    local rss max_size="${SIZES[-1]}"
    rss=$(measure_peak_rss env HOME="$BENCH_ROOT/home_${max_size}" "$BINARY" --limit "$max_size" --json)
    printf "  %-35s  %6s KB\n" "${max_size} sessions, limit=${max_size} (json)" "$rss"

    rss=$(measure_peak_rss env HOME="$BENCH_ROOT/home_large" "$BINARY" --limit 4)
    printf "  %-35s  %6s KB\n" "large files (50-50k lines), limit=4" "$rss"

    echo ""
}

# ---------------------------------------------------------------------------
# Benchmark: syscall profile (Linux only)
# ---------------------------------------------------------------------------

bench_syscalls() {
    if [[ "$OS" != "Linux" ]] || ! command -v strace &>/dev/null; then
        echo "================================================================"
        echo "6. SYSCALL PROFILE — skipped (Linux + strace required)"
        echo "================================================================"
        echo ""
        return
    fi

    echo "================================================================"
    echo "6. SYSCALL PROFILE (strace)"
    echo "================================================================"
    echo ""

    # Use first, middle, and last from SIZES array
    local strace_sizes=("${SIZES[0]}" "${SIZES[$(( ${#SIZES[@]} / 2 ))]}" "${SIZES[-1]}")
    for size in "${strace_sizes[@]}"; do
        [[ -d "$BENCH_ROOT/home_${size}" ]] || continue
        echo "  --- $size sessions ---"
        strace -c env HOME="$BENCH_ROOT/home_${size}" "$BINARY" --limit 5 \
            2>&1 >/dev/null | sed 's/^/  /'
        echo ""
    done
}

# ---------------------------------------------------------------------------
# Benchmark: tool comparison
# ---------------------------------------------------------------------------

bench_comparison() {
    echo "================================================================"
    echo "7. COMPARISON TO OTHER TOOLS"
    echo "================================================================"
    echo ""
    printf "  %-30s  %8s  %8s\n" "Tool" "Min" "Avg"
    printf "  %-30s  %8s  %8s\n" "-----------------------------" "------" "------"

    run_timer_raw "$ITERS" /bin/true
    printf "  %-30s  %6sus  %6sus\n" "/bin/true (baseline)" \
        "$(fmt_us "$_min")" "$(fmt_us "$_avg")"

    if [[ -x /bin/echo ]]; then
        run_timer_raw "$ITERS" /bin/echo hello
        printf "  %-30s  %6sus  %6sus\n" "/bin/echo" \
            "$(fmt_us "$_min")" "$(fmt_us "$_avg")"
    fi

    run_timer_raw "$ITERS" /bin/ls /tmp
    printf "  %-30s  %6sus  %6sus\n" "ls /tmp" \
        "$(fmt_us "$_min")" "$(fmt_us "$_avg")"

    # Use first and last available sizes
    for size in "${SIZES[0]}" "${SIZES[-1]}"; do
        [[ -d "$BENCH_ROOT/home_${size}" ]] || continue
        run_timer "$BENCH_ROOT/home_${size}" "$ITERS" --limit 5
        printf "  %-30s  %6sus  %6sus\n" "ccsesh ($size sessions)" \
            "$(fmt_us "$_min")" "$(fmt_us "$_avg")"
    done

    echo ""
}

# ---------------------------------------------------------------------------
# Report header / footer
# ---------------------------------------------------------------------------

print_header() {
    echo "================================================================"
    echo "     ccsesh v0.1.0 — Performance Benchmark Report"
    echo "================================================================"
    echo ""
    echo "  System:     $(sysinfo)"
    echo "  Binary:     $(ls -lh "$BINARY" | awk '{print $5}') (release)"
    echo "  Iterations: $ITERS per data point"
    echo "  Data dir:   $BENCH_ROOT"
    echo "  Date:       $(date '+%Y-%m-%d %H:%M:%S %Z')"
    echo ""
}

print_footer() {
    echo "================================================================"
    echo "SUMMARY"
    echo "================================================================"
    echo ""
    echo "  To run criterion micro-benchmarks (discover, parse, display):"
    echo "    cargo bench"
    echo ""
    echo "  To clean up benchmark data:"
    echo "    $0 --clean"
    echo ""
    echo "================================================================"
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

main() {
    build
    generate_data

    print_header
    bench_latency
    bench_formats
    bench_startup
    bench_large_files
    bench_memory
    bench_syscalls
    bench_comparison
    print_footer
}

main "$@"
