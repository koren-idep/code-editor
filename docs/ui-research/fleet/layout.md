# JetBrains Fleet UI Research — Layout Architecture

Measurements below are from `screenshots/01-main-workbench-toast.png` (`1280×800`).

## Primary Window Regions

```text
+-----------------------------------------------------------------------+ 0px
| [Logo] [Menu] [L][B][R] | [Grid][Collab]  fleet-sample-project  [⚡][▶][🔍][🔔][⚙] [-][□][×] | 36px
+-----------------------------------------------------------------------+
| 8px | +---------------------+ | 8px | +-----------------------------+ | 8px
|     | | Files  Search  +    | |     | | [⚙ main.rs ×]               | |
|     | |---------------------| |     | |-----------------------------| |
|     | | fleet-sample-project| |     | |  1 | use std::collections.. | |
|     | | > docs              | |     | |  2 |                        | |
|     | | v src               | |     | |  3 | mod ui;                | |
|     | |   > ui              | |     | |  ...                        | |
|     | |   [main.rs]         | |     | |                             | |
|     | +---------------------+ |     | +-----------------------------+ |
+-----------------------------------------------------------------------+ 772px
| src / main.rs                                        1:1  UTF-8  Rust | 800px
+-----------------------------------------------------------------------+
```

## Exact Measured Dimensions (`1280×800`)

- **Top Toolbar:** `y = 0..36` (`36px` height), full width `1280px`, background `#090909`.
  - Left cluster: App icon, hamburger menu, 3 layout toggles (left panel, bottom panel, right panel), vertical separator, workspace grid icon, collaboration icon.
  - Center cluster: Centered workspace name (`fleet-sample-project`).
  - Right cluster: Smart mode / backend status, Run action, Search Everywhere, Notifications, Settings gear, and window controls (`Minimize`, `Maximize`, `Close`).
- **Main Content Row:** `y = 36..772` (`736px` height).
  - Left outer margin: `8px` (`x = 0..8`).
  - Left Sidebar Island: `360px` wide (`x = 8..368`), background `#18191b`, rounded corners (`~8px` est.).
  - Inter-island Gap: `8px` (`x = 368..376`), background `#090909`.
  - Editor Island: `896px` wide (`x = 376..1272`), background `#18191b`, rounded corners (`~8px` est.).
  - Right outer margin: `8px` (`x = 1272..1280`).
- **Bottom Status Bar:** `y = 772..800` (`28px` height), full width `1280px`, background `#090909`.
  - Left: File breadcrumb (`src / main.rs`).
  - Right: Cursor position (`1:1`), encoding (`UTF-8`), language mode (`Rust`).

## Docking & Resizing Principles

- **Collapsible Islands:** Toggling the left, bottom, or right panel buttons in the top toolbar hides/shows the corresponding island and redistributes the `8px` gap cleanly so the remaining islands expand to fill available width/height.
- **Bottom Panel (Tool Drawer):** When opened, sits below the editor island (or spans below sidebar + editor depending on layout mode) with an `8px` vertical gap on `#090909`.
- **Responsive Narrow Widths:** At smaller window widths, the sidebar narrows or collapses automatically, and the centered workspace title truncates before overlapping the left/right toolbar clusters.
