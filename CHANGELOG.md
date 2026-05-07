# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added
- Robust drift detection & reconciliation:
  - Baseline snapshot: the backend records a baseline `~/.bazzite/base_packages.txt` at environment creation to filter out base image packages.
  - Machine-readable package queries for user-installed packages (primary + fallback):
    - Fedora/DNF: `dnf repoquery --userinstalled --qf '%{name}\n'` (primary) → fallback `rpm -qa --queryformat '%{NAME}\n'`.
    - Debian/APT: `apt-mark showmanual` (primary) → fallback `dpkg-query -f '${binary:Package}\n' -W`.
    - Arch/Pacman: `pacman -Qqe` (primary) → fallback `pacman -Qq`.
    - Alpine/APK: `apk info`.
  - Diffing formula: `(Current Packages - Baseline Packages) - Manifest Packages = Drift`.
  - Fallbacks emit a `fallback_used` flag returned by `detect_environment_drift` so the UI can warn users when a conservative fallback was necessary.
- Manifest normalization and seeding:
  - Manifest package entries are normalized (trimmed and lowercased) on creation and when updating, preventing case/whitespace mismatch drift.
  - Environment templates now seed `.envstation.json` with template base packages so newly scaffolded projects have zero day‑one drift.
- DevContainer scaffolding derived from manifest:
  - `devcontainer.json` postStartCommand content is built from the manifest (not hardcoded), ensuring DevContainer hooks align with the manifest.
- Frontend improvements:
  - `ManagePackagesModal` now consumes `fallback_used` and displays a warning banner when a fallback was used during drift detection.

### Changed
- Replaced brittle, human-oriented package-manager parsing with low-level database queries and added primary→fallback logic with warnings.
- Drift detection and baseline initialization use the new machine-readable commands and normalization pipeline.
- Documentation updated: ARCHITECTURE.md and README.md describe the new reconciliation strategy and guidance for UI behavior.

### Fixed
- Eliminated false-positive drift from transitive dependencies by switching to user-installed package queries.
- Resolved day‑one drift by seeding manifests with template packages and syncing DevContainer generation from the manifest.

---

## [1.0.0] - 2026-05-04

### Added
- C# / .NET scaffolding: minimal Program.cs and project file targeting .NET 8.0. Generated DevContainer and Distrobox scaffolds use the official .NET SDK image and recommend the VS Code C# extensions.
- Dynamic package manager detection: the backend probes the target environment and picks the appropriate package manager (dnf/apt/apk/pacman, etc.) when performing live installs.

### Changed
- Bumped application/package versions to 1.0.0 across package.json, src-tauri/tauri.conf.json, and displayed About modal.
- Updated README installation examples and CI/CD example in ARCHITECTURE.md.

### Notes
- This release focuses on improved scaffolding for .NET developers and more robust package-install flows across different container images.

