# Building PGUI as a Mac App

This guide explains how to create a native Mac app bundle from your PGUI executable.

## Quick Start

Run the complete build script:

```bash
chmod +x build_complete_app.sh
./build_complete_app.sh
```

This will:
1. Build the release executable with `cargo build --release`
2. Create the Mac app bundle structure
3. Generate an app icon from an existing SVG icon
4. Create a complete `PGUI.app` that you can double-click to run

## What Gets Created

The script creates a `PGUI.app` bundle with this structure:

```
PGUI.app/
├── Contents/
│   ├── Info.plist          # App metadata
│   ├── MacOS/
│   │   └── PGUI           # Executable
│   └── Resources/
│       └── AppIcon.icns    # App icon
```

## Using Your App

1. **Run directly**: Double-click `PGUI.app`
2. **Install**: Drag `PGUI.app` to your `/Applications` folder
3. **Dock**: The app will appear in your dock with the database icon

## Prerequisites

The script will automatically install `librsvg` via Homebrew if needed (for SVG to PNG conversion).

If you don't have Homebrew:
```bash
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
```

## Customization

Edit the variables at the top of `build_complete_app.sh`:

- `APP_NAME`: Display name of your app
- `BUNDLE_ID`: Unique identifier (use your own domain)
- `VERSION`: App version number
- `SVG_FILE`: Path to icon SVG file

## Code Signing (Optional)

To sign your app for distribution outside the App Store:

```bash
# Get your Developer ID
security find-identity -v -p codesigning

# Sign the app
codesign --deep --force --verify --verbose --sign "Developer ID Application: Your Name" PGUI.app

# Verify
codesign --verify --verbose PGUI.app
```

## Troubleshooting

**App won't open**: Check Console.app for error messages, or try running the executable directly:
```bash
./PGUI.app/Contents/MacOS/PGUI
```

**Permission denied**: Make sure the executable is marked as executable:
```bash
chmod +x PGUI.app/Contents/MacOS/PGUI
```

**Icon not showing**: Restart Dock to refresh icon cache:
```bash
killall Dock
```
