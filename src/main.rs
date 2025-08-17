use clap::{Parser, Subcommand};
use comfy_table::presets::UTF8_FULL_CONDENSED;
use comfy_table::{Cell, Color, ContentArrangement, Table};
use regex::Regex;
use reqwest::blocking::Client;
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;
use ubi::UbiBuilder;

// CLI structure using clap
#[derive(Parser)]
#[command(name = "binary_manager")]
#[command(about = "Manages binary installations in XDG_BIN_HOME", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Checks availability of binaries in XDG_BIN_HOME
    Check,
    /// Downloads a specified binary using ubi
    Get {
        /// The name of the binary to download
        bin_name: String,
    },
    /// Downloads all missing binaries
    GetMissing,
}

// Define the data mapping for binaries
fn get_data() -> HashMap<&'static str, [&'static str; 3]> {
    let mut data = HashMap::new();
    data.insert("nu", ["nushell/nushell", "nu", "--version"]);
    data.insert("uv", ["astral-sh/uv", "uv", "--version"]);
    data.insert("zoxide", ["ajeetdsouza/zoxide", "zoxide", "--version"]);
    data.insert("bun", ["oven-sh/bun", "bun", "--version"]);
    data.insert("jj", ["jj-vcs/jj", "jj", "--version"]);
    data.insert("fzf", ["junegunn/fzf", "fzf", "--version"]);
    data.insert("ubi", ["houseabsolute/ubi", "ubi", "--version"]);
    data.insert("gh", ["cli/cli", "gh", "--version"]);
    data.insert("yazi", ["sxyazi/yazi", "yazi", "--version"]);
    data.insert("micro", ["zyedidia/micro", "micro", "--version"]);
    data.insert("lazygit", ["jesseduffield/lazygit", "lazygit", "--version"]);
    data
}

// Ensures XDG_BIN_HOME is set and the directory exists
fn ensure_bin_directory() -> bool {
    let bin_directory = env::var("XDG_BIN_HOME").unwrap_or_default();
    if bin_directory.is_empty() {
        println!("Error: XDG_BIN_HOME environment variable is not set");
        return false;
    }

    let path = Path::new(&bin_directory);
    if !path.exists() {
        println!("Creating directory {}...", bin_directory);
        match fs::create_dir_all(path) {
            Ok(_) => true,
            Err(_) => {
                println!("Failed to create directory {}", bin_directory);
                false
            }
        }
    } else {
        true
    }
}

// Checks the latest release version for a GitHub repository
fn check_latest_release(repo: &str) -> String {
    let client = Client::new();
    let url = format!("https://api.github.com/repos/{}/releases/latest", repo);
    match client
        .get(&url)
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "reqwest")
        .send()
    {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<Value>() {
                    Ok(json) => json["tag_name"].as_str().unwrap_or("Error").to_string(),
                    Err(_) => "Error".to_string(),
                }
            } else {
                "Error".to_string()
            }
        }
        Err(_) => "Error".to_string(),
    }
}

// Downloads a specified binary using ubi
async fn binary_get(
    bin_name: &str,
    data: &HashMap<&str, [&str; 3]>,
) -> Result<(), Box<dyn std::error::Error>> {
    if !ensure_bin_directory() {
        return Err("Failed to ensure bin directory".into());
    }

    let bin_data = match data.get(bin_name) {
        Some(data) => data,
        None => {
            println!("Error: Binary '{}' not found in data", bin_name);
            return Err(format!("Binary '{}' not found in data", bin_name).into());
        }
    };
    let xdg_bin_home = env::var("XDG_BIN_HOME").expect("XDG_BIN_HOME not set");

    let mut ubi = UbiBuilder::new()
        .project(bin_data[0])
        .install_dir(&xdg_bin_home)
        .exe(bin_data[1])
        .build()?;

    ubi.install_binary().await?;
    println!("Successfully downloaded {}", bin_name);
    Ok(())
}

