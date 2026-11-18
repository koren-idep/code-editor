# JetBrains Fleet UI Research — Interactions

## Window & Panel Management
- **Custom Title Bar Dragging:** Empty regions of the `36px` top toolbar act as window drag handles; double-clicking toggles maximize/restore.
- **Panel Toggles:** Top-left toolbar icons toggle left, bottom, and right islands independently, preserving split ratios when restored.
- **Split Resizing:** Hovering the `8px` gap between islands displays a resize cursor (`col-resize` / `row-resize`) and highlights a subtle vertical/horizontal drag indicator.

## Keyboard & Focus Navigation
- **Focus Hierarchy:** Clear distinction between sidebar focus, editor focus, and modal/popover focus.
- **Keyboard Shortcuts:**
  - `Ctrl+K` / `Shift+Shift`: Search Everywhere / Command Palette
  - `Ctrl+,`: Open Settings
  - `Ctrl+W`: Close active editor tab
  - `Ctrl++` / `Ctrl+-` / `Ctrl+0`: Interface scaling (Koren requirement)
- **Escape Behavior:** `Esc` dismisses floating popovers, toasts, autocomplete menus, or command palettes and returns focus to the active editor.

## Hover, Selection & Feedback States
- **Subtle Hover:** Interactive buttons and tree rows brighten slightly (`#18191b` -> `#222326` -> `#28292b`) without jarring color flashes.
- **Tab Close & Dirty State:** Tab close `×` is subtle on inactive tabs and brightens on hover.
- **Toast Auto-Dismiss / Stack:** Notifications appear in the top-right corner over the editor island without shifting layout.
