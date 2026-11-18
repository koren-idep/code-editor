# ADR-001: Native Rust UI Architecture for Koren IDE Platform

**Status:** Accepted  
**Date:** 2026-11-17  
**Decision:** Build Koren's native declarative UI framework (`koren-ui`) on **`wgpu` + `winit` + `taffy` + `cosmic-text` + `accesskit` + `arboard`**, using a GPUI-inspired single-pass SDF quad & glyph-atlas renderer and reactive declarative component tree.

---

## 1. Context

Koren IDE Platform currently ships `apps/koren-code` using `tao` + `wry` (WebKitGTK on Linux) with a TypeScript/CodeMirror 6 frontend. While functional, the webview introduces significant idle memory overhead, slower cold startup, and limits fine-grained control over GPU frame pacing and desktop integration.

We need a native Rust UI architecture that:
- Matches **Fleet's** polished island-on-canvas desktop aesthetic, compact layouts, and rich floating surfaces.
- Approaches **Zed's** low-overhead, GPU-accelerated responsiveness.
- Complies with Koren's workspace constraints: **MIT license compatibility**, **Rust 1.85+**, **`unsafe_code = "forbid"`** in Koren workspace crates, Linux-first (Wayland + X11) with future Windows/macOS portability, and reusability across multiple Koren Platform applications.

---

## 2. Options Evaluated

### Option A: Zed's GPUI (`gpui` / `gpui-fast`)
- **License:** Apache-2.0 (compatible with MIT).
- **Strengths:** Purpose-built for code editors; single-pass GPU SDF quads, shadows, and glyph atlas; Taffy flexbox layout; entity/view reactive model; subpixel-aware rendering.
- **Concerns:**
  - The crates.io `gpui` (`0.2.2`) and `gpui-fast` (`0.1.13`) releases carry Zed-specific abstractions, custom platform backends (`blade-graphics` / custom Linux Wayland/X11 shims), and heavy transitive dependencies.
  - Tightly couples application architecture to Zed's `AppContext`/`Entity` lifecycle, making it harder to cleanly adapt to Koren's existing synchronous/channel-based `koren-core` `EventQueue` and `koren-protocol` services without significant impedance mismatch.
  - Accessibility (`accesskit`) and IME integration in standalone `gpui` snapshots are incomplete or tied to Zed's internal forks.

### Option B: `wgpu` + Custom Declarative UI Layer (`koren-ui`) — **SELECTED**
- **License:** `wgpu` (MIT/Apache-2.0), `winit` (Apache-2.0), `taffy` (MIT), `cosmic-text` (MIT/Apache-2.0), `accesskit` (MIT/Apache-2.0), `arboard` (MIT/Apache-2.0). All 100% compatible with Koren's MIT license.
- **Strengths:**
  - **Zero `unsafe` in Koren crates:** All GPU, windowing, text shaping, clipboard, and accessibility primitives are encapsulated in mature, well-maintained crates, allowing `koren-ui` and `apps/koren-native` to strictly obey `unsafe_code = "forbid"`.
  - **GPUI-style rendering without reinventing GPU drivers:** `wgpu` handles Vulkan/GL/DX12/Metal swapchains and shaders. On top of `wgpu`, a compact (~400-line) instanced WGSL shader renders rounded rectangles with per-corner radii, 1px borders, soft drop shadows, scissor clipping, and alpha-blended glyph atlas quads in **1–2 batched draw calls per frame**.
  - **Best-in-class text & layout:** `cosmic-text` provides HarfBuzz/rustybuzz text shaping, fallback fonts, Unicode/grapheme clustering, and crisp glyph rasterization; `taffy` provides full Flexbox/Grid layout with incremental dirty-tree invalidation.
  - **First-class desktop integration:** `winit` provides native Wayland/X11/Windows/macOS HiDPI scaling, IME preedit/commit events, and demand-driven redraws (`ControlFlow::Wait`); `accesskit` provides AT-SPI2 screen-reader trees on Linux; `arboard` provides clipboard support.
  - **Clean Koren-native API:** Declarative `Column`, `Row`, `Stack`, `SplitPane`, `ScrollView`, and themed widgets (`Button`, `TabBar`, `TreeView`, `TextInput`, `Checkbox`, `Toggle`, `Menu`, `Dialog`, `Tooltip`, `Toolbar`, `Popover`, `CodeEditor`) designed specifically around Koren's `ThemeTokens` and `koren-core` services.

### Option C: Slint
- **License:** GPLv3 or commercial proprietary license.
- **Disqualification:** GPLv3 is incompatible with Koren's MIT open-source distribution model, and commercial licensing creates friction for contributors and downstream Koren Platform applications.

### Option D: Iced
- **License:** MIT.
- **Strengths:** Mature Elm-architecture declarative Rust UI on `wgpu`/`winit` with `cosmic-text`.
- **Concerns:**
  - Strict Elm message-passing (`update`/`view`) makes high-frequency IDE interactions (virtualized 100k-line code editor, multi-cursor selection, drag-resizing split panes, floating cursor-anchored autocomplete popups, custom window chrome) cumbersome.
  - Less direct control over batched shadow/island rendering and demand-driven dirty-region invalidation.

### Option E: Skia (`skia-safe`)
- **License:** BSD-3-Clause (Skia) / MIT (`skia-safe`).
- **Concerns:** Heavy C++ toolchain dependency (`gn`/ninja/clang or prebuilt binaries), large binary footprint, slower CI/cross-compilation, and requires building a full layout + widget + windowing stack anyway.

---

## 3. Consequences

- **Positive:**
  - `koren-ui` is lightweight, fast to compile, 100% safe Rust (`unsafe_code = "forbid"`), and MIT-compatible.
  - Centralized `ThemeTokens` (Dark + OLED-Black) enable Fleet-level visual polish without hardcoding colors in widgets.
  - Demand-driven rendering (`ControlFlow::Wait` + dirty flag) yields near-0% idle CPU usage and minimal memory footprint compared to WebKitGTK.
- **Trade-offs:**
  - We own the widget set and `CodeEditor` widget implementation, which we will build incrementally while preserving `apps/koren-code` (CodeMirror 6) until feature parity is reached.
