# TODO / RUN ON HOST

Things that could not be completed **in this environment**, with the exact commands to
finish them elsewhere. Nothing here blocks building or running Pulsar.

## 1. Screenshots (blocked work — display capture is denied to apps)

`docs/screenshots/` is empty. Pulsar ships a `--screenshot` mode but this GNOME/Wayland
session denies screen capture to normal applications:

- `grim` fails: *"compositor doesn't support the screen capture protocol"* (GNOME does not
  implement `wlr-screencopy`).
- `org.gnome.Shell.Screenshot.Screenshot` returns `AccessDenied: Screenshot is not
  allowed` for non-Shell callers.
- The xdg-desktop-portal `Screenshot` method returns a request handle but requires an
  interactive consent dialog, so it cannot run unattended.
- Capturing the XWayland root with ImageMagick/ffmpeg yields a black frame (mutter does not
  compose clients into the X root).

**RUN ON HOST** (with a real user present):

```sh
# Option A — gnome-screenshot
sudo dnf install -y gnome-screenshot
cargo build --release
for v in overview processes memory power; do
  PULSAR_CAPTURE_CMD='gnome-screenshot -f {}' \
    ./target/release/pulsar --screenshot docs/screenshots/01-$v-catppuccin.png \
      --theme catppuccin-mocha --view $v --expand --delay 6
  PULSAR_CAPTURE_CMD='gnome-screenshot -f {}' \
    ./target/release/pulsar --screenshot docs/screenshots/02-$v-tokyo-night.png \
      --theme tokyo-night --view $v --expand --delay 6
done

# Option B — any wlroots compositor (sway/hyprland) where grim works
PULSAR_CAPTURE_CMD='grim {}' ./target/release/pulsar --screenshot out.png --delay 6
```

`--view overview|processes|memory|power`, `--theme <name>`, `--expand` (expand top groups)
and `--delay <s>` are all wired up.

## 2. Real (measured) power instead of the model

RAPL `energy_uj` is root-only on this kernel, so power is currently the `model`
(utilisation × TDP). **RUN ON HOST:**

```sh
sudo cp packaging/99-pulsar-rapl.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger --subsystem-match=powercap
```

or click **Enable power metrics** in Pulsar (does the same via `pkexec`). After that the
power badge flips from `model` to `est.`.

## 3. `dx` (dioxus-cli)

`dx` is not installed on this box; the desktop app is built with `cargo build --release`,
which is the supported path for the desktop platform. `dx` is only needed for
web/mobile/bundling. **RUN ON HOST:**

```sh
cargo install dioxus-cli --locked   # provides `dx`
dx build --release --platform desktop
```

## 4. Sampler CPU to < 1.5% of one core

Currently ~3.5% at 510 processes / 1 Hz (see `PERF.md`). Remaining levers:

- Cache the resolved group key per pid across ticks; only recompute `base_key` when the
  pid's `app_id`/`exe` changes. This removes most per-tick String hashing/allocation in
  `assign_groups`.
- Read `/proc/<pid>/stat` through a reused buffer and parse fields without the intermediate
  `Vec<&str>` to cut per-tick allocations.
- Optionally widen the PSS refresh window (fewer `smaps_rollup` reads) at the cost of a
  slightly laggier memory column.

## 5. Whole-app RSS to < 120 MB

The WebKitGTK webview alone is ~150 MB, which is the floor for a Dioxus-desktop app. Hitting
120 MB would require a non-webview backend (e.g. a native renderer). Recorded honestly in
`PERF.md` rather than worked around.

## 6. ARM packaging

The `PKGBUILD` lists `aarch64` but no ARM build was performed here.

## 7. Smaller polish gaps

- Process-table **virtualization** for > 200 rows: group mode renders ≤ ~150 rows, but flat
  mode with 400+ processes renders up to 400 rows (capped). Windowing would help only on
  very large flat lists.
- Per-group **energy since launch (Wh)** in the row tooltip: system Wh is shown in the Power
  view; per-group Wh would need a per-group energy accumulator in the sampler.
- Reclaim actions show current numbers; an explicit before/after pair could be captured
  around the privileged call.
