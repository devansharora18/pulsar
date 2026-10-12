# TODO / RUN ON HOST

Known gaps and the exact commands to close them elsewhere. None of these block building or
running Pulsar.

## 1. Real (measured) power instead of the model

RAPL `energy_uj` is root-only on this kernel, so power currently runs the `model`
(utilisation × TDP) path. **RUN ON HOST:**

```sh
sudo cp packaging/99-pulsar-rapl.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger --subsystem-match=powercap
```

or press `p` in the Memory view (does the same via `pkexec`). The power badge then flips from
`model` to `est.`.

## 2. Sampler CPU to < 1.5% of one core

Currently ~3.2% at ~570 processes / 1 Hz (see `PERF.md`). Remaining levers:

- Cache the resolved group key per pid (with its `starttime`) so `assign_groups` does not
  re-run `.to_ascii_lowercase()` and hashing for every process every tick.
- Read `/proc/<pid>/stat` through a reused buffer and parse fields without the intermediate
  `Vec<&str>` to cut per-tick allocations.
- Widen the change threshold / max age for `smaps_rollup` refreshes (trades memory-column
  lag for CPU).

## 3. Packaging a TUI

- The `.desktop` file launches Pulsar in a terminal (`Terminal=true`). A Flatpak/AppImage of
  a TUI is awkward (needs a shell in the sandbox); the shipped `PKGBUILD` and
  `cargo install --path app` are the primary paths.
- `packaging/io.github.pulsar.Pulsar.yml` is provided but a terminal-launching Flatpak needs
  `--socket=session-bus` and a wrapper script; not validated here.

## 4. ARM packaging

The `PKGBUILD` lists `aarch64` but no ARM build was performed here.

## 5. Smaller polish gaps

- Per-group **energy since launch (Wh)** in the group tooltip: system Wh is shown in the
  Power view; per-group Wh needs a per-group energy accumulator in the sampler.
- Process-table **virtualization** for very large flat lists (> 400 rows): the grouped view
  is bounded and fine; flat view is capped at 400 rows.
- Reclaim actions show current numbers; an explicit before/after pair could be captured
  around the privileged call.
