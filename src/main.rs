use clap::{Parser, Subcommand};
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::fs::{self, DirEntry};
use std::path::Path;
use std::process::Command;
use ubi::UbiBuilder;

#[derive(Parser)]
#[command(
    name = "bina",
    about = "Manages binary installations in XDG_BIN_HOME",
    version = "0.1.0"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Checks availability of binaries in XDG_BIN_HOME
    Check {
        /// Check the latest release version from GitHub
        #[arg(long)]
        latest: bool,
    },
    /// Downloads a specified binary using ubi
    Get {
        /// The name of the binary to download
        bin_name: String,
    },
    /// Downloads all missing binaries
    GetMissing,
}

#[derive(Debug, Serialize, Deserialize)]
struct Binary {
    name: String,
    repo: String,
    exe: String,
    version_arg: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    binaries: Vec<Binary>,
}

struct BinManager {
    data: HashMap<String, [String; 3]>,
    xdg_bin_home: String,
    regex: Regex,
}

fn new_bin_manager() -> Result<BinManager, Box<dyn std::error::Error>> {
    let data = load_config()?;
    let xdg_bin_home =
        env::var("XDG_BIN_HOME").map_err(|_| "XDG_BIN_HOME environment variable not set")?;
    let regex = Regex::new(r"(\d+\.\d+\.\d+)").map_err(|_| "Invalid regex")?;
    Ok(BinManager {
        data,
        xdg_bin_home,
        regex,
    })
}

fn load_config() -> Result<HashMap<String, [String; 3]>, Box<dyn std::error::Error>> {
    let config_dir = env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
        let home = env::var("HOME").expect("HOME environment variable not set");
        format!("{}/.config", home)
    });
    let toml_path = format!("{}/bina/binaries.toml", config_dir);
    let toml_str = fs::read_to_string(&toml_path)
        .map_err(|_| format!("Failed to read binaries.toml from {}", toml_path))?;
    let config: Config = toml::from_str(&toml_str)
        .map_err(|_| format!("Failed to parse binaries.toml from {}", toml_path))?;

    let mut data = HashMap::new();
    for binary in config.binaries {
        data.insert(
            binary.name.clone(),
            [binary.repo, binary.exe, binary.version_arg],
        );
    }
    Ok(data)
}

fn ensure_bin_directory(xdg_bin_home: &str) -> Result<(), Box<dyn std::error::Error>> {
    if xdg_bin_home.is_empty() {
        return Err("XDG_BIN_HOME environment variable is not set".into());
    }

    let path = Path::new(xdg_bin_home);
    if !path.exists() {
        println!("Creating directory {}...", xdg_bin_home);
        fs::create_dir_all(path)?;
    }
    Ok(())
}

async fn check_latest_release(repo: &str) -> String {
    let client = Client::new();
    let url = format!("https://api.github.com/repos/{}/releases/latest", repo);
    let response = client
        .get(&url)
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "reqwest")
        .send()
        .await;

    match response {
        Ok(response) if response.status().is_success() => match response.json::<Value>().await {
            Ok(json) => json["tag_name"]
                .as_str()
                .map(String::from)
                .unwrap_or("Error".to_string()),
            Err(_) => "Error".to_string(),
        },
        _ => "Error".to_string(),
    }
}

async fn get_binary(
    bin_name: &str,
    manager: &BinManager,
) -> Result<(), Box<dyn std::error::Error>> {
    ensure_bin_directory(&manager.xdg_bin_home)?;
    let bin_data = manager
        .data
        .get(bin_name)
        .ok_or_else(|| format!("Binary '{}' not found in data", bin_name))?;

    let mut ubi = UbiBuilder::new()
        .project(&bin_data[0])
        .install_dir(&manager.xdg_bin_home)
        .exe(&bin_data[1])
        .build()?;
    ubi.install_binary().await?;
    println!("Successfully downloaded {}", bin_name);
    Ok(())
}

