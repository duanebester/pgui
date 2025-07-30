#!/bin/bash

# Build the Mac app bundle for PGUI
set -e

APP_NAME="PGUI"
BUNDLE_ID="com.yourname.pgui"
VERSION="1.0.0"
EXECUTABLE_NAME="pgui"

echo "Building release executable..."
cargo build --release

echo "Creating app bundle structure..."

# Create the app bundle directory
APP_DIR="${APP_NAME}.app"
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS"
mkdir -p "$APP_DIR/Contents/Resources"

# Copy the executable
cp "target/release/$EXECUTABLE_NAME" "$APP_DIR/Contents/MacOS/$APP_NAME"

# Make sure it's executable
chmod +x "$APP_DIR/Contents/MacOS/$APP_NAME"

# Create Info.plist
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
</dict>
</plist>
EOF

echo "App bundle created successfully at $APP_DIR"
echo "To create an app icon, run: ./create_app_icon.sh"