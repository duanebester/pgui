#!/bin/bash

# Complete Mac app builder for PGUI
set -e

APP_NAME="PGUI"
BUNDLE_ID="com.duanebester.pgui"
VERSION="1.0.0"
EXECUTABLE_NAME="pgui"
SVG_FILE="assets/icons/database-zap.svg"

echo "🚀 Building complete Mac app for PGUI..."

# Step 1: Build the release executable
echo "📦 Building release executable..."
cargo build --release

# Step 2: Create app bundle structure
echo "🏗️  Creating app bundle structure..."
APP_DIR="${APP_NAME}.app"
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS"
mkdir -p "$APP_DIR/Contents/Resources"

# Copy the executable
cp "target/release/$EXECUTABLE_NAME" "$APP_DIR/Contents/MacOS/$APP_NAME"
chmod +x "$APP_DIR/Contents/MacOS/$APP_NAME"

# Step 3: Create Info.plist
echo "📋 Creating Info.plist..."
cat > "$APP_DIR/Contents/Info.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleDisplayName</key>
    <string>$APP_NAME</string>
    <key>CFBundleExecutable</key>
    <string>$APP_NAME</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>CFBundleIdentifier</key>
    <string>$BUNDLE_ID</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>$APP_NAME</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>$VERSION</string>
    <key>CFBundleVersion</key>
    <string>$VERSION</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.15</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSHumanReadableCopyright</key>
    <string>Copyright © $(date +%Y)</string>
    <key>NSAppTransportSecurity</key>
    <dict>
        <key>NSAllowsArbitraryLoads</key>
        <true/>
    </dict>
</dict>
</plist>
EOF

# Step 4: Create app icon
echo "🎨 Creating app icon..."

# Check if we have the necessary tools
if ! command -v rsvg-convert >&2 /dev/null; then
    echo "Installing librsvg (required for SVG to PNG conversion)..."
    if command -v brew >&2 /dev/null; then
        brew install librsvg
    else
        echo "❌ Error: brew not found. Please install librsvg manually:"
        echo "  brew install librsvg"
        exit 1
    fi
fi

# Create iconset directory
ICONSET_DIR="${APP_NAME}.iconset"
rm -rf "$ICONSET_DIR"
mkdir "$ICONSET_DIR"

# Generate PNG files in different sizes - using a simpler approach
echo "Generating icon_16x16.png (16x16)..."
rsvg-convert -w 16 -h 16 "$SVG_FILE" -o "$ICONSET_DIR/icon_16x16.png"

echo "Generating icon_16x16@2x.png (32x32)..."
rsvg-convert -w 32 -h 32 "$SVG_FILE" -o "$ICONSET_DIR/icon_16x16@2x.png"

echo "Generating icon_32x32.png (32x32)..."
rsvg-convert -w 32 -h 32 "$SVG_FILE" -o "$ICONSET_DIR/icon_32x32.png"

echo "Generating icon_32x32@2x.png (64x64)..."
rsvg-convert -w 64 -h 64 "$SVG_FILE" -o "$ICONSET_DIR/icon_32x32@2x.png"

echo "Generating icon_128x128.png (128x128)..."
rsvg-convert -w 128 -h 128 "$SVG_FILE" -o "$ICONSET_DIR/icon_128x128.png"

echo "Generating icon_128x128@2x.png (256x256)..."
rsvg-convert -w 256 -h 256 "$SVG_FILE" -o "$ICONSET_DIR/icon_128x128@2x.png"

echo "Generating icon_256x256.png (256x256)..."
rsvg-convert -w 256 -h 256 "$SVG_FILE" -o "$ICONSET_DIR/icon_256x256.png"

echo "Generating icon_256x256@2x.png (512x512)..."
rsvg-convert -w 512 -h 512 "$SVG_FILE" -o "$ICONSET_DIR/icon_256x256@2x.png"

echo "Generating icon_512x512.png (512x512)..."
rsvg-convert -w 512 -h 512 "$SVG_FILE" -o "$ICONSET_DIR/icon_512x512.png"

echo "Generating icon_512x512@2x.png (1024x1024)..."
rsvg-convert -w 1024 -h 1024 "$SVG_FILE" -o "$ICONSET_DIR/icon_512x512@2x.png"

# Convert to icns file
echo "Converting to .icns format..."
iconutil -c icns "$ICONSET_DIR" -o "$APP_DIR/Contents/Resources/AppIcon.icns"

# Clean up
rm -rf "$ICONSET_DIR"

echo "✅ Mac app created successfully!"
echo "📱 App location: $APP_DIR"
echo ""
echo "🎯 Next steps:"
echo "   1. Double-click $APP_DIR to run your app"
echo "   2. Drag $APP_DIR to /Applications to install it"
echo "   3. Update the bundle ID in the script if needed: $BUNDLE_ID"
echo ""
echo "🔧 Optional: To sign the app for distribution:"
echo "   codesign --deep --force --verify --verbose --sign 'Developer ID Application: Your Name' $APP_DIR"
