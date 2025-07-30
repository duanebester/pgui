#!/bin/bash

# Create Mac app icon from SVG
set -e

APP_NAME="PGUI"
SVG_FILE="assets/icons/database-zap.svg"
ICONSET_DIR="${APP_NAME}.iconset"
APP_DIR="${APP_NAME}.app"

# Check if SVG file exists
if [ ! -f "$SVG_FILE" ]; then
    echo "Error: SVG file not found at $SVG_FILE"
    exit 1
fi

# Check if we have the necessary tools
if ! command -v rsvg-convert >&2 /dev/null; then
    echo "Installing librsvg (required for SVG to PNG conversion)..."
    if command -v brew >&2 /dev/null; then
        brew install librsvg
    else
        echo "Error: brew not found. Please install librsvg manually:"
        echo "  brew install librsvg"
        exit 1
    fi
fi

echo "Creating app icon from $SVG_FILE..."

# Create iconset directory
rm -rf "$ICONSET_DIR"
mkdir "$ICONSET_DIR"

# Generate PNG files in different sizes
declare -A sizes=(
    ["icon_16x16.png"]=16
    ["icon_16x16@2x.png"]=32
    ["icon_32x32.png"]=32
    ["icon_32x32@2x.png"]=64
    ["icon_128x128.png"]=128
    ["icon_128x128@2x.png"]=256
    ["icon_256x256.png"]=256
    ["icon_256x256@2x.png"]=512
    ["icon_512x512.png"]=512
    ["icon_512x512@2x.png"]=1024
)

for filename in "${!sizes[@]}"; do
    size=${sizes[$filename]}
    echo "Generating $filename ($size x $size)..."
    rsvg-convert -w $size -h $size "$SVG_FILE" -o "$ICONSET_DIR/$filename"
done

# Convert to icns file
echo "Converting to .icns format..."
iconutil -c icns "$ICONSET_DIR" -o AppIcon.icns

# Copy to app bundle if it exists
if [ -d "$APP_DIR" ]; then
    cp AppIcon.icns "$APP_DIR/Contents/Resources/"
    echo "Icon copied to app bundle"
fi

# Clean up
rm -rf "$ICONSET_DIR"

echo "App icon created successfully as AppIcon.icns"