#!/usr/bin/env bash
set -euo pipefail

# install-ithmb-magick.sh — Install ImageMagick delegate for .ithmb files
#
# This script installs the ImageMagick delegate XML so that:
#   convert ithmb:photo.ithmb out.png
# works transparently by calling IthmbDecoder internally.
#
# Prerequisites:
#   - ImageMagick 7+ (magick command)
#   - IthmbDecoder CLI in PATH (build with: dotnet publish tools/IthmbDecoder)
#   - .NET 10 runtime (if using dotnet run instead of published binary)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DELEGATE_SRC="$SCRIPT_DIR/ithmb-delegate.xml"

# Detect ImageMagick config directory
if [[ -d "$HOME/.config/ImageMagick" ]]; then
    IM_CONF="$HOME/.config/ImageMagick"
elif [[ -d "/etc/ImageMagick-7" ]]; then
    IM_CONF="/etc/ImageMagick-7"
elif command -v magick &>/dev/null; then
    # Ask ImageMagick where its config lives
    IM_CONF=$(magick -debug configure 2>&1 | grep -oP 'Searching for configure file: \K.*(?=/delegates.xml)' | head -1)
    if [[ -z "$IM_CONF" ]]; then
        echo "Error: Could not detect ImageMagick config directory."
        echo "Set MAGICK_CONFIGURE_PATH manually and re-run."
        exit 1
    fi
else
    echo "Error: ImageMagick not found. Install ImageMagick 7+ first."
    exit 1
fi

# Check for IthmbDecoder
if ! command -v IthmbDecoder &>/dev/null; then
    echo "Warning: IthmbDecoder not found in PATH."
    echo "  Build with: dotnet publish $SCRIPT_DIR/IthmbDecoder -o ~/.local/bin/"
    echo "  Or use: alias IthmbDecoder='dotnet run --project $SCRIPT_DIR/IthmbDecoder --'"
fi

# Install delegate
DST="$IM_CONF/delegates.xml"
if [[ -f "$DST" ]]; then
    # Merge: insert our delegate before the closing </delegatemap>
    if grep -q 'decode="ITHMB"' "$DST"; then
        echo "ITHMB delegate already registered in $DST"
    else
        sed -i 's|</delegatemap>|  <delegate decode="ITHMB" command="IthmbDecoder \&quot;%M\&quot; \&quot;%o\&quot;"/>\n</delegatemap>|' "$DST"
        echo "Appended ITHMB delegate to $DST"
    fi
else
    cp "$DELEGATE_SRC" "$DST"
    echo "Created $DST"
fi

echo ""
echo "Installation complete!"
echo ""
echo "Usage:"
echo "  convert ithmb:photo.ithmb out.png"
echo "  identify ithmb:photo.ithmb"
echo "  convert ithmb:photo.ithmb -resize 200x200 thumb.png"
echo ""
echo "Or to decode all .ithmb files in a directory:"
echo "  for f in *.ithmb; do magick \"ithmb:\$f\" \"\${f%.ithmb}.png\"; done"
