# Pulsar

A fast, terminal-native Linux system monitor that fixes what GNOME System Monitor and
Resources get wrong: memory you can actually reconcile, processes grouped like Windows Task
Manager, and a per-app power estimate.

> **Status:** developed and built on Fedora 44 (GNOME/Wayland, 16 CPUs, 32 GB RAM). A
> ratatui TUI (no webview): ~7 MB RSS, ~1.7 MB binary, 570 processes at 1 Hz. See
> [`docs/PERF.md`](docs/PERF.md) for numbers and [`docs/TODO.md`](docs/TODO.md) for the
> remaining gaps.

![Pulsar overview](docs/screenshots/01-overview-catppuccin.png)

Built from two crates: `pulsar-core` (pure `/proc` + `/sys` collection, models and tests —
no UI) and `pulsar` (the ratatui TUI). The sampler runs on a background thread and publishes
immutable snapshots over a channel; the UI never blocks on `/proc`.

## What it fixes

**1. Memory that adds up.** GNOME's header says "10.3 GB used" while its process list sums
to ~3.4 GB, because per-process numbers are RSS-shaped and ignore shared pages and kernel
memory. Pulsar decomposes memory explicitly:

```
Total = MemTotal
Available = MemAvailable                       (what apps can take without swapping)
Used (real) = MemTotal − MemAvailable
Cache (reclaimable) = Cached + Buffers + SReclaimable − Shmem
Shared = Shmem
Kernel = SUnreclaim + KernelStack + PageTables + VmallocUsed
Free = MemFree
Swap used = SwapTotal − SwapFree   (+ ZSwap/zram when present)
```

Per process, Pulsar shows **PSS** from `/proc/<pid>/smaps_rollup` (falling back to RSS when
unreadable), so group sums reconcile with the header. A **kernel + unattributed** panel makes
the leftover explicit (`Σ PSS` is shown alongside), so the numbers never contradict. A
plain-language line reads *"… really used · … cache you can reclaim · … available"* and
memory pressure (PSI) is a small gauge.

**2. Grouped processes, like Windows Task Manager.** Multi-process apps fold into one
expandable row: Zen/Firefox "Web Content" / "Privileged Cont" / "WebExtensions" children
under the browser; two `opencode` processes become one `opencode (2)` row. Grouping uses,
in order, the systemd user cgroup `app-*.scope`/`app-*.service`, a match against installed
`.desktop` files, the executable basename, then the process-tree root.

**3. Power per app, honestly labelled.** No Linux kernel exposes a per-process power meter,
so Pulsar estimates and *says so*. CPU watts come from RAPL package energy where readable
(`est.`) and from a utilisation × TDP model otherwise (`model`); GPU watts come from amdgpu
`power1_average` / NVML / `nvidia-smi`; battery from `power_now` (or
`current_now × voltage_now`). CPU watts are attributed by each process's share of CPU
ticks of the package's *dynamic* (non-idle) draw.

## Views

- **Overview** (`1`): resource gauges (CPU / memory / swap / power), a per-core heat grid
  with frequency, load average and temperatures, top memory consumers, network and
  filesystem panels.
- **Processes** (`2`): grouped / flat / tree table with sortable columns (CPU, PSS, disk IO,
  power, PID), search, pinning, kernel-thread toggle, expandable children, and an action bar
  (kill / terminate / suspend / resume / renice / open location).
- **Memory** (`3`): the stacked bar and plain-language line, the three reconciliation
  panels, an interactive **treemap by app**, and the Reclaim panel.
- **Power** (`4`): system watts over time, per-source breakdown (package / dynamic /
  baseline / GPU / battery), top consumers, cumulative Wh.

## Controls

| Key | Action |
|---|---|
| `1` `2` `3` `4` / `Tab` | Overview / Processes / Memory / Power |
| `/` | search |
| `j` `k` / arrows | move selection |
| `Enter` | expand / collapse group |
| `x` / `k` | kill selected group |
| `t` | terminate · `z`/`u` suspend/resume |
| `+` `-` | renice selected |
| `o` | open exe location · `p` pin |
| `g` `f` `t` | group / flat / tree view |
| `S` / `R` | cycle sort column / toggle direction |
| `h` | hide kernel threads |
| `c` `C` | drop clean caches / drop all (pkexec) |
| `m` | compact memory · `w` flush swap |
| `p` | enable power metrics (RAPL udev, pkexec) |
| `r` | reload config + theme |
| `s` | settings · `a` activity log · `?` help |
| `q` | quit |

Every privileged action is confirmed first and logged in the activity drawer.

## Requirements

- Linux with `/proc` and cgroup v2 (any modern distro).
- Rust ≥ 1.75 to build. No GUI toolkit needed.

## Install / run

```sh
cargo build --release
./target/release/pulsar

# install the binary
cargo install --path app
```

Packaging lives in [`packaging/`](packaging/): a `.desktop` file (terminal launcher), an SVG
icon, a udev rule for RAPL, a `PKGBUILD`, and a Flatpak manifest.

To get **measured** (instead of modelled) CPU power, install the RAPL udev rule — or press
`p` in the Memory view, which applies it via `pkexec`:

```sh
sudo cp packaging/99-pulsar-rapl.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger --subsystem-match=powercap
```

## Screenshots

`--dump <dir>` renders every view headlessly (no compositor capture needed):

```sh
PULSAR_DUMP_THEMES="catppuccin-mocha,tokyo-night" \
  pulsar --dump docs/screenshots --png
```

It writes a `.txt` render and, with `--png`, a real coloured PNG of each view. The captures
in [`docs/screenshots/`](docs/screenshots/) were produced this way.

## Theme authoring

Drop a file named `<name>.toml` in `~/.config/pulsar/themes/` and select it in `s` →
Settings → `t`. Every field is optional and falls back to Catppuccin Mocha:

```toml
name = "My Rice"
bg = "#101014"
bg_alt = "#0b0b0f"
fg = "#e6e6e6"
fg_dim = "#7a7a85"
border = "#22222a"
accent = "#7aa2f7"
green = "#9ece6a"
yellow = "#e0af68"
orange = "#ff9e64"
red = "#f7768e"
blue = "#7aa2f7"
magenta = "#bb9af7"
cyan = "#7dcfff"
is_light = false
```

Built-ins: Catppuccin Mocha, Gruvbox Dark, Rosé Pine, Tokyo Night, Nord, Everforest, plus
**pywal** (reads `~/.cache/wal/colors.json` and re-colours when it changes). `config.toml`
hot-reloads on save. In Settings, `S` saves and `w` writes the current theme out to
`~/.config/pulsar/themes/`.

## Honest notes about power estimation

- There is **no** per-process power meter in Linux. Per-app watts are an estimate labelled
  `est.` (RAPL-based) or `model` (utilisation × TDP).
- RAPL energy counters are root-only since Linux 5.10. Without the udev rule Pulsar runs the
  model path.
- GPU watts are **system-level** unless a driver exposes per-process utilisation (NVIDIA via
  NVML, behind the `nvidia` cargo feature); they are not split across apps.
- Network is never attributed per process — Linux does not expose per-pid network counters,
  so Pulsar does not fake it.

## License

MIT — see [`LICENSE`](LICENSE). Bundled JetBrains Mono is under the SIL Open Font License
(`app/assets/fonts/JetBrainsMono-OFL.txt`); the TTFs are used to rasterize `--png` captures.
