use anyhow::{Context, Result, anyhow};
use std::fs;
use std::path::Path;
use std::process::Command;

const APP_NAME: &str = "PGUI";
const BUNDLE_ID: &str = "com.duanebester.pgui";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const EXECUTABLE_NAME: &str = "pgui";
const SVG_FILE: &str = "assets/icons/database-zap.svg";

fn main() -> Result<()> {
    println!("🚀 Building complete Mac app for PGUI...");

    build_release_executable()?;
    create_app_bundle_structure()?;
    create_info_plist()?;
    create_app_icon()?;

    println!("✅ Mac app created successfully!");
    println!("📱 App location: {}.app", APP_NAME);
    println!();
    println!("🎯 Next steps:");
    println!("   1. Double-click {}.app to run your app", APP_NAME);
    println!("   2. Drag {}.app to /Applications to install it", APP_NAME);
    println!("   3. Update the bundle ID if needed: {}", BUNDLE_ID);
    println!();
    println!("🔧 Optional: To sign the app for distribution:");
    println!(
        "   codesign --deep --force --verify --verbose --sign 'Developer ID Application: Your Name' {}.app",
        APP_NAME
    );

    Ok(())
}

fn build_release_executable() -> Result<()> {
    println!("📦 Building release executable...");

    let output = Command::new("cargo")
        .args(&["build", "--release"])
        .output()
        .context("Failed to execute cargo build")?;

    if !output.status.success() {
        return Err(anyhow!(
            "Cargo build failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(())
}

fn create_app_bundle_structure() -> Result<()> {
    println!("🏗️  Creating app bundle structure...");

    let app_dir = format!("{}.app", APP_NAME);

    // Remove existing app bundle
    if Path::new(&app_dir).exists() {
        fs::remove_dir_all(&app_dir).context("Failed to remove existing app bundle")?;
    }

    // Create directory structure
    fs::create_dir_all(format!("{}/Contents/MacOS", app_dir))
        .context("Failed to create MacOS directory")?;
    fs::create_dir_all(format!("{}/Contents/Resources", app_dir))
        .context("Failed to create Resources directory")?;

    // Copy the executable
    let source_executable = format!("target/release/{}", EXECUTABLE_NAME);
    let target_executable = format!("{}/Contents/MacOS/{}", app_dir, APP_NAME);

    fs::copy(&source_executable, &target_executable).context("Failed to copy executable")?;

    // Make executable
    let output = Command::new("chmod")
        .args(&["+x", &target_executable])
        .output()
        .context("Failed to make executable")?;

    if !output.status.success() {
        return Err(anyhow!("Failed to chmod executable"));
    }

    Ok(())
}

fn create_info_plist() -> Result<()> {
    println!("📋 Creating Info.plist...");

    let year = chrono::Utc::now().format("%Y");
    let info_plist_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleDisplayName</key>
    <string>{}</string>
    <key>CFBundleExecutable</key>
    <string>{}</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>CFBundleIdentifier</key>
    <string>{}</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>{}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>{}</string>
    <key>CFBundleVersion</key>
    <string>{}</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.15</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSHumanReadableCopyright</key>
    <string>Copyright © {}</string>
    <key>NSAppTransportSecurity</key>
    <dict>
        <key>NSAllowsArbitraryLoads</key>
        <true/>
    </dict>
</dict>
</plist>"#,
        APP_NAME, APP_NAME, BUNDLE_ID, APP_NAME, VERSION, VERSION, year
    );

    let plist_path = format!("{}.app/Contents/Info.plist", APP_NAME);
    fs::write(&plist_path, info_plist_content).context("Failed to write Info.plist")?;

    Ok(())
}

fn create_app_icon() -> Result<()> {
    println!("🎨 Creating app icon...");

    // Check if rsvg-convert is available
    ensure_rsvg_convert_available()?;

    let iconset_dir = format!("{}.iconset", APP_NAME);

    // Remove existing iconset directory
    if Path::new(&iconset_dir).exists() {
        fs::remove_dir_all(&iconset_dir).context("Failed to remove existing iconset directory")?;
    }

    // Create iconset directory
    fs::create_dir(&iconset_dir).context("Failed to create iconset directory")?;

    // Generate PNG files in different sizes
    let icon_sizes = vec![
        ("icon_16x16.png", 16),
        ("icon_16x16@2x.png", 32),
        ("icon_32x32.png", 32),
        ("icon_32x32@2x.png", 64),
        ("icon_128x128.png", 128),
        ("icon_128x128@2x.png", 256),
        ("icon_256x256.png", 256),
        ("icon_256x256@2x.png", 512),
        ("icon_512x512.png", 512),
        ("icon_512x512@2x.png", 1024),
    ];

    for (filename, size) in icon_sizes {
        println!("Generating {} ({}x{})...", filename, size, size);

        let output_path = format!("{}/{}", iconset_dir, filename);
        let output = Command::new("rsvg-convert")
            .args(&[
                "-w",
                &size.to_string(),
                "-h",
                &size.to_string(),
                SVG_FILE,
                "-o",
                &output_path,
            ])
            .output()
            .context("Failed to run rsvg-convert")?;

        if !output.status.success() {
            return Err(anyhow!(
                "Failed to generate {}: {}",
                filename,
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }

    // Convert to icns file
    println!("Converting to .icns format...");
    let icns_path = format!("{}.app/Contents/Resources/AppIcon.icns", APP_NAME);
    let output = Command::new("iconutil")
        .args(&["-c", "icns", &iconset_dir, "-o", &icns_path])
        .output()
        .context("Failed to run iconutil")?;

    if !output.status.success() {
        return Err(anyhow!(
            "Failed to create .icns file: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    // Clean up iconset directory
    fs::remove_dir_all(&iconset_dir).context("Failed to clean up iconset directory")?;

    Ok(())
}

fn ensure_rsvg_convert_available() -> Result<()> {
    // Check if rsvg-convert is available
    let output = Command::new("which")
        .arg("rsvg-convert")
        .output()
        .context("Failed to check for rsvg-convert")?;

    if !output.status.success() {
        println!("Installing librsvg (required for SVG to PNG conversion)...");

        // Check if brew is available
        let brew_check = Command::new("which")
            .arg("brew")
            .output()
            .context("Failed to check for brew")?;

        if !brew_check.status.success() {
            return Err(anyhow!(
                "❌ Error: brew not found. Please install librsvg manually:\n  brew install librsvg"
            ));
        }

        // Install librsvg using brew
        let install_output = Command::new("brew")
            .args(&["install", "librsvg"])
            .output()
            .context("Failed to install librsvg")?;

        if !install_output.status.success() {
            return Err(anyhow!(
                "Failed to install librsvg: {}",
                String::from_utf8_lossy(&install_output.stderr)
            ));
        }
    }

    Ok(())
}
