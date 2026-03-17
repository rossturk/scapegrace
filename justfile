default:
    @just --list

# Build all distribution packages
build: dmg appimage

# Run the game
run:
    cargo run --release

# Build macOS .app bundle
app:
    cargo bundle --release
    @echo "Built: target/release/bundle/osx/Scapegrace.app"

# Build macOS .dmg
dmg: app
    hdiutil create -volname Scapegrace \
        -srcfolder target/release/bundle/osx/Scapegrace.app \
        -ov -format UDZO \
        target/Scapegrace-mac.dmg
    @echo "Built: target/Scapegrace-mac.dmg"

# Build Linux AppImage
appimage:
    #!/bin/bash
    set -euo pipefail
    cargo build --release
    APP_DIR="target/Scapegrace.AppDir"
    rm -rf "$APP_DIR"
    mkdir -p "$APP_DIR/usr/bin"
    cp target/release/scapegrace "$APP_DIR/usr/bin/"
    cp appimage/scapegrace.desktop "$APP_DIR/"
    cp icon.png "$APP_DIR/scapegrace.png"
    cat > "$APP_DIR/AppRun" << 'APPRUN'
    #!/bin/bash
    HERE="$(dirname "$(readlink -f "$0")")"
    exec "$HERE/usr/bin/scapegrace" "$@"
    APPRUN
    chmod +x "$APP_DIR/AppRun"
    APPIMAGETOOL="target/appimagetool"
    if [ ! -f "$APPIMAGETOOL" ]; then
        ARCH="$(uname -m)"
        echo "Downloading appimagetool for $ARCH..."
        curl -sSL -o "$APPIMAGETOOL" \
            "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-${ARCH}.AppImage"
        chmod +x "$APPIMAGETOOL"
    fi
    ARCH="$(uname -m)" "$APPIMAGETOOL" "$APP_DIR" "target/Scapegrace-$(uname -m).AppImage"
    echo "Built: target/Scapegrace-$(uname -m).AppImage"

# Install cargo-bundle if missing
setup:
    cargo install cargo-bundle
