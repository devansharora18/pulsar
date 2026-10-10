# Performance

Measured on the build host: Fedora 44, AMD Ryzen AI 7 350 (8 cores / **16 logical CPUs**),
32 GB RAM, GNOME on Wayland, **510 concurrent processes**, release profile
(`lto = "fat"`, `codegen-units = 1`, `strip = true`, `panic = "abort"`), default 1 Hz.

## Budget

| Metric | Target | Measured | Notes |
|---|---|---|---|
| Sampler CPU | < 1.5% of one core | **~3.5%** | the sampler thread only, 510 procs @ 1 Hz |
| Whole-app CPU | < 1.5% of one core | 4–16% | dominated by the WebKitGTK webview, not the sampler |
| RSS (Pulsar process) | < 120 MB | **~193 MB** | WebKitGTK baseline (~150 MB) + process model |
| RSS (whole app incl. webkit children) | — | ~450 MB | three processes: pulsar + WebKitWebProcess + WebKitNetworkProcess |
| Release binary size | — | **5,131,096 bytes (~4.9 MiB)** | stripped + fat LTO |

The sampler and the webview are the two costs. The webview baseline is the floor for any
Dioxus-desktop (wry/WebKitGTK) app; the sampler is the part Pulsar controls and it is what
the funding target really refers to.

## How the sampler stays cheap

Every tick it reads what must be fresh and defers what need not be:

- `/proc/<pid>/stat` for **every** process every tick — this keeps row CPU % and RSS live.
- `/proc/<pid>/smaps_rollup` (PSS) and `/proc/<pid>/io` for a **rotating slice** of
  processes (`detail_every` ticks), so the expensive reads are spread out. Without this the
  sampler used ~15% of a core; with it, ~3.5%.
- Static per-pid data (`exe`, `cmdline`, cgroup app id, uid) is read **once per new
  pid/starttime** and cached, never every tick.
- Desktop-entry matching uses prebuilt `HashMap`s (O(1)) instead of linear scans — a linear
  scan over 200 `.desktop` files per process per tick was a measurable cost.
- Kernel threads (no `exe`/`cmdline`) never get a `smaps_rollup` read.

## Measurement method

CPU is taken as the delta of thread `utime+stime` (from
`/proc/<pid>/task/<tid>/stat`) over a wall-clock window; RSS from `/proc/<pid>/status`.
The sampler thread is identified by its name `pulsar-sampler`.

## Known levers if you want the sampler lower

Recorded in [`TODO.md`](TODO.md) — chiefly: cache the resolved group key per pid across
ticks (only recompute when `app_id`/`exe` changes), and read `/proc/<pid>/stat` through a
reused buffer to cut per-tick allocation.
