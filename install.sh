#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# 1. Rust toolchain
if ! command -v cargo >/dev/null 2>&1; then
    echo "Rust toolchain niet gevonden. Installeren via rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
    # shellcheck source=/dev/null
    . "$HOME/.cargo/env"
fi

# 2. Build dependencies (cmake + C++ compiler)
OS="$(uname -s)"
case "$OS" in
    Darwin)
        if ! xcode-select -p >/dev/null 2>&1; then
            echo "Xcode Command Line Tools nodig. Run: xcode-select --install"
            exit 1
        fi
        if ! command -v cmake >/dev/null 2>&1; then
            echo "cmake niet gevonden."
            if command -v brew >/dev/null 2>&1; then
                echo "Installeren via Homebrew..."
                brew install cmake
            else
                echo "Installeer cmake (bv. via https://brew.sh) en run dit script opnieuw."
                exit 1
            fi
        fi
        ;;
    Linux)
        if ! command -v cmake >/dev/null 2>&1; then
            echo "cmake niet gevonden. Installeer 'cmake' en een C++ compiler (g++ of clang++) via je package manager."
            exit 1
        fi
        ;;
esac

# 3. Platform-specific cargo features
case "$OS" in
    Darwin) FEATURES="metal,gui" ;;
    *)      FEATURES="gui" ;;
esac

# 4. Build (CLI + GUI)
echo
echo "Building (eerste keer 5-15 min, daarna seconden)..."
cargo build --release --features "$FEATURES"

# 5. Install both binaries
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
mkdir -p "$INSTALL_DIR"
cp "target/release/transcribe" "$INSTALL_DIR/transcribe"
cp "target/release/transcribe-gui" "$INSTALL_DIR/transcribe-gui"
chmod +x "$INSTALL_DIR/transcribe" "$INSTALL_DIR/transcribe-gui"

# 6. macOS: bundle GUI as a real .app so Finder dubbel-klik direct het venster opent
if [ "$OS" = "Darwin" ]; then
    APPS_DIR="${APPS_DIR:-$HOME/Applications}"
    mkdir -p "$APPS_DIR"
    APP="$APPS_DIR/Transcribe.app"
    rm -rf "$APP"
    mkdir -p "$APP/Contents/MacOS"
    cp "target/release/transcribe-gui" "$APP/Contents/MacOS/transcribe-gui"
    chmod +x "$APP/Contents/MacOS/transcribe-gui"
    cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>transcribe-gui</string>
    <key>CFBundleIdentifier</key>
    <string>local.transcribe</string>
    <key>CFBundleName</key>
    <string>Transcribe</string>
    <key>CFBundleDisplayName</key>
    <string>Transcribe</string>
    <key>CFBundleVersion</key>
    <string>0.1.0</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
PLIST
fi

echo
echo "Geïnstalleerd:"
echo "  $INSTALL_DIR/transcribe       (CLI)"
echo "  $INSTALL_DIR/transcribe-gui   (GUI venster, terminal)"
[ "$OS" = "Darwin" ] && echo "  $APPS_DIR/Transcribe.app      (dubbel-klik vanuit Finder)"
echo

case ":$PATH:" in
    *":$INSTALL_DIR:"*)
        echo "Run vanuit terminal:"
        echo "  transcribe [path]    of    transcribe-gui"
        ;;
    *)
        echo "$INSTALL_DIR staat niet in je PATH. Opties:"
        echo "  1) Voeg permanent toe (zsh):"
        echo "     echo 'export PATH=\"$INSTALL_DIR:\$PATH\"' >> ~/.zshrc && source ~/.zshrc"
        echo "  2) Run direct met volledig pad:"
        echo "     $INSTALL_DIR/transcribe-gui"
        ;;
esac

if [ "$OS" = "Darwin" ]; then
    echo
    echo "Of: dubbel-klik Transcribe.app in $APPS_DIR (Finder)."
fi
