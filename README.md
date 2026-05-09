<p align="center">
  <img src="git_src/assets/EnvStation_Banner.png" alt="EnvStation" width="500" />
  <br/>
  <br/>
  <sub><b>Built with the power of</b></sub><br/>
  <img src="https://img.shields.io/badge/backend-Rust_1.80+-000000?style=for-the-badge&logo=rust&logoColor=white" alt="Rust" />
  <img src="https://img.shields.io/badge/framework-Tauri_V2-024B6A?style=for-the-badge&logo=tauri&logoColor=white" alt="Tauri" />
  <img src="https://img.shields.io/badge/frontend-React_18-0D6EFD?style=for-the-badge&logo=react&logoColor=white" alt="React" />
  <img src="https://img.shields.io/badge/runtime-Podman-076B2A?style=for-the-badge&logo=podman&logoColor=white" alt="Podman" />
  
  <br/>
  <br/>
  <sub><b>Ready-to-use environments for</b></sub><br/>
 <img src="https://img.shields.io/badge/Python-3776AB?style=flat-square&logo=python&logoColor=white" />
  <img src="https://img.shields.io/badge/TS%20%2F%20JS-3178C6?style=flat-square&logo=typescript&logoColor=white" />
  <img src="https://img.shields.io/badge/Rust-black?style=flat-square&logo=rust&logoColor=white" />
  <img src="https://img.shields.io/badge/C%2B%2B-00599C?style=flat-square&logo=c%2B%2B&logoColor=white" />
  <img src="https://img.shields.io/badge/C%23-239120?style=flat-square&logo=c-sharp&logoColor=white" />
  <img src="https://img.shields.io/badge/Java-ED8B00?style=flat-square&logo=openjdk&logoColor=white" />
</p>

# EnvStation

