# Switchboot

<div style="display:flex;align-items:flex-start;gap:12px">
   <img src="./app-icon.svg" alt="Switchboot app icon" width="64" height="64" />
   <p style="margin:0">Switchboot is a tiny cross-platform tool for managing and switching boot entries with a Svelte + Tauri UI and native helpers packaged for Windows, DEB, and RPM.</p>
</div>

<a id="quick-links"></a>
## Quick links

- [Latest Windows installer](https://github.com/moooozi/switchboot/releases/latest/download/Switchboot_x64-setup.exe)
- [Latest Windows portable](https://github.com/moooozi/switchboot/releases/latest/download/Switchboot_x64-portable.exe)(See [limitations](#portable-limitations))
- [Set up APT repository (Debian/Ubuntu)](#deb)
- [Set up RPM repository (Fedora/OpenSUSE)](#rpm)

## Introduction

This repository contains the Switchboot web UI (Svelte), Tauri native code (`src-tauri/`), and packaging/helpers to produce Windows installers, DEB, and RPM packages. Releases publish packaged artifacts and a small GitHub Pages site that hosts APT/RPM repository metadata and keys.

## Install

### Windows (installer / portable)

See [Quick links](#quick-links) above or visit the [Releases page](https://github.com/moooozi/switchboot/releases/latest):

### Debian / Ubuntu & derivatives (APT)

Add the repository and key, then install:

```bash
# Download the repository keyring into the system keyrings directory
sudo wget -qO /usr/share/keyrings/switchboot-archive-keyring.gpg \
   https://moooozi.github.io/switchboot/deb/switchboot-archive-keyring.gpg

# Add the repository
echo "deb [signed-by=/usr/share/keyrings/switchboot-archive-keyring.gpg] https://moooozi.github.io/switchboot/deb stable main" | sudo tee /etc/apt/sources.list.d/switchboot.list

# Update and install
sudo apt update
sudo apt install -y switchboot
```
Or get the latest `.deb` package from the [Releases page](https://github.com/moooozi/switchboot/releases/latest) (no automatic updates).

### Fedora / OpenSUSE & derivatives (RPM)

```bash
# Add repo
sudo wget -O /etc/yum.repos.d/switchboot.repo https://moooozi.github.io/switchboot/rpm/switchboot.repo

# Install
sudo dnf install switchboot
# or on older systems
sudo yum install switchboot
```
Or get the latest `.rpm` package from the [Releases page](https://github.com/moooozi/switchboot/releases/latest) (no automatic updates).

## How to self-compile

Prerequisites:

- Node.js and pnpm
- Rust toolchain (rustup)

Common commands (from project root):

```bash
# Install deps
pnpm install

# Generate icons
pnpm icons
pnpm tauri icon

# Build native Tauri app
pnpm tauri build

# (optional) On Windows, create portable Windows bundle
pnpm bundle:portable
```

## Repo layout

- `src/` — Svelte frontend
- `src-tauri/` — Rust + Tauri code
- `repo-config/` — static content published to Pages to host APT/RPM repo
- `.github/workflows/` — CI and release pipelines

## License

Please see `LICENSE` in the repository root if present, or contact the project owner for licensing details.

## Support

Open an issue: https://github.com/moooozi/switchboot/issues
