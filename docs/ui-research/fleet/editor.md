# JetBrains Fleet UI Research — Editor Surface

Observed from `screenshots/01-main-workbench-toast.png` (`main.rs` open in `1280×800` window).

## Gutter & Line Numbers
- **Gutter Width:** `~52px–56px` (est.) accommodating up to 3–4 digit line numbers plus a `16px` action column (e.g., green run play icon next to `fn main()`).
- **Line Number Alignment:** Right-aligned muted numerals (`#5a5b5f` est.), brighter on the active line.
- **Current Line Highlight:** Full-width `#152639` band spanning both the gutter/line-number area and the code viewport.

## Code Rendering & Visual Aids
- **Syntax Highlighting:** High-contrast dark-mode palette on `#18191b`:
  - Keywords (`use`, `mod`, `fn`, `let`, `mut`, `if`, `else`): Warm orange/amber (`#cf8e6d` est.)
  - Types (`HashMap`, `AppState`, `String`): Teal/cyan-blue (`#56a8f5` est.)
  - Strings (`"theme"`, `"dark"`, `"Koren Sample"`): Soft green (`#6aab73` est.)
  - Comments (`/// Application entry point...`): Muted gray (`#7a7e85` est.)
  - Function calls / identifiers: Crisp off-white (`#bcbec4` est.)
- **Indent Guides:** Thin `1px` vertical lines at tab-stop intervals inside indented blocks.
- **Inlay Hints:** Italicized inline type and parameter annotations (`: HashMap<&str, &str>`, `title:`, `count:`) rendered in muted gray (`#86868a`) with slight horizontal padding so they read distinctly from source code.
- **Cursor:** Crisp `2px` vertical bar in bright white (`#ffffff`) on the active line.

## Editor Splitting & Overlays
- **Splits:** Editor island can split horizontally or vertically into nested editor groups, each with its own tab strip and `1px` or `4px` divider.
- **Diagnostics & Autocomplete:** Inline squiggles for warnings/errors; floating `#252629` completion popup anchored to the cursor with `1px solid #414245` border, item icon, label, and right-aligned type signature.
