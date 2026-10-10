# Screenshots

Capture could not be performed unattended on the build host (GNOME/Wayland denies screen
capture to normal apps — see [`../TODO.md`](../TODO.md) §1 for the exact `RUN ON HOST`
commands).

The intended set, produced by `pulsar --screenshot`, is:

| File | View | Theme |
|---|---|---|
| `01-overview-catppuccin.png` | Overview | Catppuccin Mocha |
| `01-processes-catppuccin.png` | Processes (grouped, expanded) | Catppuccin Mocha |
| `01-memory-catppuccin.png` | Memory (stacked bar + treemap) | Catppuccin Mocha |
| `01-power-catppuccin.png` | Power | Catppuccin Mocha |
| `02-overview-tokyo-night.png` | Overview | Tokyo Night |
| `02-processes-tokyo-night.png` | Processes (grouped, expanded) | Tokyo Night |
| `02-memory-tokyo-night.png` | Memory (stacked bar + treemap) | Tokyo Night |
| `02-power-tokyo-night.png` | Power | Tokyo Night |

Run:

```sh
PULSAR_CAPTURE_CMD='gnome-screenshot -f {}' \
  ./target/release/pulsar --screenshot docs/screenshots/01-overview-catppuccin.png \
    --theme catppuccin-mocha --view overview --delay 6
```
