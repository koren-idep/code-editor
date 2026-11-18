# JetBrains Fleet UI Research — Overview

**Observed version:** JetBrains Fleet `1.48.261` (Linux x86_64 archive `Fleet-1.48.261.tar.gz`)  
**Methodology:** Black-box visual and behavioral inspection of Fleet running locally on Wayland/XWayland (`1280×800` default window) with a temporary Rust workspace (`/tmp/fleet-sample-project`).  
**Reference capture:** `screenshots/01-main-workbench-toast.png`

## Core Architectural Philosophy

Fleet's UI is built around an **island-on-canvas** desktop model rather than a traditional edge-to-edge split-pane IDE:

1. **Dark Outer Canvas (`#090909`)**: The window frame, top toolbar, inter-panel gutters (`8px`), and bottom status bar sit directly on a near-black shell canvas.
2. **Elevated Island Surfaces (`#18191b`)**: Major functional regions—the left tool/project sidebar and the central editor region—are rendered as rounded-rectangle cards (`~8px` corner radius, estimated from screenshot) separated by `8px` canvas gaps.
3. **Restrained, Compact Chrome**: Top toolbar (`36px` tall) and bottom status bar (`28px` tall) avoid heavy gradients, drop shadows, or bulky bevels. Separation is achieved through subtle luminance steps (`#090909` -> `#18191b` -> `#28292b`) rather than bright 1px grid lines.
4. **Contextual Floating Surfaces (`#252629`)**: Notifications, popovers, command palettes, and menus float above islands with a distinct `1px` border (`#414245`) and soft drop shadow, maintaining clear z-order hierarchy.
5. **High Information Density without Clutter**: Compact tree rows, single-row tab pills, inline type/parameter inlay hints, and a minimal status strip keep focus on code.

## Key Takeaways for Koren IDE Platform

- **Adopt:** Island-on-canvas layering, subtle luminance contrast, compact `36px` top bar and `28px` status strip, pill-shaped active tabs/selections, and clean `1px` bordered floating overlays.
- **Preserve Koren Identity:** Use Koren's own icon set, brand mark, and platform-first security/approval surfaces (e.g., explicit task/run approval dialogs) rather than copying Fleet branding or assets.
- **Provide Both Themes:** A Fleet-inspired Dark theme (`#090909` canvas / `#18191b` islands) and an OLED-Black variant (`#000000` canvas / `#0b0c0e` islands with crisp `1px` borders).
