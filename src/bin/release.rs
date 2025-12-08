use anyhow::{Context, Result, anyhow};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn main() -> Result<()> {
    let root = project_root();
    env::set_current_dir(&root).context(format!("Failed to change to project root: {:?}", root))?;

    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    let version = &args[1];

    // Handle special commands
    match version.as_str() {
        "--help" | "-h" => {
            print_usage();
            return Ok(());
        }
        "patch" | "minor" | "major" => {
            let current = get_current_version()?;
            let new_version = bump_version(&current, version)?;
            return do_release(&new_version);
        }
        _ => {}
    }

    // Validate semver format
    if !is_valid_semver(version) {
        return Err(anyhow!(
            "Invalid version format: '{}'\n\
             Expected semver format: MAJOR.MINOR.PATCH (e.g., 0.1.20)\n\
             Or use: patch, minor, major",
            version
        ));
    }

    do_release(version)
}

fn print_usage() {
    let current = get_current_version().unwrap_or_else(|_| "unknown".to_string());
    println!("📦 PGUI Release Tool");
    println!();
    println!("Current version: {}", current);
    println!();
    println!("USAGE:");
    println!("    cargo run --bin release <VERSION>");
    println!();
    println!("ARGS:");
    println!("    <VERSION>    Semantic version (e.g., 0.1.20)");
    println!("                 Or: patch, minor, major");
    println!();
    println!("EXAMPLES:");
    println!("    cargo run --bin release 0.1.20   # Set specific version");
    println!(
        "    cargo run --bin release patch    # {} -> {}",
        current,
        bump_version(&current, "patch").unwrap_or_default()
    );
    println!(
        "    cargo run --bin release minor    # {} -> {}",
        current,
        bump_version(&current, "minor").unwrap_or_default()
    );
    println!(
        "    cargo run --bin release major    # {} -> {}",
        current,
        bump_version(&current, "major").unwrap_or_default()
    );
    println!();
    println!("This will:");
    println!("    1. Update version in Cargo.toml");
    println!("    2. Commit the change");
    println!("    3. Create and push a git tag");
    println!("    4. GitHub Actions will build and create the release");
}

fn is_valid_semver(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3 {
        return false;
    }
    parts.iter().all(|p| p.parse::<u32>().is_ok())
}

fn get_current_version() -> Result<String> {
    let cargo_toml = fs::read_to_string("Cargo.toml").context("Failed to read Cargo.toml")?;

    for line in cargo_toml.lines() {
        if line.starts_with("version = ") {
            let version = line
                .trim_start_matches("version = ")
                .trim_matches('"')
                .to_string();
            return Ok(version);
        }
    }

    Err(anyhow!("Could not find version in Cargo.toml"))
}

fn bump_version(current: &str, bump_type: &str) -> Result<String> {
    let parts: Vec<u32> = current
        .split('.')
        .map(|p| p.parse::<u32>())
        .collect::<Result<Vec<_>, _>>()
        .context("Invalid current version")?;

    if parts.len() != 3 {
        return Err(anyhow!("Invalid current version format"));
    }

    let (major, minor, patch) = (parts[0], parts[1], parts[2]);

    let new_version = match bump_type {
        "major" => format!("{}.0.0", major + 1),
        "minor" => format!("{}.{}.0", major, minor + 1),
        "patch" => format!("{}.{}.{}", major, minor, patch + 1),
        _ => return Err(anyhow!("Invalid bump type")),
    };

    Ok(new_version)
}

fn do_release(version: &str) -> Result<()> {
    let current = get_current_version()?;

    println!("📦 Releasing PGUI v{} (current: v{})", version, current);
    println!();

    // Update Cargo.toml
    println!("📝 Updating Cargo.toml...");
    update_cargo_version(version)?;

    // Update Cargo.lock by running cargo check
    println!("🔒 Updating Cargo.lock...");
    run_command("cargo", &["check", "--quiet"])?;

    // Git operations
    println!("📌 Creating git commit...");
    run_command("git", &["add", "Cargo.toml"])?;
    run_command("git", &["commit", "-m", &format!("Release v{}", version)])?;

    println!("🏷️  Creating git tag v{}...", version);
    run_command("git", &["tag", &format!("v{}", version)])?;

    println!("🚀 Pushing to origin...");
    run_command("git", &["push", "origin", "main"])?;
    run_command("git", &["push", "origin", &format!("v{}", version)])?;

    println!();
    println!("✅ Released v{}!", version);
    println!();
    println!("🔗 Watch the build: https://github.com/duanebester/pgui/actions");
    println!("📦 Release will appear at: https://github.com/duanebester/pgui/releases");

    Ok(())
}

fn update_cargo_version(version: &str) -> Result<()> {
    let cargo_toml = fs::read_to_string("Cargo.toml").context("Failed to read Cargo.toml")?;

    let mut updated = String::new();
    let mut in_package = false;
    let mut version_updated = false;

    for line in cargo_toml.lines() {
        if line.starts_with("[package]") {
            in_package = true;
        } else if line.starts_with('[') {
            in_package = false;
        }

        if in_package && line.starts_with("version = ") && !version_updated {
            updated.push_str(&format!("version = \"{}\"", version));
            version_updated = true;
        } else {
            updated.push_str(line);
        }
        updated.push('\n');
    }

    fs::write("Cargo.toml", updated).context("Failed to write Cargo.toml")?;
    Ok(())
}

fn run_command(cmd: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(cmd)
        .args(args)
        .status()
        .context(format!("Failed to run: {} {:?}", cmd, args))?;

    if !status.success() {
        return Err(anyhow!("Command failed: {} {:?}", cmd, args));
    }

    Ok(())
}
