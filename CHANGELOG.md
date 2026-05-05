# Changelog

All notable changes to this project will be documented in this file.

## [1.2.0] - 2026-05-04

### Added
- C# / .NET scaffolding: minimal Program.cs and project file targeting .NET 8.0. Generated DevContainer and Distrobox scaffolds use the official .NET SDK image and recommend the VS Code C# extensions.
- Dynamic package manager detection: the backend probes the target environment and picks the appropriate package manager (dnf/apt/apk/pacman, etc.) when performing live installs.

### Changed
- Bumped application/package versions to 1.2.0 across package.json, src-tauri/tauri.conf.json, and displayed About modal.
- Updated README installation examples and CI/CD example in ARCHITECTURE.md.

### Notes
- This release focuses on improved scaffolding for .NET developers and more robust package-install flows across different container images.

