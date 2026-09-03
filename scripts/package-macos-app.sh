#!/usr/bin/env bash
# Build a double-clickable macOS .app from a release binary.
# Usage: package-macos-app.sh <path-to-binary> <output-dir> [app-name]
set -euo pipefail

BIN="${1:?binary path}"
OUT_DIR="${2:?output dir}"
APP_NAME="${3:-Iris Visualizer}"
BUNDLE_ID="${BUNDLE_ID:-app.iris.visualizer}"
VERSION="${VERSION:-0.1.0}"

if [[ ! -f "$BIN" ]]; then
  echo "Missing binary: $BIN" >&2
  exit 1
fi

APP="$OUT_DIR/${APP_NAME}.app"
CONTENTS="$APP/Contents"
MACOS="$CONTENTS/MacOS"
RESOURCES="$CONTENTS/Resources"

rm -rf "$APP"
mkdir -p "$MACOS" "$RESOURCES"

# Executable must keep +x or Finder treats it like a document.
cp "$BIN" "$MACOS/iris_visualizer"
chmod 755 "$MACOS/iris_visualizer"

# Presets / assets next to the binary (app looks beside current_exe).
if [[ -d presets ]]; then
  cp -R presets "$MACOS/presets"
fi
if [[ -d assets ]]; then
  cp -R assets "$MACOS/assets"
  cp -R assets "$RESOURCES/assets" 2>/dev/null || true
fi
if [[ -f README.md ]]; then
  cp README.md "$RESOURCES/"
fi
if [[ -f docs/MACOS.md ]]; then
  mkdir -p "$RESOURCES/docs"
  cp docs/MACOS.md "$RESOURCES/docs/"
fi

# Optional .icns from PNG (best-effort; app still works without it).
if [[ -f assets/icon.png ]] && command -v sips >/dev/null && command -v iconutil >/dev/null; then
  ICONSET="$(mktemp -d)/AppIcon.iconset"
  mkdir -p "$ICONSET"
  for size in 16 32 128 256 512; do
    sips -z "$size" "$size" assets/icon.png --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
    sips -z $((size * 2)) $((size * 2)) assets/icon.png --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
  done
  if iconutil -c icns "$ICONSET" -o "$RESOURCES/AppIcon.icns" 2>/dev/null; then
    ICON_KEY=$'\n  <key>CFBundleIconFile</key>\n  <string>AppIcon</string>'
  else
    ICON_KEY=""
  fi
  rm -rf "$(dirname "$ICONSET")"
else
  ICON_KEY=""
fi

cat > "$CONTENTS/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleExecutable</key>
  <string>iris_visualizer</string>
  <key>CFBundleIdentifier</key>
  <string>${BUNDLE_ID}</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>${APP_NAME}</string>
  <key>CFBundleDisplayName</key>
  <string>${APP_NAME}</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>${VERSION}</string>
  <key>CFBundleVersion</key>
  <string>${VERSION}</string>
  <key>LSMinimumSystemVersion</key>
  <string>12.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
  <key>NSMicrophoneUsageDescription</key>
  <string>Iris Visualizer needs access to an audio input device for the live spectrum.</string>${ICON_KEY}
</dict>
</plist>
PLIST

# Clear any stale quarantine on the package we just built (local runs).
xattr -cr "$APP" 2>/dev/null || true

echo "Created $APP"
ls -la "$MACOS"
file "$MACOS/iris_visualizer"
