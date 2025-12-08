use anyhow::{Context, Result, anyhow};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};

const APP_NAME: &str = "PGUI";
const BUNDLE_ID: &str = "com.duanebester.pgui";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const EXECUTABLE_NAME: &str = "pgui";
const PRE_GENERATED_ICNS: &str = "assets/icons/AppIcon.icns";
const SVG_FILE: &str = "assets/icons/db-spark.svg";

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn main() -> Result<()> {
    // Change to project root so all paths are consistent
    let root = project_root();
    env::set_current_dir(&root).context(format!("Failed to change to project root: {:?}", root))?;

    println!("🚀 Building complete Mac app for PGUI v{}...", VERSION);

    build_release_executable()?;
    create_app_bundle_structure()?;
    create_info_plist()?;
    setup_app_icon()?;

    println!("✅ Mac app created successfully!");
    println!("📱 App location: {}.app", APP_NAME);
    println!();
    println!("🎯 Next steps:");
    println!("   1. Double-click {}.app to run your app", APP_NAME);
    println!("   2. Drag {}.app to /Applications to install it", APP_NAME);

    Ok(())
}

fn build_release_executable() -> Result<()> {
    println!("📦 Building release executable...");

    // Use status() instead of output() to stream the build output
    let status = Command::new("cargo")
        .args(["build", "--release"])
        .status() // This streams output to terminal
        .context("Failed to execute cargo build")?;

    if !status.success() {
        return Err(anyhow!("Cargo build failed"));
    }

    // Verify the executable exists
    let exe_path = format!("target/release/{}", EXECUTABLE_NAME);
    if !Path::new(&exe_path).exists() {
        return Err(anyhow!("Release executable not found at {}", exe_path));
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
        .args(["+x", &target_executable])
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
    <string>11.0</string>
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

fn setup_app_icon() -> Result<()> {
    let icns_dest = format!("{}.app/Contents/Resources/AppIcon.icns", APP_NAME);

    // Check if pre-generated .icns exists (preferred for CI)
    if Path::new(PRE_GENERATED_ICNS).exists() {
        println!("🎨 Using pre-generated app icon...");
        fs::copy(PRE_GENERATED_ICNS, &icns_dest).context("Failed to copy pre-generated icon")?;
        return Ok(());
    }

    // Fall back to generating from SVG (for local dev)
    println!("🎨 Generating app icon from SVG (no pre-generated icon found)...");
    create_app_icon_from_svg(&icns_dest)
}

fn create_app_icon_from_svg(icns_dest: &str) -> Result<()> {
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
        let output_path = format!("{}/{}", iconset_dir, filename);
        let output = Command::new("rsvg-convert")
            .args([
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
    let output = Command::new("iconutil")
        .args(["-c", "icns", &iconset_dir, "-o", icns_dest])
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
    let output = Command::new("which")
        .arg("rsvg-convert")
        .output()
        .context("Failed to check for rsvg-convert")?;

    if !output.status.success() {
        return Err(anyhow!(
            "rsvg-convert not found. Install with: brew install librsvg\n\
             Or commit a pre-generated assets/icons/AppIcon.icns file."
        ));
    }

    Ok(())
}
