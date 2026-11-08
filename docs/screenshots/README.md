# Screenshots

Six captures in **Rosé Pine**: overview, processes (grouped + expanded), memory (stacked bar
and treemap), power, ports (grouped by process), and the settings panel.

They are rendered headlessly — no compositor capture needed:

```sh
PULSAR_DUMP_SIZE=180x48 PULSAR_DUMP_THEMES=rose-pine \
  pulsar --dump /tmp/pulsar-shots --png
```

The dump writes `<index>-<view>-<theme>.png` / `.txt`; the files here are those captures
renamed to `overview.png`, `processes.png`, `memory.png`, `power.png`, `ports.png` and
`settings.png`.

Each view also has a plain-text render available from the same command (without `--png`).
