# Pulsar

A fast, beautiful Linux system monitor that fixes what GNOME System Monitor and Resources
get wrong: memory you can actually reconcile, processes grouped like Windows Task Manager,
and a per-app power estimate.

> **Status:** developed and built on Fedora 44 (GNOME/Wayland, 16 CPUs, 32 GB RAM). See
> [`docs/PERF.md`](docs/PERF.md) for measured numbers and [`docs/TODO.md`](docs/TODO.md)
> for the few things that need a different host (screenshots, RAPL permission, `dx`).

![Pulsar overview](docs/screenshots/01-overview-catppuccin.png)

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
unreadable), so group sums reconcile with the header. A **kernel + unattributed** row makes
the leftover explicit, so the numbers never contradict. A plain-language line reads
*"… really used · … cache you can reclaim · … available"* and memory pressure (PSI) is a
small gauge.

**2. Grouped processes, like Windows Task Manager.** Multi-process apps fold into one
expandable row: Zen/Firefox "Web Content" / "Privileged Cont" / "WebExtensions" children
under the browser; two `opencode` processes become one `opencode (2)` row. Grouping uses,
in order, the systemd user cgroup `app-*.scope`/`app-*.service`, a match against installed
`.desktop` files (name + icon), the executable basename, then the process-tree root.

**3. Power per app, honestly labelled.** No Linux kernel exposes a per-process power meter,
so Pulsar estimates and *says so*. CPU watts come from RAPL package energy where readable
(`est.`) and from a utilisation × TDP model otherwise (`model`); GPU watts come from
amdgpu `power1_average` / NVML / `nvidia-smi`; battery from `power_now` (or
`current_now × voltage_now`). CPU watts are attributed by each process's share of CPU
ticks of the package's *dynamic* (non-idle) draw.

## Features

- **Overview**: 16-core heat grid with frequency + temperature, load average, big ring
  gauges (CPU / memory / swap / power), per-interface network and per-device disk
  sparklines, filesystem usage bars.
- **Processes**: grouped / flat / tree views, sort any column, search, pin groups, hide
  kernel threads, live sparklines, expand to child processes. Kill group (SIGTERM →
  SIGKILL), terminate, suspend/resume, renice, open exe location, copy PID.
- **Memory**: the stacked bar above, a **treemap by app** (click a tile to jump to that
  group), and a **Reclaim memory** panel: one-click `drop_caches`, `compact_memory` and
  `flush swap`, each through `pkexec` with an explicit confirmation and a before/after
  activity log.
- **Power**: system watts over time, per-source breakdown (package / dynamic / baseline /
  GPU / battery), top consumers strip, cumulative Wh since launch.
- **Theming**: TOML themes in `~/.config/pulsar/themes/`, six built-ins (Catppuccin Mocha,
  Gruvbox Dark, Rosé Pine, Tokyo Night, Nord, Everforest) plus **pywal** live-reload from
  `~/.cache/wal/colors.json`, GNOME accent colour detection, hot-reloaded config, frameless
  transparent window with a custom titlebar.
- **Keyboard-first**: `1–4` views, `/` search, `j/k` navigate, `Enter` expand, `x` kill,
  `g/f/t` group/flat/tree, `r` reload, `s` settings, `a` activity, `?` cheat sheet,
  `Esc` close.

## Requirements

- Linux with `/proc` and cgroup v2 (any modern distro).
- Runtime libs: `webkit2gtk-4.1`, `gtk3`, `libsoup3`.
- Rust ≥ 1.75 to build.

## Install / run

```sh
# from source
cargo build --release
./target/release/pulsar

# install the binary
cargo install --path app
```

Packaging lives in [`packaging/`](packaging/): a `.desktop` file, an SVG icon, a udev rule
for RAPL, a `PKGBUILD`, a Flatpak manifest, and a linuxdeploy AppImage script.

To get **measured** (instead of modelled) CPU power, install the RAPL udev rule — or click
**Enable power metrics** in Pulsar, which applies it via `pkexec`:

```sh
sudo cp packaging/99-pulsar-rapl.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger --subsystem-match=powercap
```

## Keybinds

| Key | Action |
|---|---|
| `1` `2` `3` `4` | Overview / Processes / Memory / Power |
| `/` | focus search |
| `j` `k` | move selection down / up |
| `Enter` | expand / collapse selected group |
| `x` | kill selected group |
| `g` `f` `t` | group / flat / tree view |
| `r` | reload config + theme from disk |
| `s` | settings |
| `a` | activity log |
| `?` | cheat sheet |
| `Esc` | close overlays |

## Theme authoring

Drop a file named `<name>.toml` in `~/.config/pulsar/themes/` and select it in Settings.
Every field is optional and falls back to Catppuccin Mocha:

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

`accent` drives highlights and glow; `green/yellow/orange/red` drive the heat grid and the
memory legend. Themes and `config.toml` hot-reload on save. Choose `pywal` as the theme and
Pulsar re-colours whenever `~/.cache/wal/colors.json` changes.

## Honest notes about power estimation

- There is **no** per-process power meter in Linux. Pulsar's per-app watts are an estimate
  labelled `est.` (RAPL-based) or `model` (utilisation × TDP).
- RAPL energy counters are root-only since Linux 5.10. Without the udev rule Pulsar runs the
  model path.
- GPU watts are **system-level** unless a driver exposes per-process utilisation (NVIDIA
  via NVML, behind the `nvidia` cargo feature); they are not split across apps.
- Network is never attributed per process — Linux does not expose per-pid network counters,
  so Pulsar does not fake it.

## License

MIT — see [`LICENSE`](LICENSE). Bundled JetBrains Mono is under the SIL Open Font License
(`app/assets/fonts/JetBrainsMono-OFL.txt`); the bundled Symbols Nerd Font is under its own
upstream licenses.
