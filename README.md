# Bina

`bina` is a Rust-based command-line interface (CLI) tool for managing binary installations in the `XDG_BIN_HOME` directory. It leverages the `ubi` crate to download binaries from GitHub repositories and provides a user-friendly interface to check the status of installed binaries, including their versions and the latest available releases.

![License](https://img.shields.io/badge/License-MIT-blue)

## Features

- **Check Binary Status**: Displays a formatted table showing the availability, installed version, and latest version of supported binaries.
- **Download Specific Binaries**: Installs a specified binary using the `ubi` crate.
- **Download Missing Binaries**: Automatically downloads all missing binaries listed in the tool's configuration.
- **Formatted Table Output**: Presents status in a clean, aligned table with color-coded status (green for installed, red for missing) using the `comfy-table` crate.

## Installation

### Prerequisites

- **Rust**: Requires Rust version 1.56 or later. Install via [rustup](https://rustup.rs/):
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```
- **XDG_BIN_HOME**: Set the `XDG_BIN_HOME` environment variable to specify the installation directory for binaries (e.g., `~/.local/bin`).

### Steps

1. **Clone the Repository**:
   ```bash
   git clone https://github.com/gunungpw/bina.git
   cd bina
   ```

2. **Set Environment Variable**:
   Set `XDG_BIN_HOME` to a writable directory and add it to your `PATH`:
   ```bash
   export XDG_BIN_HOME=$HOME/.local/bin
   echo 'export PATH=$XDG_BIN_HOME:$PATH' >> ~/.bashrc
   source ~/.bashrc
   ```

3. **Build the Project**:
   Compile the project using `cargo`:
   ```bash
   cargo build --release
   ```

4. **Install the Binary**:
   Copy the compiled binary to `XDG_BIN_HOME`:
   ```bash
   cp target/release/binary_manager $XDG_BIN_HOME/
   ```

## Usage

Run the CLI with the following subcommands:

### Check Binary Status
Display a table of supported binaries, their installation status, current version, and latest version available on GitHub:
```bash
bina check
```
**Example Output**:
```
┌─────────┬──────────┬─────────┬────────┐
│ Binary  │ Status   │ Version │ Latest │
├─────────┼──────────┼─────────┼────────┤
│ nu      │ Found    │ 0.99.1  │ 0.99.1 │
│ uv      │ Not Found│ -       │ 0.4.0  │
│ zoxide  │ Found    │ 0.9.4   │ 0.9.4  │
│ bun     │ Not Found│ -       │ 1.1.0  │
│ ...     │ ...      │ ...     │ ...    │
└─────────┴──────────┴─────────┴────────┘
```

### Download a Specific Binary
Install a specific binary (e.g., `nu`):
```bash
bina get nu
```

### Download All Missing Binaries
Install all binaries not currently in `XDG_BIN_HOME`:
```bash
bina get-missing
```

## Supported Binaries

The tool supports the following binaries (defined in the `get_data` function in `src/main.rs`):

- `nu` (nushell/nushell)
- `uv` (astral-sh/uv)
- `zoxide` (ajeetdsouza/zoxide)
- `bun` (oven-sh/bun)
- `jj` (jj-vcs/jj)
- `fzf` (junegunn/fzf)
- `ubi` (houseabsolute/ubi)
- `gh` (cli/cli)
- `yazi` (sxyazi/yazi)
- `micro` (zyedidia/micro)
- `lazygit` (jesseduffield/lazygit)

To add support for additional binaries, edit the `get_data` function in `src/main.rs`.

## Troubleshooting

- **GitHub API Rate Limits**: The `check` subcommand makes multiple HTTP requests to the GitHub API. Unauthenticated requests are limited to 60 per hour. If you hit rate limits, consider adding a GitHub token to the `reqwest` headers in `src/main.rs`.
- **XDG_BIN_HOME**: Ensure the directory specified in `XDG_BIN_HOME` is writable and in your `PATH`.

## Contributing

Contributions are welcome! To contribute:

1. Fork the repository.
2. Create a feature branch (`git checkout -b feature/your-feature`).
3. Commit your changes (`git commit -m "Add your feature"`).
4. Push to the branch (`git push origin feature/your-feature`).
5. Open a pull request.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
