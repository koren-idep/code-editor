# Native UI Performance Benchmarks

**Environment:** Linux (Wayland `labwc:wlroots`), Vulkan 1.4 (`wgpu 27.0.1`), Rust `1.99.1`, `--release` profile, workspace `/tmp/fleet-sample-project`.

## Methodology

- **Startup (`koren-native`):** Measured wall-clock time from `main()` entry (`Instant::now()`) to first presented GPU frame (`--smoke` flag) across 3 consecutive runs.
- **Idle Memory (RSS):** Measured `VmRSS` via `ps -o pid,ppid,rss,%cpu,comm` 3–4 seconds after launching each application on `/tmp/fleet-sample-project`. For `koren-code`, RSS includes its spawned `WebKitWebProcess` and `WebKitNetworkProcess` child processes.
- **Binary Size:** Measured via `stat -c '%s'` on `target/release/koren-native` and `target/release/koren-code`.

## Measured Results

| Metric | `koren-code` (`tao` + `wry` / WebKitGTK) | `koren-native` (`koren-ui` + `wgpu`) | Delta |
|---|---|---|---|
| **Binary Size** | `16,653,344` bytes (`15.88 MiB`) | `16,000,368` bytes (`15.26 MiB`) | `-3.9%` |
| **Total Idle RSS** | `401,948 KB` (`392.5 MiB`) *(155,204 + 55,300 + 191,444)* | `75,964 KB` (`74.2 MiB`) *(single process)* | **`-81.1%` (5.29× lower)** |
| **First-Frame Startup** | Not instrumented (`--smoke` not supported on `koren-code` CLI) | `140.73 ms` – `164.62 ms` (3 runs: `153.00`, `164.62`, `140.73`) | — |
| **Process Count** | 3 (`koren-code` + 2 WebKit processes) | 1 (`koren-native`) | `-2` processes |

## Optimization Techniques Implemented in `koren-ui`

- **Demand-Driven Event Loop:** `ControlFlow::Wait` ensures zero idle redraw loops when no input or platform events occur.
- **Batched GPU Submission:** Consecutive quads and glyphs sharing a scissor clip are merged into minimal instanced draw calls (`vs_quad`/`fs_quad` and `vs_text`/`fs_text`).
- **Virtualized Lists & Editor:** `paint_tree_view` and `CodeEditorState::paint` only process and emit draw commands for rows/lines currently intersecting the viewport.

## Remaining Benchmark Work

- Frame-time percentile histograms (`p50`/`p95`/`p99`) during continuous 100k-line scrolling and rapid window resizing.
- Large-workspace (`10,000+` files) tree expansion and search latency comparison.
