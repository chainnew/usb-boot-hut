#!/bin/bash
set -e

echo "🔨 Building USB Boot Hut..."

# Check for required tools
check_dependency() {
    if ! command -v "$1" &> /dev/null; then
        echo "❌ Error: $1 is required but not installed."
        echo "Please install $1 and try again."
        exit 1
    fi
}

# Check Rust toolchain
check_dependency "cargo"
check_dependency "rustc"

# Platform-specific checks
if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    echo "🐧 Detected Linux platform"
    
    # Check for required Linux tools
    check_dependency "cryptsetup"
    check_dependency "sgdisk"
    check_dependency "grub-install"
    check_dependency "mkfs.ext4"
    check_dependency "mkfs.fat"
    
    # Check cryptsetup version for LUKS2 support
    CRYPTSETUP_VERSION=$(cryptsetup --version | grep -oP '\d+\.\d+')
    REQUIRED_VERSION="2.0"
    
    if [ "$(printf '%s\n' "$REQUIRED_VERSION" "$CRYPTSETUP_VERSION" | sort -V | head -n1)" != "$REQUIRED_VERSION" ]; then
        echo "⚠️  Warning: cryptsetup $CRYPTSETUP_VERSION detected. LUKS2 requires version 2.0+"
    fi
elif [[ "$OSTYPE" == "darwin"* ]]; then
    echo "🍎 Detected macOS platform"
    check_dependency "diskutil"
elif [[ "$OSTYPE" == "msys" || "$OSTYPE" == "cygwin" ]]; then
    echo "🪟 Detected Windows platform"
    echo "Note: Administrative privileges will be required"
fi

# Build the project
echo "📦 Building release binary..."
cargo build --release

# Run tests
echo "🧪 Running tests..."
cargo test

# Check binary size
BINARY_PATH="target/release/usb-boot-hut"
if [[ -f "$BINARY_PATH" ]]; then
    SIZE=$(du -h "$BINARY_PATH" | cut -f1)
    echo "✅ Build complete! Binary size: $SIZE"
    echo "📍 Binary location: $BINARY_PATH"
else
    echo "❌ Build failed: binary not found"
    exit 1
fi

# Installation instructions
echo ""
echo "🚀 Installation:"
echo "  sudo cp $BINARY_PATH /usr/local/bin/"
echo "  sudo chmod +x /usr/local/bin/usb-boot-hut"
echo ""
echo "Or run directly:"
echo "  sudo $BINARY_PATH --help"