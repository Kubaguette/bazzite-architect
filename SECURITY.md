# Security Policy

## Supported Versions

We currently only provide security updates for the latest major version of Bazzite Architect.

| Version | Supported          |
| ------- | ------------------ |
| 1.1.x   | :white_check_mark: |
| < 1.0   | :x:                |

## Reporting a Vulnerability

Please do **not** report security vulnerabilities through public GitHub issues. 

If you believe you have found a security vulnerability in Bazzite Architect, please report it to us privately via email: **jakubpietraszkoapps@gmail.com**

We will acknowledge receipt of your vulnerability report within 48 hours and strive to send you regular updates about our progress. If you're reporting a bug related to privilege escalation or rootless Podman bypasses, please include steps to reproduce.

## Architecture Security Note
Bazzite Architect is designed to operate using **rootless Podman** and standard user privileges. The application should never be run with `sudo`.
