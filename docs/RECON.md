# Pulsar — Recon of the target machine

Recorded on the build host before writing any collector code. Everything listed here was
verified by direct reads, not assumed.

## Hardware / OS

- OS: Fedora Linux 44 (Workstation), kernel `7.2.7-200.fc44.x86_64`.
- CPU: AMD Ryzen AI 7 350 w/ Radeon 860M — 8 cores / **16 logical CPUs**.
- RAM: 32 GiB (`MemTotal` 32113600 kB at capture), swap ~72 GiB (`SwapTotal` 75497464 kB).
- Session: GNOME on **Wayland** (`XDG_SESSION_TYPE=wayland`, `wayland-0`), `DISPLAY=:0`.
- User: uid 1000, member of `wheel`. `pkexec` present at `/usr/bin/pkexec`.
- Toolchain: `cargo`/`rustc` 1.96.0. `dx` (dioxus-cli) **not installed**; `cargo build`
  is used for the desktop binary (dx is only needed for bundling/web targets).
- GUI toolkit: `webkit2gtk-4.1` 2.54.1, `gtk+-3.0`, `libsoup-3.0` devel present → a
  Dioxus desktop (wry/tao) build is viable on this box.

## Memory sources

| Source | Path | Readable as user 1000? |
|---|---|---|
| meminfo | `/proc/meminfo` | yes |
| PSI memory | `/proc/pressure/memory` | yes (`some`/`full` avg10/60/300) |
| per-pid rollup | `/proc/<pid>/smaps_rollup` | yes (own + world-readable); `Pss`, `Shared_*`, `Private_*`, `Swap` |
| per-pid maps | `/proc/<pid>/smaps` | yes but expensive → only used as documented fallback |
| per-pid cgroup | `/proc/<pid>/cgroup` | yes, unified v2 (`0::/user.slice/...`) |
| per-pid io | `/proc/<pid>/io` | yes; `read_bytes`/`write_bytes` are real disk IO |

`/proc/meminfo` fields present on this kernel: `MemTotal`, `MemFree`, `MemAvailable`,
`Buffers`, `Cached`, `SwapCached`, `Shmem`, `SReclaimable`, `SUnreclaim`, `Slab`,
`KReclaimable`, `KernelStack`, `PageTables`, `VmallocUsed`, `Zswap`, `Zswapped`,
`Percpu`, `SecPageTables`. No `Zram`/`SwapFree` feeds zram (swap is a file/partition).

cgroup notes observed live:
- Zen Browser (flatpak) → `.../app.slice/app-graphical.slice/app-flatpak-app.zen_browser.zen-*.scope`
- GNOME-launched apps → `.../app.slice/app-*.scope` / `app-*.service`
- opencode started from a terminal → `.../app.slice/ptyxis-spawn-*.scope` (NOT an `app-*`
  scope) → grouping must fall back to exe basename so the two `opencode` PIDs fold into one.

## Power sources

| Source | Path | Status |
|---|---|---|
| RAPL package | `/sys/class/powercap/intel-rapl:0/energy_uj` | **root-only** (`-r-------- root root`), verified `Permission denied` |
| RAPL max range | `/sys/class/powercap/intel-rapl:0/max_energy_range_uj` | readable (65532610987) |
| RAPL core (subzone) | `/sys/class/powercap/intel-rapl:0:0/energy_uj` | root-only |
| Battery `power_now` | `/sys/class/power_supply/BAT1/power_now` | readable (0 W on AC) |
| Battery voltage | `/sys/class/power_supply/BAT1/voltage_now` | readable (16.574 V) |
| Battery `current_now` | `/sys/class/power_supply/BAT1/current_now` | **absent** → fall back to `power_now`, else `current_now*voltage_now` |
| amdgpu GPU power | `/sys/class/drm/card1/device/hwmon/hwmon7/power1_average` | readable (4160000 µW = 4.16 W) |
| amdgpu GPU busy | `/sys/class/drm/card1/device/gpu_busy_percent` | readable (3%) |
| NVML / `nvidia-smi` | — | **not present** (AMD iGPU); feature-gated, off |

Consequence: RAPL is the accurate CPU source but is denied to the user. Pulsar ships a
udev rule + tmpfiles snippet and a `pkexec` one-click enabler. Until applied (or if the
box lacks RAPL) Pulsar falls back to a utilization × TDP **model** and labels the value
`model` instead of `est.`.

## Temps

`/sys/class/hwmon`: `k10temp` (CPU, `temp1_input` readable = 41.4 °C), `amdgpu`,
`acpitz`, `nvme`, `BAT1`, `mt7925_phy0` wifi. All world-readable.

## Desktop / icon sources

- `/usr/share/applications` (180 `.desktop`), `~/.local/share/applications`,
  `/var/lib/flatpak/exports/share/applications`.
- Icon themes under `/usr/share/icons`: current GTK icon theme = **`kora`**, plus
  `hicolor`. `.desktop` `Icon=` may be a themed name or an absolute path; flatpak icons
  live under `/var/lib/flatpak/exports/share/icons`.
- `StartupWMClass` present (e.g. Zen `zen`, OpenCode `ai.opencode.desktop`).
- pywal cache `~/.cache/wal/colors.json` **absent** → theme engine must tolerate missing
  and hot-reload when it appears.
- GNOME accent colour readable: `gsettings get org.gnome.desktop.interface accent-color`
  → `'blue'`.

## Fallbacks chosen

- PSS from `smaps_rollup` primary; RSS from `/proc/<pid>/statm` fallback for kernels/pids
  where rollup is unreadable.
- Power: RAPL (if permitted) → battery/GPU direct → utilization×TDP model. Always label.
- Icon: themed lookup → hicolor → generated monogram tile.
- Charts and containers are hand-built; no chart library.
