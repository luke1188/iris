# Live Visualizer

Real-time DJ music visualizer for Windows (TV/projector). Native Rust + wgpu + egui — **not** a video editor.

**Current milestone: Phase 3–6 lite** — stage images, band tuning, particles, visual presets.

## Requirements

- Rust 1.75+ (edition 2021)
- Windows 10/11 x64 (primary target)
- A working audio input (interface, mic, Serato Virtual Audio Cable, Stereo Mix, etc.)

macOS / Linux can build for development; WASAPI loopback notes below apply to Windows only.

## Build & run

```bash
cargo run --release
```

GitHub Actions builds **Windows x64**, **macOS Apple Silicon**, and **macOS Intel** release zips on pushes/PRs to `main` (Artifacts) and attaches them to GitHub Releases when you push a `v*` tag (e.g. `v0.1.0`).

If a previous run left the process hung (audio device busy), force-quit it first, then relaunch. Closing the window now releases the audio device on exit.

### Presets

In the **PRESETS** panel (or `presets/` folder):

| Preset | Look |
|--------|------|
| `rgb` | Classic RGB bar ring |
| `mono` | Clean grayscale / non-RGB |
| `smooth` | Filled cyan spectrum shape |
| `particles` | RGB soft glow + ambient particles |
| `bar` | Loud-venue tuning + magenta smooth + particles |
| `neon` | High-energy RGB with particles |
| `default` | Baseline |

## Loud bar / bass pinning

Open **BAND TUNING**, or load the **`bar`** preset:

1. Raise **Threshold / gate**
2. Lower **Bass** + **Spectrum bass tilt**
3. Lower **Ceiling** for soft-clip headroom

## Controls

| Key | Action |
|-----|--------|
| **F11** | Toggle fullscreen |
| **Esc** | Exit fullscreen |
| **F1** | Toggle settings UI |



## Audio notes (Windows)

- Devices come from the default cpal host (**WASAPI** on Windows).
- **Serato Virtual Audio Cable**, VB-Cable, Voicemeeter, and USB interfaces appear as normal inputs when installed.
- **True WASAPI speaker loopback** (capture what the speakers are playing without a virtual cable) is **not fully wired yet**. Devices named like Stereo Mix / loopback / cable output are flagged in the UI when present.
- Recommended DJ setup for now:
  1. Route Serato master → **Serato Virtual Audio Cable** (or VB-Cable)
  2. Select that cable in Live Visualizer
  3. Fullscreen the visualizer on the TV/projector monitor (monitor picker arrives in Phase 7)

## Project layout

```
src/
  main.rs
  app/          # state + egui UI
  audio/        # capture, FFT, analyzer, beat
  renderer/     # wgpu stubs (Phase 3+)
  config/       # settings + preset schema
  display/      # fullscreen helpers
presets/        # TOML presets (loaded in Phase 7)
```

## Architecture

```
Audio device (cpal stream thread)
        ↓
   Ring buffer
        ↓
 Analyzer (FFT → bands → smooth → beat)
        ↓
   AudioFeatures
        ↓
  egui preview (Phase 1–2) / wgpu renderer (Phase 3+)
```

The renderer will consume `AudioFeatures` only — it never opens the audio device.

## Roadmap

| Phase | Focus |
|-------|--------|
| 3 | Background image, center disc, logo |
| 4 | Circular FFT spectrum (GPU) |
| 5 | RGB gradient + glow |
| 6 | GPU particles + beat reactions |
| 7 | Second-monitor fullscreen, presets, persistence |
| 8 | Performance polish + packaging |

## License

MIT
