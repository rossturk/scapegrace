#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
APP_DIR="$PROJECT_DIR/target/Scapegrace.AppDir"

# Build release binary
cargo build --release

# Clean and create AppDir structure
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/usr/bin"

# Copy binary
cp "$PROJECT_DIR/target/release/scapegrace" "$APP_DIR/usr/bin/"

# Copy desktop file and icon
cp "$SCRIPT_DIR/scapegrace.desktop" "$APP_DIR/"
if [ -f "$PROJECT_DIR/icon.png" ]; then
    cp "$PROJECT_DIR/icon.png" "$APP_DIR/scapegrace.png"
else
    # Generate a placeholder 1x1 icon if none exists
    echo "Warning: no icon.png found at project root, using placeholder"
    printf '\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x02\x00\x00\x00\x90wS\xde\x00\x00\x00\x0cIDATx\x9cc\xf8\x0f\x00\x00\x01\x01\x00\x05\x18\xd8N\x00\x00\x00\x00IEND\xaeB`\x82' > "$APP_DIR/scapegrace.png"
fi

# Create AppRun
cat > "$APP_DIR/AppRun" << 'APPRUN'
#!/bin/bash
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/scapegrace" "$@"
APPRUN
chmod +x "$APP_DIR/AppRun"

# Download appimagetool if not present
APPIMAGETOOL="$PROJECT_DIR/target/appimagetool"
if [ ! -f "$APPIMAGETOOL" ]; then
    ARCH="$(uname -m)"
    echo "Downloading appimagetool for $ARCH..."
    curl -sSL -o "$APPIMAGETOOL" \
        "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-${ARCH}.AppImage"
    chmod +x "$APPIMAGETOOL"
fi

# Build AppImage
ARCH="$(uname -m)" "$APPIMAGETOOL" "$APP_DIR" "$PROJECT_DIR/target/Scapegrace-$(uname -m).AppImage"
echo "Built: target/Scapegrace-$(uname -m).AppImage"