The missing Control Center for reproducible environments using [Distrobox](https://github.com/89luca89/distrobox) and [Devcontainers](https://containers.dev/).

---

## 🚀 Demo

![EnvStation Demo](git_src/assets/final_cut.webp)

*Creating a full Python environment and launching VS Code **in under 90 seconds** on Bazzite — UI remains fully responsive. Waiting time skipped in clip.*

---

## Introduction

**EnvStation** is a lightweight, distro-agnostic desktop application designed to simplify and synchronize development environments on immutable Linux distributions. 

Built with a **Rust** backend and a **React/Tauri** frontend, it seamlessly manages the interplay between **Distrobox** and **DevContainers**. By utilizing a single manifest as the "source of truth," EnvStation ensures your host terminal and IDE environment remain perfectly aligned—all while respecting the architectural constraints of a read-only root filesystem.

For a technical deep dive into the synchronization logic, see [ARCHITECTURE.md](./ARCHITECTURE.md).

---
## Why EnvStation?

Traditional development workflows often break on **immutable distributions** (like Bazzite, Fedora Silverblue, or Arch Linux). EnvStation fixes this by acting as the intelligent bridge between your host terminal and your IDE.

### 1. Unified Environment Parity
On immutable systems, your host is read-only. Usually, your dependencies are trapped inside a DevContainer, leaving your native host terminal (Ptyxis, Alacritty) "dumb."
* **The Solution:** EnvStation mirrors your environment across **Distrobox** (for your native terminal) and **DevContainers** (for your IDE).
* **The Result:** Run `gcc`, `python`, or `git` anywhere—inside or outside of VS Code—without configuring it twice.

### 2. Intelligent Drift Detection
Manual changes in the terminal are inevitable—but they often lead to "snowflake" environments that are impossible to reproduce. EnvStation keeps you in control:
* **Smart Baseline:** Instead of cluttering your view with hundreds of pre-installed system packages, EnvStation snapshots the environment the moment it's created. This ensures you only see the packages *you* actually added.
* **No More Guesswork:** The app detects when your container, your VS Code setup, and your configuration file fall out of sync.
* **One-Click Sync:** If things "drift" apart, EnvStation offers a simple fix: a single click to bring all layers back into perfect alignment.

**The Result:** Your environments stay clean, slim, and—most importantly—reproducible, no matter how much you experiment in the terminal.

### 3. Abstracting the Complexity
You shouldn't need a PhD in Container-Orchestration just to write "Hello World."
* **No Terminal Acrobatics:** Manage Podman and Distrobox through a clean GUI. No more memorizing cryptic flags or complex volume mount syntax.
* **Safe Defaults:** Automatically handles tricky technical hurdles like **Podman GraphRoot relocation**—a common point of failure on immutable systems.
* **Ready in under 2 Minutes:** Choose from pre-configured templates (Python, Rust, C++, etc.) and go from zero to "ready-to-code" in three clicks.

### 4. Sharing Environment Setups
Share only the small .envstation.json (no source or large files) so teammates can recreate your environment quickly.

* **Export a tiny manifest:** Publish the `.envstation.json` to a GitHub Gist and copy the short URL.
* **Recreate in two clicks:** Import a Gist to write the manifest and start creation; if it matches the same name, EnvStation treats it as an update.
* **Privacy-first:** The PAT is stored locally and manifests contain no secrets. Gists are public—share deliberately.

> **EnvStation is the Control Center for your development flow.** It combines the isolation of containers with the comfort of a native OS, ensuring your manifest remains the single source of truth.

---

## Key Technical Features

- Rust backend: fast and memory-safe code for background tasks.
- Rootless operation: uses user-level Podman/Distrobox so no root access is required.
- Single manifest: one .envstation.json controls synchronization between host Distrobox and DevContainer.
- Bidirectional Drift Detection: machine-readable, baseline-driven detection keeps the Distrobox container, the VS Code `devcontainer.json`, and the central manifest in sync; UI surfaces a fallback warning if a conservative query fallback is used.
- Storage helpers: tools to move Podman user storage to another location to save space on constrained disks.

---

## Supported Environments

EnvStation provides scaffolding and DevContainer sync for these ecosystems:

- Python (data science, AI, scripting)
- Node / React (frontend & fullstack)
- Rust (systems)
- Java (backend)
- C / C++ (native & embedded)
- C# (.NET, backend & desktop)

Each environment includes a starter manifest and suggested VS Code extensions. 

---

## Comparison

| Concern | Manual Distrobox / DevContainer Setup | EnvStation |
|---|---:|---|
| Repeatable setup | ad-hoc scripts and manual edits | declarative manifest + scaffolding |
| Security model | varies by setup | rootless Podman with controlled mounts |
| Drift handling | manual reconciliation | manifest-driven sync |
| Disk management | manual moves | guided storage relocation |

---

## Roadmap

- ✅ MVP (done): environment creation, manifest-based sync, storage relocation
- ✅ Drift detection and adoption flows (implemented)
- 🔜 In progress: transactional rollback for sync operations


---

## Motivation: From "Nightmare Setup" to Native Flow

EnvStation was born out of real-world friction. 

Coming from a Frontend background (React/TS), I faced a major hurdle when moving my AI research - local training of OpenCV and ResNet models - to **Bazzite**. In a prior project (Goldgrube Coin Tool) I relied on ResNet models and local toolchains; you can find that repository [here](https://github.com/Kubaguette/goldgrube-coin-tool). As an immutable distribution, the "Windows way" or even the "standard Linux way" of installing toolchains simply didn't work.

I found myself trapped in a **"Nightmare Setup"**:
- ❌ Traditional terminal Python installs failed on the read-only root.
- ❌ Fragmented Conda environments that didn't talk to VS Code properly.
- ❌ High barrier to entry for students and engineers new to immutable OSs.

**The Mission:**
I built this tool to ensure that no developer has to waste hours on environment plumbing again. EnvStation bridges the gap, providing a frictionless, native-feeling UI to manage what used to be a complex, manual process.

---

## Quick Start & Requirements

These steps are intentionally concise. Expand them to match your environment and distribution.

### Prerequisites (common)
- An immutable Linux host (Bazzite / Kinoite / Fedora Silverblue / SteamOS, etc.)
- Podman (rootless) available on the host
- Distrobox installed for comfortable host-container integration
- Node >= 18 and npm or pnpm (for frontend development)
- Rust (stable) + cargo (for backend build)

Verify Podman and Distrobox are available:
```bash
podman --version
distrobox --version
podman info   # ensures Podman can run for your user
# If you use the Podman API/socket with other tools, enable it:
systemctl --user enable --now podman.socket
```

---

### Installation (End-User)
EnvStation is currently distributed as native packages (.deb and .rpm). Choose the option that fits your distro and policy.

#### Debian / Ubuntu (.deb)
```bash
# Newer apt supports local .deb install:
sudo apt update
sudo apt install ./EnvStation_1.0.0_amd64.deb

# Or with dpkg + fix dependencies:
sudo dpkg -i EnvStation_1.0.0_amd64.deb
sudo apt-get install -f
```

#### Fedora / RHEL Immutable Hosts (Silverblue / Bazzite / Kinoite)
On OSTree-based immutable systems you cannot use `sudo dnf` to install host packages. Use rpm-ostree and reboot to apply the deployment:
```bash
# Install to the immutable OSTree deployment (example):
sudo rpm-ostree install ./EnvStation-1.0.0.x86_64.rpm
# A system reboot is required to apply the new deployment
sudo systemctl reboot
```
Notes:
- rpm-ostree performs an atomic OSTree deployment. The package is staged and becomes active after reboot.
- If you prefer not to reboot or modify the host image, use the AppImage instead.

#### Fedora / RHEL (traditional, mutable systems)
If you are on a mutable Fedora/RHEL workstation (not OSTree-based), you can install with dnf:
```bash
sudo dnf install ./EnvStation-1.0.0.x86_64.rpm
```

#### Arch Linux (pkg.tar.zst or AUR)
Arch users typically install using a package in pacman format or from the AUR if available:
```bash
# If you have a built pacman package:
sudo pacman -U ./envstation-1.0.0-1-x86_64.pkg.tar.zst

# Or install from AUR using an AUR helper (if package published):
paru -S envstation   # or yay -S envstation
```
If no Arch package is available, prefer the AppImage or build from source.

#### AppImage

Comming soon.

---

#### Notes:
- Make sure Podman and Distrobox are installed and working before running EnvStation. EnvStation expects rootless Podman for normal operation.
- On OSTree-based systems (Silverblue / Bazzite / Kinoite) prefer the AppImage or use rpm-ostree and reboot to install host packages.
- For RHEL / CentOS you may prefer to use EPEL or the distro's packaging tools to get Podman and its dependencies.

---

### Installing Podman & Distrobox (examples, skip if installed)

#### Fedora / RHEL:
```bash
sudo dnf install -y podman distrobox
systemctl --user enable --now podman.socket
```

#### Ubuntu / Debian (example: 22.04+):
```bash
sudo apt update
sudo apt install -y podman distrobox
# If distrobox isn't available on your Ubuntu version, see:
# https://github.com/89luca89/distrobox or the upstream distrobox README
```

#### Arch:
```bash
sudo pacman -Syu podman distrobox
systemctl --user enable --now podman.socket
```

If a distribution does not ship distrobox or a recent Podman, follow the official upstream instructions:
- Podman: https://podman.io/getting-started/installation
- Distrobox: https://github.com/89luca89/distrobox

---

### Installation (developer mode)

Development should run inside a mutable container (distrobox/toolbox) because the host OS is immutable. Example:

```bash
# create or enter a mutable development container
# distrobox create --name devbox --image registry.fedoraproject.org/fedora-toolbox:latest --yes
distrobox enter devbox
```

1. Clone and install JS deps
```bash
git clone https://github.com/Kubaguette/envstation.git
cd envstation
npm install
```

2. Install native dev dependencies (common packages vary by distro)

#### Fedora / Bazzite / Kinoite (dnf)
```bash
sudo dnf install -y webkit2gtk4.1-devel libappindicator-gtk3-devel librsvg2-devel gtk3-devel gcc gcc-c++ make xdg-utils fuse
```

#### Ubuntu / Debian (apt)
```bash
sudo apt update
sudo apt install -y libwebkit2gtk-4.0-dev libappindicator3-dev librsvg2-dev libgtk-3-dev build-essential xdg-utils fuse
# package names can vary by Ubuntu version; if a package is missing, search with apt-cache search
```

#### Arch (pacman)
```bash
sudo pacman -Syu webkit2gtk libappindicator-gtk3 librsvg gtk3 base-devel xdg-utils fuse
# Arch package names may differ slightly; use pacman -Ss to confirm exact names
```

3. Start the unified dev workflow
```bash
npm run tauri dev
```

Notes:
- `npm run tauri dev` compiles the Rust backend and starts the Vite dev server together. You may need PKG_CONFIG and other env vars for WebKit bindings.
- If you run into missing headers or pkg-config errors, double-check the dev packages above for your distro and install pkg-config if required.

---
## Contributing

We welcome contributions that respect the project's architecture and testing boundaries. Please consult ARCHITECTURE.md before making large structural changes; follow the Core → Commands → View separation and prefer small, reviewable PRs.

---

## Author

**Kubaguette**
*Frontend Engineer | Exploring Rust & Linux Systems*

- 🐙 [GitHub Profile](https://github.com/Kubaguette)
- 💰 Inspiration: [Goldgrube Coin Tool](https://github.com/Kubaguette/goldgrube-coin-tool)

---

## License

EnvStation is distributed under the GNU General Public License v3.0. See LICENSE for details.

---

Built by developers, for the Bazzite and whole Linux community. Aiming for native, frictionless engineering.
