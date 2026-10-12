# Screenshots

Generated headlessly with Pulsar's `--dump` mode (no compositor screen-capture API needed):

```sh
PULSAR_DUMP_THEMES="catppuccin-mocha,tokyo-night" \
  pulsar --dump docs/screenshots --png
```

Each view is written as a plain-text render (`.txt`) and a coloured PNG (`.png`).

| Prefix | Theme |
|---|---|
| `01-*` | Catppuccin Mocha |
| `02-*` | Tokyo Night |

Views: `overview`, `processes` (grouped + expanded), `memory` (stacked bar + treemap),
`power`.

The `processes` captures on this box show Zen Browser folding 32 helper processes (Web
Content, Privileged Cont, WebExtensions, Isolated Web Co, …) under one row, and three
`opencode` processes as one `opencode (3)` row.
