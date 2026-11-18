# JetBrains Fleet UI Research — Components

Observed component anatomy from `screenshots/01-main-workbench-toast.png` and standard Fleet desktop patterns.

## 1. Toolbar & Window Controls
- **Icon Buttons:** `~24×24px`–`28×28px` hit target (est.) with `16×16px` monochrome/semantic stroke icons, subtle hover background (`#28292b`), `4px–6px` corner radius.
- **Window Controls (Linux/XWayland):** Custom-drawn right-aligned `Minimize` (`—`), `Maximize` (`□`), `Close` (`×`) buttons integrated into the `36px` top bar.

## 2. Sidebar Header Tabs (`Files`, `Search`, `+`)
- **Active Tab Pill:** Rounded pill (`#28292b` fill, `~6px` radius est.) with `#ffffff` text.
- **Inactive Tab:** Transparent background, `#86868a` text, transitions to `#28292b` on hover.
- **Add Button (`+`):** Compact icon button to open additional tool views.

## 3. Tree View (Project Explorer)
- **Row Height:** `~24px–26px` (est.), full-width row with `8px` horizontal inset inside the island.
- **Selected Row:** Full-row rounded pill (`#28292b`), white text and crisp file-type icon.
- **Disclosure Chevron:** `>` (collapsed) / `v` (expanded) at left of directory rows, `~12px` indent per nesting level.
- **File Icons:** Distinct compact `16×16px` file-type badges (e.g., Rust gear, Markdown `M↓`, TOML `[T]`).

## 4. Editor Tabs
- **Tab Strip:** Sits at top of the editor island (`~36px–38px` tall est.) with `#18191b` background.
- **Active Tab:** Elevated pill (`#28292b`, `~6px` radius est.) containing file icon, filename (`main.rs`), and close `×` button.
- **Dirty Indicator:** Unsaved state replaces or augments the close button with a small dot until hovered.

## 5. Floating Notification Toast
- **Container:** `~320px` wide (est.), `#252629` background, `1px solid #414245` border, `~8px` radius, soft drop shadow.
- **Layout:** Left status icon (`16×16px`) + right vertical stack of bold title (`#e3e4e7`) and muted multi-line message (`#86868a`).

## 6. Menus, Popovers, Dialogs & Form Controls (Inferred / Standard Fleet Patterns)
- **Context Menus & Dropdowns:** `#252629` surface, `1px solid #414245`, `8px` radius, `4px` inner padding, `26px` row height, right-aligned shortcut hints in `#86868a`.
- **Command Palette / Search Everywhere:** Centered floating modal (`~600px` wide) with top search input, filter pills, and virtualized result list.
- **Buttons:** Primary (`#3574f0` fill, white text), Secondary (`#28292b` fill, `1px solid #414245` border), `4px–6px` radius, `28px` height.
- **Checkboxes & Toggles:** `16×16px` rounded-square (`4px` radius) checkbox; pill toggle (`~32×18px`) with smooth thumb slide.
- **Scrollbars:** Thin overlay track (`~6px–8px`), pill thumb with low-opacity fill that brightens on hover.
