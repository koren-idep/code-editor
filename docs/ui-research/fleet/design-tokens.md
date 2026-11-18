# JetBrains Fleet UI Research — Design Tokens

Values below marked **(measured)** were sampled directly from `screenshots/01-main-workbench-toast.png` (`1280×800`). Values marked **(estimated)** are inferred from visual inspection of screenshots and standard desktop proportions.

## Color Palette (Dark Theme)

| Token Role | Observed Hex | Notes |
|---|---|---|
| `canvas.bg` | `#090909` | **(measured)** Outer window shell, top toolbar, inter-island gaps, bottom status bar |
| `island.bg` | `#18191b` | **(measured)** Sidebar island and editor island surface |
| `surface.elevated` | `#252629` | **(measured)** Floating notification toast / popover surface |
| `surface.pill.active` | `#28292b` | **(measured)** Active editor tab pill, active sidebar header tab (`Files`), selected tree row |
| `border.floating` | `#414245` | **(measured)** `1px` border on floating toast/popover cards |
| `border.subtle` | `#26272a` | **(estimated)** Subtle hairline divider inside toolbars/islands |
| `editor.line.active` | `#152639` | **(measured)** Current-line highlight across editor width |
| `text.primary` | `#e3e4e7` / `#ffffff` | **(measured)** Active tab title, primary code text, bold headings |
| `text.secondary` | `#86868a` | **(measured)** Inactive tabs, status bar metadata, toast body, inlay hints |
| `text.muted` | `#5a5b5f` | **(estimated)** Line numbers, indent guides, disabled icons |
| `accent.primary` | `#3574f0` | **(estimated)** Primary focus ring, active selection accent, primary buttons |
| `status.error` | `#e05252` | **(estimated)** Error icon (red lightning bolt in toolbar & toast) |
| `status.success` | `#499c54` | **(estimated)** Green run gutter triangle |

## Spacing & Radii

| Token | Value | Notes |
|---|---|---|
| `space.outer_margin` | `8px` | **(measured)** Left/right margin between window edge and islands (`x=0..7`, `x=1272..1279`) |
| `space.island_gap` | `8px` | **(measured)** Horizontal gap between sidebar island and editor island (`x=368..375`) |
| `space.xs` | `4px` | **(estimated)** Icon-to-label gap, tight inline padding |
| `space.sm` | `8px` | **(estimated)** Standard element padding, tab internal horizontal gap |
| `space.md` | `12px` | **(estimated)** Panel internal horizontal padding, toast padding |
| `space.lg` | `16px` | **(estimated)** Dialog/section spacing |
| `radius.island` | `8px` | **(estimated)** Corner radius of sidebar and editor islands |
| `radius.floating` | `8px` | **(estimated)** Corner radius of toast/popover cards |
| `radius.pill` | `6px` | **(estimated)** Active tab pill, sidebar tab pill, selected tree row highlight |
| `radius.control` | `4px` | **(estimated)** Buttons, checkboxes, small badges |
