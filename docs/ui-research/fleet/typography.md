# JetBrains Fleet UI Research — Typography

All font sizes and line heights below are **(estimated)** from `screenshots/01-main-workbench-toast.png` (`1280×800` at `1.0×` scale).

## Font Families

- **UI Sans:** Clean geometric/humanist sans-serif (system UI sans / Inter-like metrics) for toolbars, tabs, file tree, notifications, and status bar.
- **Code Monospace:** JetBrains Mono-style monospace with clear distinction between `0`/`O` and `1`/`l`/`I`, supporting italic variants for inlay hints and comments.

## Type Scale

| Role | Size (est.) | Weight | Line Height (est.) | Usage |
|---|---|---|---|---|
| `ui.title` | `13px` | SemiBold (`600`) | `18px` | Notification titles, project root header (`fleet-sample-project`), dialog headers |
| `ui.body` | `13px` | Regular (`400`) | `18px` | Tree items, active/inactive tab labels, toast body text, menu items |
| `ui.tab` | `12.5px–13px` | Medium (`500`) | `18px` | Sidebar section tabs (`Files`, `Search`), editor tabs |
| `ui.caption` | `12px` | Regular (`400`) | `16px` | Top bar workspace title, status bar breadcrumbs (`src / main.rs`), `1:1 UTF-8 Rust` |
| `code.editor` | `13px` | Regular (`400`) | `20px–21px` | Editor buffer text and line numbers |
| `code.inlay` | `12px` | Italic (`400i`) | `20px` | Inline type/parameter hints (`: HashMap<&str, &str>`, `title:`, `count:`) |

## Typographic Hierarchy Rules

- **Muted Chromatic Contrast:** Non-focused metadata (workspace title in top bar, status bar items, inactive tabs) uses `#86868a` rather than pure white, keeping visual attention on the active editor buffer.
- **Tabular Numerals:** Line numbers and cursor coordinates (`1:1`) align cleanly without horizontal jitter during scrolling or cursor movement.
- **Truncation:** Long notification titles and paths truncate with an ellipsis (`…`) rather than wrapping awkwardly across narrow floating cards.
