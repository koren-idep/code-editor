# Koren Native UI (`koren-ui` & `koren-native`)

`koren-ui` is Koren IDE Platform's native Rust declarative UI framework, built on `wgpu`, `winit`, `taffy`, `cosmic-text`, `accesskit`, and `arboard` with `#![forbid(unsafe_code)]`.

## Building and Running

```bash
# Run koren-ui unit tests
cargo test -p koren-ui

# Build and run the native IDE on a workspace
cargo run -p koren-native -- /path/to/workspace

# Run headless/first-frame smoke check
cargo run -p koren-native -- /path/to/workspace --smoke
```

## Keyboard Shortcuts

- `Ctrl+K`: Open Command Palette
- `Ctrl+,`: Open Settings (Dark / OLED-Black theme, editor toggles)
- `Ctrl+J`: Toggle Bottom Panel (`Output`, `Problems`, `Terminal`)
- `Ctrl+S`: Save active document
- `Ctrl+W`: Close active tab
- `Ctrl++` / `Ctrl+-` / `Ctrl+0`: Increase, decrease, or reset UI scaling
- `Tab` / `Shift+Tab`: Cycle keyboard focus across interactive controls
- `Esc`: Dismiss active modal, menu, or notification toast

## Extending `koren-ui`

1. **Themes:** Add or customize `ThemeTokens` in `crates/koren-ui/src/theme.rs` without touching widget rendering code.
2. **Layouts:** Compose interfaces declaratively using `Column`, `Row`, `Spacer`, and `layout_and_paint`.
3. **Widgets:** Reuse `Button`, `Checkbox`, `Toggle`, `TextInput`, `TabBar`, `TreeView`, `Menu`, `Dialog`, `Toast`, `Scrollbar`, and `CodeEditorState` from `crates/koren-ui/src/widgets.rs` and `editor.rs`.
4. **Applications:** Implement `koren_ui::app::NativeApp` and launch with `koren_ui::run_app(app, false)`.
