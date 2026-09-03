# Iris Visualizer

Real-time DJ music visualizer for Windows (TV/projector). Native Rust + wgpu + egui

## Requirements

- Rust 1.75+ (edition 2021)
- Windows 10/11 x64 (primary target)
- A working audio input (interface, mic, Serato Virtual Audio Cable, Stereo Mix, etc.)

macOS / Linux can build for development; WASAPI loopback notes below apply to Windows only.

## Build & run

```bash
cargo run --release
```

CI builds **Windows x64** and **macOS Apple Silicon** on `main` / PRs (Actions artifacts). Tag `v*` for a GitHub Release.

- **Windows:** `iris_visualizer.exe`
- **macOS:** `Iris Visualizer.app` — if Gatekeeper says damaged: `xattr -cr "Iris Visualizer.app"`, then right-click → Open

### Presets

Built-in presets ship inside the binary. Your saves go to the app config `presets/` folder.

| Preset | Look |
|--------|------|
| `rgb` | Classic RGB bar ring |
| `mono` | Clean grayscale / non-RGB |
| `smooth` | Filled cyan spectrum shape |
| `particles` | RGB soft glow + ambient particles |
| `bar` | Loud-venue tuning + magenta smooth + particles |
| `neon` | High-energy RGB with particles |
| `default` | Baseline |

## Bass pinning

Open **BAND TUNING**:

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
  3. Fullscreen the visualizer on the TV/projector monitor

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

## License

MIT
