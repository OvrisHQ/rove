#!/bin/sh
# Rove installer script
# Usage: curl -fsSL https://raw.githubusercontent.com/OvrisHQ/rove/main/scripts/install.sh | sh

set -e

REPO="OvrisHQ/rove"
BINARY="rove"
INSTALL_DIR="/usr/local/bin"

main() {
    # Detect OS
    OS="$(uname -s)"
    case "$OS" in
        Linux)  OS_TARGET="unknown-linux-gnu" ;;
        Darwin) OS_TARGET="apple-darwin" ;;
        *)
            echo "Error: Unsupported OS: $OS"
            exit 1
            ;;
    esac

    # Detect architecture
    ARCH="$(uname -m)"
    case "$ARCH" in
        x86_64|amd64)  ARCH_TARGET="x86_64" ;;
        aarch64|arm64) ARCH_TARGET="aarch64" ;;
        *)
            echo "Error: Unsupported architecture: $ARCH"
            exit 1
            ;;
    esac

    TARGET="${ARCH_TARGET}-${OS_TARGET}"
    ASSET_NAME="${BINARY}-${TARGET}"

    echo "Rove Installer"
    echo "=============="
    echo "  OS:     $OS ($OS_TARGET)"
    echo "  Arch:   $ARCH ($ARCH_TARGET)"
    echo "  Target: $TARGET"
    echo ""

    # Fetch latest release tag
    echo "Fetching latest release..."
    LATEST=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/')

    if [ -z "$LATEST" ]; then
        echo "Error: Could not determine latest release"
        exit 1
    fi

    echo "  Latest version: $LATEST"
    echo ""

    # Download binary
    DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${LATEST}/${ASSET_NAME}"
    TEMP_FILE="$(mktemp)"

    echo "Downloading ${ASSET_NAME}..."
    if ! curl -fSL --progress-bar "$DOWNLOAD_URL" -o "$TEMP_FILE"; then
        rm -f "$TEMP_FILE"
        echo ""
        echo "Error: Failed to download $DOWNLOAD_URL"
        echo "Make sure a release exists with asset: $ASSET_NAME"
        exit 1
    fi

    chmod +x "$TEMP_FILE"

    # Install
    if [ -w "$INSTALL_DIR" ]; then
        mv "$TEMP_FILE" "${INSTALL_DIR}/${BINARY}"
        echo ""
        echo "Installed to ${INSTALL_DIR}/${BINARY}"
    elif command -v sudo >/dev/null 2>&1; then
        echo ""
        echo "Installing to ${INSTALL_DIR}/${BINARY} (requires sudo)..."
        sudo mv "$TEMP_FILE" "${INSTALL_DIR}/${BINARY}"
        echo "Installed to ${INSTALL_DIR}/${BINARY}"
    else
        # Fallback to ~/.local/bin
        INSTALL_DIR="${HOME}/.local/bin"
        mkdir -p "$INSTALL_DIR"
        mv "$TEMP_FILE" "${INSTALL_DIR}/${BINARY}"
        echo ""
        echo "Installed to ${INSTALL_DIR}/${BINARY}"
        echo "Make sure ${INSTALL_DIR} is in your PATH"
    fi

    echo ""
    echo "Run 'rove setup' to configure."
    echo "Run 'rove doctor' to verify installation."
}

main
