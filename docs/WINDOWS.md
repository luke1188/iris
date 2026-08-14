# Windows audio & display notes

Live Visualizer targets **Windows 10/11 x64** for DJ use. This document covers audio capture realities that matter on stage.

## Recommended capture path (DJ / Serato)

1. In Serato (or your DJ software), set the master / record output to **Serato Virtual Audio Cable** (or VB-Audio Cable / Voicemeeter).
2. In Live Visualizer, open **AUDIO → Device** and select that virtual cable.
3. Confirm the input level meter moves with the music.
4. Press **F11** to fullscreen on the TV/projector (monitor selection lands in Phase 7).

This path is reliable and low-latency. Prefer it over microphone pickup.

## Device types you should see

| Device | Typical use |
|--------|-------------|
| Serato Virtual Audio Cable | Serato master → visualizer |
| CABLE Output (VB-Audio) | Generic virtual loop |
| Voicemeeter Output | Mixer / multi-app routing |
| USB audio interface inputs | Hardware mixer / booth feed |
| Microphone / line | Fallback only |
| Stereo Mix / “What U Hear” | Legacy system-output capture (if enabled in Windows sound settings) |

## WASAPI loopback status (Phase 2)

- Enumeration uses **cpal** on the WASAPI host.
- Devices that look like loopback / Stereo Mix / virtual cables are tagged `[loopback]` in the UI when their names match known patterns.
- **True WASAPI process-loopback / render-endpoint capture** (capture the current default speakers without a virtual cable) is **not fully implemented yet**. It will use native Windows APIs in a later phase if needed.
- Until then: use a virtual cable or enable **Stereo Mix** in Windows Sound → Recording → Show Disabled Devices.

## Permissions / exclusive mode

- If a device fails to open, close other apps that hold it in exclusive WASAPI mode.
- Shared mode is preferred so DJ software and the visualizer can coexist.

## Multi-monitor (coming in Phase 7)

Intended layout:

- Laptop = controls / Serato
- HDMI TV/projector = borderless fullscreen visualizer

Phase 2 already supports **F11 fullscreen** on the current window; explicit monitor targeting is next.
