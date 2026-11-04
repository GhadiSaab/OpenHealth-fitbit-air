# Design system

Calm and readable, a tool you can use every day. Tokens live in
`app/style/tokens.css`; components in `app/style/app.css` and `app/src/ui/`.

## Tokens

| Token | Light | Dark | Use |
|---|---|---|---|
| `--page` | `#F6F3EC` warm ivory | `#121314` | background |
| `--surface` | `#FFFDF8` | `#1C1D1F` | cards, sheets |
| `--surface-2` | `#EFEBE2` | `#26272A` | tracks, pressed |
| `--ink` | `#1A1A18` | `#F2F0EB` | primary text |
| `--ink-2` | `#4A4843` | `#CFCCC4` | secondary text |
| `--muted` | `#66635D` | `#A9A69E` | labels, axes (≥ 4.5:1) |
| `--activity` | `#1F8A5B` | `#2FA472` | steps, distance, calories |
| `--sleep` | `#6E56CF` | `#8A75E6` | sleep, AI answers |
| `--heart` | `#D2604F` | `#D0604F` | heart metrics |
| `--oxygen` | `#2B7BB9` | `#4A93D6` | SpO₂ |
| `--variability` | `#A8781F` | `#AE7F25` | HRV |
| `--temperature` | `#B8467F` | `#C0609A` | skin temperature |
| `--breath` | `#0B93A6` | `#1C9FAE` | respiratory rate |
| `--stage-awake/rem/light/deep` | `#EB6834 #2A78D6 #1BAF7A #6250D6` | `#D95926 #3987E5 #199E70 #9085E9` | sleep stages |

Sleep-stage colours pass the palette validator in both themes (lightness
band, chroma floor, colour-vision-deficiency separation and the normal-vision
floor). The light-theme aqua is below 3:1 against the surface, so the stage is
also encoded by row position, row labels, a legend with totals, and a table.
Metric tones are identity accents. They are never plotted as adjacent series
and always appear with an icon and a label.

* **Type**: Inter (variable, bundled, OFL). Tabular figures for axes, tables
  and readouts; proportional figures for the hero number. Scale 13/14/16/19/24/32/48.
* **Spacing**: 4-pt scale (`--s1`…`--s12`). Touch targets ≥ 48 px.
* **Radius**: 10 / 16 / 22 px; pills for chips and buttons.
* **Motion**: 150–300 ms ease-out on press, sheet and ring transitions only.
  `prefers-reduced-motion` and the in-app setting reduce all animation.

## Components

`Header`, `TabBar` (bottom bar on phones, side rail from 900 px), `Segmented`
(radio semantics, arrow keys), `Switch` (`role="switch"`), `SheetFrame`
(dialog, Escape to close), `Section`, `EmptyState`, `DemoBanner`, status
strip, pills (`live`, `recent`, `stale`, `hist`), tiles, `Ring`,
`Sparkline`, `BarChart`, `LineChart`, `IntradayChart`, `SleepTimeline`,
`DataTable`.

## Charts

* Bars ≤ 24 px with a 4 px rounded data end and square baseline; 2 px lines;
  markers r = 4 with a 2 px surface ring; hairline solid gridlines.
* Missing days are a hollow marker on the baseline, never a zero bar; lines
  break at gaps; recording gaps are shaded and labelled "No recording".
* Every chart is focusable: ←/→/Home/End move the selection, the readout is
  an `aria-live` region, and "Show table" gives a full table.
* Text uses ink tokens, never series colours.

## Language

Factual and calm. No guilt, streaks or alarms. Associations are described as
associations ("compared with … on other days") with counts and windows, and
never as causes. AI output is labelled "AI-generated" and visually distinct
from measurements.
