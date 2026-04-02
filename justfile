default:
    @just --list

# Build all distribution packages
build: dmg appimage

# Run the game
run:
    cargo run --release

# Run the level builder
builder:
    cargo run --bin level_builder

# Run the level builder with dev campaigns (builds Vite frontend first)
builder-dev:
    cd static/level_builder && npm run build
    cargo run --bin level_builder -- -o campaigns_dev.json

# Run the level builder with legacy monolithic HTML (no Vite build)
builder-legacy:
    cargo run --bin level_builder -- -o campaigns_dev.json

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

# Run the new Vite-based level builder (frontend dev server + Rust API)
builder-vite:
    #!/bin/bash
    set -euo pipefail
    cargo run --bin level_builder -- -o campaigns_dev.json &
    RUST_PID=$!
    cd static/level_builder && npx vite &
    VITE_PID=$!
    trap "kill $RUST_PID $VITE_PID 2>/dev/null" EXIT
    wait

# Build the Vite frontend for production
builder-build:
    cd static/level_builder && npm run build

# Install cargo-bundle if missing
setup:
    cargo install cargo-bundle