// Checks availability of binaries in XDG_BIN_HOME
fn binary_check(data: &HashMap<&str, [&str; 3]>) -> Vec<HashMap<String, String>> {
    if !ensure_bin_directory() {
        return vec![];
    }

    let xdg_bin_home = env::var("XDG_BIN_HOME").expect("XDG_BIN_HOME not set");
    let paths = fs::read_dir(&xdg_bin_home).expect("Failed to read XDG_BIN_HOME");
    let binaries: Vec<String> = paths
        .filter_map(|entry| {
            entry
                .ok()
                .map(|e| e.file_name().into_string().unwrap_or_default())
        })
        .collect();

    let re = Regex::new(r"(\d+\.\d+\.\d+)").expect("Invalid regex");

    let mut results = vec![];
    for (bin_name, bin_data) in data {
        let mut result = HashMap::new();
        result.insert("Binary".to_string(), bin_name.to_string());

        if binaries.contains(&bin_name.to_string()) {
            let output = Command::new(bin_name)
                .arg(bin_data[2])
                .output()
                .expect("Failed to execute binary");
            let version_output = String::from_utf8_lossy(&output.stdout);
            let version = re
                .captures(&version_output)
                .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
                .unwrap_or("-".to_string());
            let latest = tokio::task::block_in_place(|| check_latest_release(bin_data[0]));
            result.insert("Status".to_string(), "Found".to_string());
            result.insert("Version".to_string(), version);
            result.insert("Latest".to_string(), latest);
        } else {
            let latest = tokio::task::block_in_place(|| check_latest_release(bin_data[0]));
            result.insert("Status".to_string(), "Not Found".to_string());
            result.insert("Version".to_string(), "-".to_string());
            result.insert("Latest".to_string(), latest);
        }
        results.push(result);
    }
    results
}

// Downloads missing binaries and returns their status
async fn binary_get_missing(
    data: &HashMap<&str, [&str; 3]>,
) -> Result<String, Box<dyn std::error::Error>> {
    if !ensure_bin_directory() {
        return Ok("".to_string());
    }

    let xdg_bin_home = env::var("XDG_BIN_HOME").expect("XDG_BIN_HOME not set");
    let paths = fs::read_dir(&xdg_bin_home).expect("Failed to read XDG_BIN_HOME");
    let binaries: Vec<String> = paths
        .filter_map(|entry| {
            entry
                .ok()
                .map(|e| e.file_name().into_string().unwrap_or_default())
        })
        .collect();

    let not_found: Vec<&str> = data
        .keys()
        .filter(|&bin_name| !binaries.contains(&bin_name.to_string()))
        .copied()
        .collect();

    if not_found.is_empty() {
        return Ok("All binaries are already present.".to_string());
    }

    for bin_name in not_found {
        println!("Downloading {}...", bin_name);
        binary_get(bin_name, data).await?;
    }

    Ok("".to_string())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let data = get_data();

    match cli.command {
        Commands::Check => {
            let results = binary_check(&data);
            let mut table = Table::new();
            table
                .load_preset(UTF8_FULL_CONDENSED)
                .set_content_arrangement(ContentArrangement::Dynamic)
                .set_header(vec!["Binary", "Status", "Version", "Latest"]);
            for result in results {
                let status = result["Status"].as_str();
                let cell_color = if status.contains("Found") {
                    Color::Green
                } else {
                    Color::Red
                };
                table.add_row(vec![
                    Cell::new(&result["Binary"]),
                    Cell::new(status).fg(cell_color),
                    Cell::new(&result["Version"]),
                    Cell::new(&result["Latest"]),
                ]);
            }
            println!("{}", table);
        }
        Commands::Get { bin_name } => {
            binary_get(&bin_name, &data).await?;
        }
        Commands::GetMissing => {
            let missing_result = binary_get_missing(&data).await?;
            if !missing_result.is_empty() {
                println!("{}", missing_result);
            }
        }
    }
    Ok(())
}
