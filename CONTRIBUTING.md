# CONTRIBUTING.md

Welcome — thanks for wanting to help make EnvStation better! This short guide shows the easiest way to get started and how to submit changes.

## Quick setup (Distrobox / Bazzite)

1. Create or enter a mutable development container (we use Distrobox as an example):

```bash CONTRIBUTING.md
# Example: create/enter a Distrobox container
# distrobox-create --name devbox --image registry.fedoraproject.org/fedora-toolbox:latest --yes
distrobox enter devbox
```

2. Clone the repo and install JS dependencies:

```bash CONTRIBUTING.md
git clone https://github.com/Kubaguette/envstation.git
cd envstation
npm install
```

3. Install native build packages (Fedora / Bazzite example):

```bash CONTRIBUTING.md
# Required for WebKit/GTK and native builds
sudo dnf install -y webkit2gtk4.1-devel libappindicator-gtk3-devel librsvg2-devel gtk3-devel gcc gcc-c++ make
```

(Install these packages inside your container or ensure they are available from the host.)

4. Start the development loop:

```bash CONTRIBUTING.md
npm run tauri dev
```

Optional: if you need to manage Rust manually:

```bash CONTRIBUTING.md
rustup default stable
cargo build
cargo test
```

## Architecture (short)

We follow a simple separation: Core -> Commands -> View. Put core business logic in the Rust core crate, keep the command/IPC layer thin, and keep UI code in the view. See ARCHITECTURE.md for details.

A helpful tip about host commands (the golden rule)

Because development often runs inside a container, let the project handle executing host tools for you. Use the shared host executor (build_host_command_async) instead of spawning host commands directly. This helps commands work reliably both inside and outside containers.

## How to contribute

- Fork the repo, create a branch, make changes, and test them locally.
- Keep PRs small and focused.
- In your PR description, explain what you changed and why (include steps to reproduce or test).
- Submit the PR from your fork when ready.

## Code quality

- Run formatting and linters before opening a PR:
  - Rust: cargo fmt, cargo clippy
  - JS/TS: npm run lint, npm run format
- Add tests where appropriate (unit tests in the core for Rust business logic).

## Need help?

Open an issue describing the problem or idea. Tag it with "help wanted" or "design discussion" if you want feedback before working on it.

Thanks again — contributions big and small are welcome!
