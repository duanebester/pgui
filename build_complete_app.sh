#!/bin/bash

# Complete Mac app builder for PGUI
set -e

APP_NAME="PGUI"
BUNDLE_ID="com.yourname.pgui"
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
    rsvg-convert -w $size -h $size "$SVG_FILE" -o "$ICONSET_DIR/$filename"
done

# Convert to icns file
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