async fn check_binaries(manager: &BinManager, check_latest: bool) -> Vec<HashMap<String, String>> {
    if ensure_bin_directory(&manager.xdg_bin_home).is_err() {
        return vec![];
    }

    let binaries: Vec<String> = fs::read_dir(&manager.xdg_bin_home)
        .expect("Failed to read XDG_BIN_HOME")
        .filter_map(|entry: Result<DirEntry, _>| {
            entry.ok().and_then(|e| e.file_name().into_string().ok())
        })
        .collect();

    let mut results = vec![];
    for (bin_name, bin_data) in &manager.data {
        let mut result = HashMap::new();
        result.insert("Binary".to_string(), bin_name.to_string());

        if binaries.contains(bin_name) {
            let version = Command::new(bin_name)
                .arg(&bin_data[2])
                .output()
                .map(|output| String::from_utf8_lossy(&output.stdout).to_string())
                .map(|version_output| {
                    manager
                        .regex
                        .captures(&version_output)
                        .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
                        .unwrap_or("-".to_string())
                })
                .unwrap_or("-".to_string());
            result.insert("Status".to_string(), "✓".to_string());
            result.insert("Version".to_string(), version);
            if check_latest {
                let latest = check_latest_release(&bin_data[0]).await;
                let latest_version = manager
                    .regex
                    .captures(&latest)
                    .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
                    .unwrap_or("-".to_string());
                result.insert("Latest".to_string(), latest_version);
            }
        } else {
            result.insert("Status".to_string(), "✗".to_string());
            result.insert("Version".to_string(), "-".to_string());
            if check_latest {
                let latest = check_latest_release(&bin_data[0]).await;
                let latest_version = manager
                    .regex
                    .captures(&latest)
                    .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
                    .unwrap_or("-".to_string());
                result.insert("Latest".to_string(), latest_version);
            }
        }
        results.push(result);
    }
    results
}

async fn get_missing_binaries(manager: &BinManager) -> Result<String, Box<dyn std::error::Error>> {
    ensure_bin_directory(&manager.xdg_bin_home)?;
    let binaries: Vec<String> = fs::read_dir(&manager.xdg_bin_home)
        .expect("Failed to read XDG_BIN_HOME")
        .filter_map(|entry: Result<DirEntry, _>| {
            entry.ok().and_then(|e| e.file_name().into_string().ok())
        })
        .collect();

    let not_found: Vec<String> = manager
        .data
        .keys()
        .filter(|bin_name| !binaries.contains(bin_name))
        .cloned()
        .collect();

    if not_found.is_empty() {
        return Ok("All binaries are already present.".to_string());
    }

    for bin_name in not_found {
        println!("Downloading {}...", bin_name);
        get_binary(&bin_name, manager).await?;
    }
    Ok("".to_string())
}

fn print_results(results: Vec<HashMap<String, String>>, check_latest: bool) {
    const WIDTHS: [usize; 4] = [15, 10, 15, 15];

    if check_latest {
        println!(
            "{:<width1$}{:<width2$}{:<width3$}{:<width4$}",
            "BINARY",
            "STATUS",
            "VERSION",
            "LATEST",
            width1 = WIDTHS[0],
            width2 = WIDTHS[1],
            width3 = WIDTHS[2],
            width4 = WIDTHS[3]
        );
    } else {
        println!(
            "{:<width1$}{:<width2$}{:<width3$}",
            "BINARY",
            "STATUS",
            "VERSION",
            width1 = WIDTHS[0],
            width2 = WIDTHS[1],
            width3 = WIDTHS[2]
        );
    }

    for result in results {
        if check_latest {
            println!(
                "{:<width1$}{:<width2$}{:<width3$}{:<width4$}",
                result["Binary"],
                result["Status"],
                result["Version"],
                result.get("Latest").unwrap_or(&"-".to_string()),
                width1 = WIDTHS[0],
                width2 = WIDTHS[1],
                width3 = WIDTHS[2],
                width4 = WIDTHS[3]
            );
        } else {
            println!(
                "{:<width1$}{:<width2$}{:<width3$}",
                result["Binary"],
                result["Status"],
                result["Version"],
                width1 = WIDTHS[0],
                width2 = WIDTHS[1],
                width3 = WIDTHS[2]
            );
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let manager = new_bin_manager()?;

    match cli.command {
        Some(Commands::Check { latest }) => {
            let results = check_binaries(&manager, latest).await;
            print_results(results, latest);
        }
        Some(Commands::Get { bin_name }) => {
            get_binary(&bin_name, &manager).await?;
        }
        Some(Commands::GetMissing) => {
            let result = get_missing_binaries(&manager).await?;
            if !result.is_empty() {
                println!("{}", result);
            }
        }
        None => {
            Cli::parse_from(&["bina", "--help"]);
        }
    }
    Ok(())
}
