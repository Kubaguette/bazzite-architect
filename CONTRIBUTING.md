# CONTRIBUTING.md

Thank you for wanting to contribute to EnvStation! This document explains how to submit contributions, which technical guardrails apply, and what the development workflow looks like.

## Welcome

We welcome contributions — bug reports, feature requests and pull requests are all appreciated. Please read this document and the ARCHITECTURE.md (see below) before you start making changes.

## Getting Started

- Reporting issues: Use the repository issue tracker. Provide reproducible steps, expected behavior, actual behavior and relevant logs.
- Discuss larger changes first as an issue or RFC before starting implementation.

## Development Workflow (brief)

Recommended environment:
- Development should run inside a mutable container (Distrobox/toolbox) because the host OS (Bazzite) is read-only.

Basic setup:

- Create or enter a mutable development container (example):

```bash CONTRIBUTING.md
# create or enter your mutable development container (example):
# distrobox-create --name devbox --image registry.fedoraproject.org/fedora-toolbox:latest --yes
distrobox enter devbox
```

- Clone the repository and install JS dependencies:

```bash CONTRIBUTING.md
git clone https://github.com/Kubaguette/envstation.git
cd envstation
npm install
```

- Native build dependencies (Fedora / Bazzite example):

```bash CONTRIBUTING.md
# Install system packages required for WebKit/Gtk bindings and native builds
sudo dnf install -y webkit2gtk4.1-devel libappindicator-gtk3-devel librsvg2-devel gtk3-devel gcc gcc-c++ make
```

Note: Install these native packages inside the container or ensure they are accessible from the host environment.

- Start the unified development workflow:

```bash CONTRIBUTING.md
# From the project root, this compiles the Rust backend and starts the Vite dev server together
npm run tauri dev
```

This single command runs the integrated developer loop appropriate for immutable-host workflows: it builds the Rust backend and launches the frontend dev server (Vite) so you can iterate on both layers seamlessly.

- Optional: Rust toolchain

```bash CONTRIBUTING.md
# If you need to manage the Rust toolchain manually:
rustup default stable
cargo build
cargo test
```

- WebKit / GTK note

UI components may rely on WebKit-related features (e.g. WebKitGTK). Prefer the package set shown above (webkit2gtk4.1-devel and friends) on Fedora/Bazzite. If you work inside a Distrobox container, install these dependencies in the container or ensure they are provided by the host.

## Technical Rules (mandatory)

1) Technical Design Authority
- ARCHITECTURE.md is the authoritative document for all design decisions and must be read before making code changes.
- If you propose a design change, open an issue or RFC and reference the relevant section in ARCHITECTURE.md. Larger changes require approval from the Technical Design Authority (TDA) before implementation.

2) Architecture pattern: Core | Commands | View
- Strict pattern: Core → Commands → View
  - Core: All core logic belongs in the Rust core (library/crate). The core must remain platform- and UI-agnostic.
  - Commands: The IPC/command surface (e.g. CLI handlers, IPC adapters) wraps calls into the core and translates between external callers and core functions.
  - View: All UI/UX (React + TypeScript) must remain in the view layer only.
- Avoid putting business logic in Commands or View. Unit and integration tests for core logic belong in the Rust test suite.

3) Host delegation (absolutely critical)
- All CLI invocations to the host system (e.g. podman, distrobox, systemd, etc.) must go through the host executor — use the shared interface build_host_command_async.
- Calls such as Command::new("podman") or direct spawn/exec of host tools are not allowed. Delegation ensures commands run correctly when the code executes inside a container.
- Audit existing commands for direct host calls and refactor them if necessary.

4) I/O safety
- Backend contributions must follow principles of limited parallelism:
  - Cap the Rayon thread pool for I/O-heavy tasks (Rayon thread-pool capping). Use shared configuration or ThreadPoolBuilder to limit thread counts.
  - Use cancellation / generation tokens (Generation-Tokens) for long-running or cancellable I/O tasks. Tasks must respond to cancellation signals and release resources deterministically.
- Document rationale for limits and any configuration points in code.

## Development environment (details)

- Because the host OS (Bazzite) is read-only, we recommend local mutable container-based development environments (e.g. Distrobox).
- Benefits:
  - Full write access for dependencies, builds and local caches.
  - Close to the target platform while retaining flexibility.

Example (Distrobox):

```bash CONTRIBUTING.md
# Create and enter a Distrobox (example):
distrobox-create -n bazzite-dev -i registry.fedoraproject.org/fedora:latest
distrobox-enter -n bazzite-dev
# Inside the container: clone the repo, install dependencies, run builds/tests
```

## Code quality

- Create small, reviewable pull requests that focus on a single change/bugfix/feature.
- Maintain a strict separation between Core, Commands and View.
- Linting / formatting:
  - Rust: cargo fmt, cargo clippy
  - TypeScript / React: npm run lint, npm run format (or the project-specific scripts)
- Tests: Add unit tests at the appropriate layer. Core logic must be covered by Rust tests.

## PR process

Before opening a PR:
- Ensure you have read and considered ARCHITECTURE.md.
- Small, focused changes: one PR = one topic.
- Update or add tests for your feature/bugfix.
- Run format and lint checks locally.
- Ensure all CI checks (if configured) pass.

When writing the PR description:
- Reference issues or RFCs that explain the design and motivation.
- Briefly describe the change, why it is needed, and which parts of the system are affected (Core / Commands / View).
- List manual test steps and include relevant logs or screenshots if available.

Review criteria (what reviewers should check):
- Does the change comply with ARCHITECTURE.md?
- Is the Core | Commands | View separation preserved?
- Are all host CLI invocations delegated through build_host_command_async?
- Does the code respect I/O safety principles (thread capping, generation tokens)?
- Are tests present and meaningful?
- Have lint/format checks passed?

Merge process:
- After the required reviewers have approved and CI is green, the PR may be merged.
- Breaking changes or architecture modifications require TDA approval before merge.

## Support and contact

- If in doubt: open an issue and tag it as "design discussion" or "help wanted".
- For security-sensitive vulnerabilities: report them confidentially via the repository's designated contact channels.

Thank you for contributing to EnvStation! Your care helps us build a secure, maintainable and reliable tool for development environments on Bazzite/Fedora Atomic.
