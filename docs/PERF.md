# Performance

Measured on the build host: Fedora 44, AMD Ryzen AI 7 350 (8 cores / **16 logical CPUs**),
32 GB RAM, **~570 concurrent processes**, release profile (`lto = "fat"`,
`codegen-units = 1`, `strip = true`, `panic = "abort"`), default 1 Hz, terminal size
180×48, run under a pseudo-terminal.

## Budget

| Metric | Target | Measured | Notes |
|---|---|---|---|
| Idle CPU | < 1.5% of one core | **~3.2%** | whole process (sampler + render), 570 procs @ 1 Hz |
| RSS | < 120 MB | **~7 MB** | no webview — this is a TUI |
| Release binary size | — | **1,720,584 bytes (~1.6 MiB)** | stripped + fat LTO |

The migration from the Dioxus/WebKitGTK desktop build to a ratatui TUI cut the binary from
5.1 MB to 1.7 MB and memory from ~450 MB to ~7 MB. CPU is dominated by reading `/proc` for
570 processes every second; the terminal render is negligible.

## How the sampler stays cheap

Every tick reads what must be fresh and defers what need not be:

- `/proc/<pid>/stat` for **every** process every tick — keeps row CPU % and RSS live.
- `/proc/<pid>/smaps_rollup` (PSS) is the hot spot: it scales with the process's number of
  VMAs. It is re-read **only when a process's RSS moved more than 512 kB** since the last
  read, or every 40 ticks as a floor. Stable processes cost nothing; active ones stay live.
- `/proc/<pid>/io` is read alongside the rollup, not every tick.
- Static per-pid data (`exe`, `cmdline`, cgroup app id, uid) is read **once per new
  pid/starttime** and cached.
- Desktop-entry matching uses prebuilt `HashMap`s (O(1)); a linear scan over ~200
  `.desktop` files per process per tick was a measurable cost.
- Kernel threads (no `exe`/`cmdline`) never get a `smaps_rollup` read.

## Measurement method

CPU is the delta of the process's `utime+stime` (`/proc/<pid>/stat`) over a wall-clock
window; RSS from `/proc/<pid>/status`. The TUI runs under a pseudo-terminal (crossterm
needs a tty). Reproduce with `docs/`-adjacent tooling: run the release binary under a pty
and sample `/proc` for 10 s.

## Remaining levers to reach 1.5%

Recorded in [`TODO.md`](TODO.md): cache the resolved group key per pid (skip re-hashing in
`assign_groups` on every tick), and read `/proc/<pid>/stat` through a reused buffer/parse
without the intermediate field vector.
