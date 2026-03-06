# Homebrew Claudelytics

This repository contains the Homebrew formula for [claudelytics](https://github.com/jy1655/claudelytics), a fast CLI tool for analyzing Claude Code usage patterns.

## Installation

```bash
# Add the tap
brew tap jy1655/claudelytics

# Install claudelytics
brew install claudelytics
```

## Usage

After installation, you can use claudelytics directly:

```bash
# Show daily usage report
claudelytics daily

# Launch interactive TUI
claudelytics tui

# Show help
claudelytics --help
```

## Updating

To update to the latest version:

```bash
brew update
brew upgrade claudelytics
```

## Uninstalling

To remove claudelytics:

```bash
brew uninstall claudelytics
brew untap jy1655/claudelytics
```

## Issues

If you encounter any issues with the Homebrew formula, please [open an issue](https://github.com/jy1655/claudelytics/issues) in the main repository.