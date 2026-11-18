# Feature Parity Checklist (`koren-code` vs `koren-native`)

`apps/koren-code` (`tao` + `wry` + CodeMirror 6) remains the primary production frontend until all items below reach full parity.

| Capability | `koren-code` | `koren-native` | Status |
|---|---|---|---|
| **Window Chrome & Layout** | Custom HTML/CSS | Native `wgpu` islands (`36px` toolbar, `28px` status bar) | **Implemented** |
| **Themes (Dark + OLED-Black)** | CSS themes | Centralized `ThemeTokens::dark()` & `oled_black()` | **Implemented** |
| **UI Scaling (`Ctrl++`/`-`/`0`)** | Browser zoom | Native logical/HiDPI scaling (`0.7×`–`2.0×`) | **Implemented** |
| **Workspace File Explorer** | `koren-workspace` | `koren-workspace` (`WorkspaceManager`, virtualized `TreeView`) | **Implemented** |
| **Document Open / Edit / Save** | `koren-documents` + CodeMirror 6 | `koren-documents` + native `CodeEditorState` (UTF-8/UTF-16, undo/redo) | **Implemented** |
| **Syntax Highlighting** | CodeMirror 6 grammars | Native tokenizer (Rust, TOML, JSON, Lua, Markdown) | **Implemented (Basic)** |
| **Command Palette & Search** | TypeScript modal/panel | Native `CommandPalette` (`Ctrl+K`) & sidebar `Search` tab | **Implemented** |
| **Settings UI** | HTML settings modal | Native `Settings` modal (`Ctrl+,`) with `Toggle` & `Checkbox` | **Implemented** |
| **Run Approval Dialog** | Graphical approval dialog | Native `RunApproval` modal showing exact command & CWD | **Implemented** |
| **Git Status / Branch** | `koren-git` panel | Branch indicator in status bar | **Partial** |
| **LSP Diagnostics & Completion** | `koren-lsp` + CodeMirror | Pending full `koren-lsp` & `koren-inspections` wiring | **Pending** |
| **Integrated Terminal (PTY)** | `koren-terminal` + xterm | Bottom `Terminal` tab placeholder; PTY stream wiring pending | **Pending** |
| **DAP Debugging UI** | `koren-debug` panel | Pending native variables/breakpoints/callstack views | **Pending** |
| **Lua Extension UI Contributions** | `koren-extensions` + `koren-lua` | Pending native contribution host | **Pending** |
| **Accessibility & IME** | WebKitGTK | `accesskit` tree builder + `winit` IME preedit/commit + `arboard` clipboard | **Partial** (AccessKit adapter wiring to `winit` pending) |
