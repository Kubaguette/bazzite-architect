use crate::core::devcontainer::write_devcontainer_files;
use crate::core::util::build_host_command_async;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;

#[derive(Clone)]
struct EnvironmentTemplate {
    image: &'static str,
    packages: &'static [&'static str],
    init_snippet: &'static str,
}

fn get_template(name: &str) -> Result<EnvironmentTemplate, String> {
    match name {
        "react-ts" => Ok(EnvironmentTemplate {
            image: "registry.fedoraproject.org/fedora-toolbox:latest",
            packages: &[
                "nodejs",
                "npm",
                "tar",
            ],
            init_snippet: r#"
set -e
npm install -g typescript typescript-language-server pnpm yarn eslint prettier || true
"#,
        }),
        "python" => Ok(EnvironmentTemplate {
            image: "registry.fedoraproject.org/fedora-toolbox:latest",
            packages: &[
                "bzip2-devel",
                "gcc",
                "gcc-c++",
                "libffi-devel",
                "make",
                "openssl-devel",
                "pkgconf-pkg-config",
                "python3",
                "python3-devel",
                "python3-pip",
                "zlib-devel",
            ],
            init_snippet: r#"
set -e
python3 -m pip install --user --upgrade pip || true
"#,
        }),
        "cpp" => Ok(EnvironmentTemplate {
            image: "registry.fedoraproject.org/fedora-toolbox:latest",
            packages: &[
                "gcc",
                "gcc-c++",
                "glibc-devel",
                "libstdc++-devel",
                "make",
                "cmake",
                "ninja-build",
                "pkgconf-pkg-config",
                "ccache",
                "gdb",
                "lldb",
                "clang",
                "clang-tools-extra",
            ],
            init_snippet: r#"
set -e
cmake --version || true
make --version || true
gcc --version || true
g++ --version || true
clang++ --version || true
"#,
        }),
        "rust" => Ok(EnvironmentTemplate {
            image: "registry.fedoraproject.org/fedora-toolbox:latest",
            packages: &[
                "gcc",
                "gcc-c++",
                "make",
                "cmake",
                "pkgconf-pkg-config",
                "openssl-devel",
                "zlib-devel",
                "curl",
            ],
            init_snippet: r#"
set -e
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
. "$HOME/.cargo/env"
rustup component add rust-analyzer || true
rustc --version || true
cargo --version || true
rust-analyzer --version || true
"#,
        }),
        "java" => Ok(EnvironmentTemplate {
            image: "registry.fedoraproject.org/fedora-toolbox:latest",
            packages: &[
                "java-21-openjdk",
                "java-21-openjdk-devel",
                "maven",
            ],
            init_snippet: r#"
set -e
java -version || true
javac -version || true
mvn -v || true
"#,
        }),
        "csharp" => Ok(EnvironmentTemplate {
            image: "mcr.microsoft.com/dotnet/sdk:8.0",
            // Keep additional distrobox create packages empty for the Debian-based
            // Microsoft images to avoid invoking Fedora's --additional-packages
            // behavior which expects dnf. System packages for this template are
            // installed via apt when requested (see install_system_package).
            packages: &[],
            init_snippet: r#"
set -e
# Verify dotnet is available; avoid running distro package managers here.
dotnet --info || true
"#,
        }),
        other => Err(format!("Unknown template: {}", other)),
    }
}

/// Represents the semantic level of a progress update emitted during environment
/// creation.
///
/// Why: callers (UI and logging) need a compact, serializable indicator of whether a
/// progress message is informational or an error so that the frontend can decide how
/// to render and whether to interrupt workflows. Keeping this small avoids coupling
/// progress transport to presentation details.
#[derive(Debug, Clone)]
pub enum ProgressKind {
    Info,
    Error,
}

/// A progress event produced while creating an environment.
///
/// Why: progress updates carry a stable stage identifier and a human message. The
/// stage is &'static so producers can reuse constant stage labels without
/// allocating, simplifying lifecycle handling and event filtering in the UI.
#[derive(Debug, Clone)]
pub struct ProgressUpdate {
    pub stage: &'static str,
    pub message: String,
    pub kind: ProgressKind,
}

/// Parameters required to create a development environment.
///
/// Why: the parameters intentionally capture only immutable, serializable values
/// (strings and an optional path). Any path normalization and validation is done
/// inside create_environment to centralize safety checks and avoid leaking
/// platform-specific behavior to callers.
pub struct CreateEnvironmentParams {
    pub name: String,
    pub template: String,
    pub home_mount: Option<String>,
}

/// Result produced after a successful environment creation.
///
/// Why: the result contains a human-facing message and structured metadata used
/// by the frontend to display created artifacts and the actual home mount used.
/// Returning created file paths allows the UI to show what was scaffolded without
/// re-scanning the filesystem.
pub struct CreateEnvironmentResult {
    pub message: String,
    pub created_files: Vec<String>,
    pub created_home_dir: Option<String>,
    pub chosen_home: Option<String>,
}

/// On-disk manifest describing a scaffolded environment within a project
/// directory (.envstation.json).
///
/// Why: persisting a small manifest enables idempotent operations (e.g. adding
/// system packages) and allows the frontend to inspect environment metadata
/// without re-parsing project files.
#[derive(Serialize, Deserialize)]
pub struct EnvironmentManifest {
    pub version: String,
    pub name: String,
    pub stack: String,
    pub system_packages: Vec<String>,
}

/// Create and initialize a development environment according to the provided
/// parameters, emitting progress updates via the supplied callback.
///
/// Architectural intent / Why:
/// - Runs external tools (distrobox/podman) asynchronously to avoid blocking the
///   caller runtime. CPU/text-heavy work is executed on the async runtime where
///   appropriate and filesystem I/O uses non-blocking primitives when available.
/// - Emits structured progress events so the UI can remain responsive and reflect
///   long-running steps (create, enter, scaffold). This decouples creation logic
///   from presentation.
/// - Validates and normalizes user-provided paths and enforces safety guards
///   (absolute paths, directory checks) before performing destructive actions.
/// - Avoids capturing the progress closure across async boundaries; callers get
///   immediate events through the provided FnMut during the operation.
///
/// # Errors
/// Returns Err(String) for any operational failure, including but not limited to:
/// - Unknown template name.
/// - Failure to read or normalize HOME when needed.
/// - External command execution failures (distrobox create/enter/setup).
/// - Filesystem errors while scaffolding or writing the environment manifest.
pub async fn create_environment(
    params: CreateEnvironmentParams,
    mut progress: impl FnMut(ProgressUpdate) + Send,
) -> Result<CreateEnvironmentResult, String> {
    progress(ProgressUpdate {
        stage: "init",
        message: format!(
            "Preparing environment '{}' ({})",
            params.name, params.template
        ),
        kind: ProgressKind::Info,
    });

    let template = get_template(&params.template)?;

    let mut created_home_dir: Option<String> = None;
    let mut chosen_home: Option<String> = None;

    if let Some(hm) = params.home_mount.as_deref() {
        if !hm.trim().is_empty() {
            let mut v = hm.trim().to_string();
            if v.starts_with("$HOME/") {
                if let Ok(h) = std::env::var("HOME") {
                    v = format!(
                        "{}/{}",
                        h.trim_end_matches('/'),
                        v.trim_start_matches("$HOME/")
                    );
                }
            } else if v.starts_with("~/") {
                if let Ok(h) = std::env::var("HOME") {
                    v = format!("{}/{}", h.trim_end_matches('/'), v.trim_start_matches("~/"));
                }
            }
            chosen_home = Some(v);
        }
    }
    if chosen_home.is_none() {
        let host_home = std::env::var("HOME").map_err(|_| "Failed to read HOME".to_string())?;
        chosen_home = Some(format!(
            "{}/{}",
            host_home.trim_end_matches('/'),
            params.name
        ));
    }

    if let Some(home_mount) = &chosen_home {
        let p = Path::new(home_mount);
        if !p.is_absolute() {
            progress(ProgressUpdate {
                stage: "validate_home",
                message: format!("Home mount is not absolute: {}", home_mount),
                kind: ProgressKind::Error,
            });
            return Err(format!(
                "Home mount must be an absolute path: {}",
                home_mount
            ));
        }
        if !p.exists() {
            progress(ProgressUpdate {
                stage: "prepare_home",
                message: format!("Creating home mount directory: {}", home_mount),
                kind: ProgressKind::Info,
            });
            std::fs::create_dir_all(p).map_err(|e| {
                format!(
                    "Failed to create home mount directory ({}): {}",
                    home_mount, e
                )
            })?;
            created_home_dir = Some(home_mount.to_string());
        }
        if !p.is_dir() {
            progress(ProgressUpdate {
                stage: "validate_home",
                message: format!("Home mount is not a directory: {}", home_mount),
                kind: ProgressKind::Error,
            });
            return Err(format!("Home mount is not a directory: {}", home_mount));
        }
    }

    progress(ProgressUpdate {
        stage: "distrobox_create",
        message: "Running distrobox create".to_string(),
        kind: ProgressKind::Info,
    });

    let mut create_cmd = build_host_command_async("distrobox");
    create_cmd.args([
        "create",
        "--name",
        &params.name,
        "--image",
        template.image,
        "--yes",
    ]);
    if !template.packages.is_empty() {
        create_cmd
            .arg("--additional-packages")
            .arg(template.packages.join(" "));
    }
    if let Some(home_mount) = &chosen_home {
        create_cmd.args(["--home", home_mount]);
    }

    let create_output = create_cmd
        .output()
        .await
        .map_err(|e| format!("Failed to execute 'distrobox create': {}", e))?;

    if !create_output.status.success() {
        let err = String::from_utf8_lossy(&create_output.stderr)
            .trim()
            .to_string();
        progress(ProgressUpdate {
            stage: "distrobox_create",
            message: err.clone(),
            kind: ProgressKind::Error,
        });
        return Err(err);
    }

    progress(ProgressUpdate {
        stage: "distrobox_create",
        message: "distrobox create completed".to_string(),
        kind: ProgressKind::Info,
    });

    let init_script = format!(
        r#"set -e
{}
echo "ENVIRONMENT_READY""#,
        template.init_snippet
    );

    progress(ProgressUpdate {
        stage: "distrobox_enter",
        message: "Running initial setup inside environment".to_string(),
        kind: ProgressKind::Info,
    });

    let mut setup_cmd = build_host_command_async("distrobox");
    setup_cmd.args(["enter", &params.name, "--", "bash", "-lc", &init_script]);

    let setup_output = setup_cmd
        .output()
        .await
        .map_err(|e| format!("Failed to execute environment setup: {}", e))?;

    if !setup_output.status.success() {
        let err = format!(
            "Environment was created, but setup failed: {}",
            String::from_utf8_lossy(&setup_output.stderr).trim()
        );
        progress(ProgressUpdate {
            stage: "distrobox_enter",
            message: err.clone(),
            kind: ProgressKind::Error,
        });
        return Err(err);
    }

    // Record a baseline snapshot of installed packages inside the container so
    // future drift detection can diff against the initial environment state.
    progress(ProgressUpdate {
        stage: "baseline",
        message: "Recording baseline package snapshot inside environment".to_string(),
        kind: ProgressKind::Info,
    });

    // Probe package manager and choose a suitable listing command
    let probe = r#"if command -v apt-get >/dev/null 2>&1; then echo apt; \
elif command -v dnf >/dev/null 2>&1; then echo dnf; \
elif command -v apk >/dev/null 2>&1; then echo apk; \
elif command -v pacman >/dev/null 2>&1; then echo pacman; \
else echo unknown; fi"#;
    let probe_out = build_host_command_async("distrobox")
        .args(["enter", &params.name, "--", "sh", "-lc", probe])
        .output()
        .await;

    let pm = match probe_out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => String::from("unknown"),
    };

    // Determine primary and fallback listing commands that return only user-installed packages when possible.
    let (primary_cmd, fallback_cmd): (&str, Option<&str>) = match pm.as_str() {
        "apt" => ("apt-mark showmanual", Some("dpkg-query -f '${binary:Package}\\n' -W")),
        "dnf" => ("dnf repoquery --userinstalled --qf '%{name}\\n'", Some("rpm -qa --queryformat '%{NAME}\\n'")),
        "apk" => ("apk info", None),
        "pacman" => ("pacman -Qqe", Some("pacman -Qq")),
        _ => ("rpm -qa --queryformat '%{NAME}\\n'", None),
    };

    // Attempt primary, fall back if empty or failing.
    let mut list_output = String::new();
    let mut used_fallback = false;

    let mut primary_exec = build_host_command_async("distrobox");
    primary_exec.args(["enter", &params.name, "--", "sh", "-lc", primary_cmd]);
    if let Ok(po) = primary_exec.output().await {
        if po.status.success() {
            list_output = String::from_utf8_lossy(&po.stdout).to_string();
        }
    }

    if list_output.trim().is_empty() {
        if let Some(fb) = fallback_cmd {
            let mut fb_exec = build_host_command_async("distrobox");
            fb_exec.args(["enter", &params.name, "--", "sh", "-lc", fb]);
            if let Ok(fo) = fb_exec.output().await {
                if fo.status.success() {
                    list_output = String::from_utf8_lossy(&fo.stdout).to_string();
                    used_fallback = true;
                }
            }
        }
    }

    if used_fallback {
        progress(ProgressUpdate {
            stage: "baseline",
            message: format!("Warning: primary package-listing command failed or returned empty; used fallback for PM='{}'.", pm),
            kind: ProgressKind::Info,
        });
    }

    if list_output.trim().is_empty() {
        progress(ProgressUpdate {
            stage: "baseline",
            message: "Could not record baseline package snapshot (non-fatal)".to_string(),
            kind: ProgressKind::Error,
        });
    } else {
        // Normalize and write baseline file
        let mut printf_body = String::new();
        for line in list_output.lines() {
            let ln = line.trim();
            if ln.is_empty() { continue; }
            printf_body.push_str(&ln.to_lowercase());
            printf_body.push('\n');
        }
        let write_cmd = format!(
            "mkdir -p ~/.bazzite && cat > ~/.bazzite/base_packages.txt <<'EOF'\\n{}EOF\\n",
            printf_body
        );
        let mut write_exec = build_host_command_async("distrobox");
        write_exec.args(["enter", &params.name, "--", "sh", "-lc", &write_cmd]);
        match write_exec.output().await {
            Ok(wout) => {
                if !wout.status.success() {
                    progress(ProgressUpdate {
                        stage: "baseline",
                        message: "Could not record baseline package snapshot (non-fatal)".to_string(),
                        kind: ProgressKind::Error,
                    });
                } else {
                    progress(ProgressUpdate {
                        stage: "baseline",
                        message: "Baseline package snapshot recorded".to_string(),
                        kind: ProgressKind::Info,
                    });
                }
            }
            Err(e) => {
                progress(ProgressUpdate {
                    stage: "baseline",
                    message: format!("Failed to record baseline package snapshot: {}", e),
                    kind: ProgressKind::Error,
                });
            }
        }
    }

    progress(ProgressUpdate {
        stage: "scaffolding",
        message: "Scaffolding project files".to_string(),
        kind: ProgressKind::Info,
    });

    let mut created_files: Vec<String> = Vec::new();
    // extra_files is used to avoid borrow issues when the closure below is active.
    // Files written via the closure will be appended to extra_files and merged
    // into the final created_files list after the closure's scope ends.
    let mut extra_files: Vec<String> = Vec::new();
    // Collect errors from write attempts so we can report them after the
    // closure's lifetime ends. This avoids capturing `progress` inside the
    // closure (prevents E0499 mutable borrow issues).
    let mut write_errors: Vec<String> = Vec::new();
    // async_files collects files created via tokio async I/O so they can be
    // merged into the final created_files list after scaffolding completes.
    let mut async_files: Vec<String> = Vec::new();
    let mut devcontainer_files: Vec<String> = Vec::new();
    let mut write_file_if_absent = |path: &Path, content: &str| {
        if !path.exists() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Err(e) = std::fs::write(path, content) {
                // Do not call progress here to avoid borrowing it; record the
                // error for later reporting.
                write_errors.push(format!("Could not write {}: {}", path.display(), e));
            } else {
                // push to extra_files to avoid borrowing created_files while the
                // closure is still borrowed (prevents E0499 borrow-checker errors).
                extra_files.push(path.display().to_string());
            }
        }
    };

    if let Some(home_root) = &chosen_home {
        let root = Path::new(home_root);

        // Initialize manifest from the template's base packages so day-one drift is avoided.
        let manifest = EnvironmentManifest {
            version: "1.0.0".to_string(),
            name: params.name.clone(),
            stack: params.template.clone(),
            // Normalize manifest packages (lowercase, trimmed) to avoid case/whitespace mismatches.
            system_packages: template.packages.iter().map(|s| s.trim().to_lowercase()).collect(),
        };
        let manifest_path = root.join(".envstation.json");
        let json = serde_json::to_string_pretty(&manifest)
            .map_err(|e| format!("Failed to serialize manifest: {}", e))?;
        std::fs::write(&manifest_path, json).map_err(|e| {
            format!(
                "Failed to write manifest ({}): {}",
                manifest_path.display(),
                e
            )
        })?;
        // Record manifest as a created file so the UI can display it.
        created_files.push(manifest_path.display().to_string());

        match params.template.as_str() {
            "python" => {
                let src_dir = root.join("src");
                let tests_dir = root.join("tests");
                let _ = std::fs::create_dir_all(&src_dir);
                let _ = std::fs::create_dir_all(&tests_dir);

                write_file_if_absent(
                    &root.join("README.md"),
                    "# Python Project\n\nGenerated by EnvStation\n",
                );
                let exts = ["ms-python.python"];
                // Build post command from manifest system_packages + template-specific extras
                let install_fragment = if !manifest.system_packages.is_empty() {
                    format!("dnf install -y {}", manifest.system_packages.join(" "))
                } else {
                    String::new()
                };
                let mut post_cmd = String::new();
                if !install_fragment.is_empty() {
                    post_cmd.push_str(&install_fragment);
                    post_cmd.push_str(" && ");
                }
                post_cmd.push_str("python3 -m pip install -r requirements.txt || true");
                let post = Some(post_cmd.as_str());
                // write_devcontainer_files expects the post command to live long enough; we pass a temporary string but it's used immediately.
                if let Ok(mut files) = write_devcontainer_files(
                    root,
                    &params.name,
                    "registry.fedoraproject.org/fedora-toolbox:latest",
                    post,
                    &exts,
                ) {
                    devcontainer_files.append(&mut files);
                }

                let vs_dir = root.join(".vscode");
                let _ = std::fs::create_dir_all(&vs_dir);
                let ext_json = serde_json::json!({
                    "recommendations": ["ms-vscode-remote.remote-containers", "ms-python.python"]
                });
                let ext_path = vs_dir.join("extensions.json");
                if let Ok(data) = serde_json::to_string_pretty(&ext_json) {
                    write_file_if_absent(&ext_path, &data);
                }

                write_file_if_absent(&root.join(".gitignore"), "# Python\n__pycache__/\n*.py[cod]\n*.egg-info/\n.venv/\n.env\n\n# Editors\n.vscode/\n.idea/\n");
                write_file_if_absent(&root.join("pyproject.toml"), "[project]\nname = \"python_app\"\nversion = \"0.1.0\"\ndescription = \"Generated Python app\"\nrequires-python = \">=3.11\"\n\n[tool.pytest.ini_options]\npythonpath = [\"src\"]\n");
                // Ensure a minimal requirements.txt exists so postCreateCommand pip installs
                // do not fail with "file not found" in the DevContainer terminal.
                write_file_if_absent(&root.join("requirements.txt"), "# Requirements\n");
                write_file_if_absent(&src_dir.join("main.py"), "def main():\n    print(\"Hello from EnvStation!\")\n\nif __name__ == \"__main__\":\n    main()\n");
                write_file_if_absent(
                    &tests_dir.join("test_basic.py"),
                    "def test_example():\n    assert 1 + 1 == 2\n",
                );
            }
            "react-ts" => {
                let src_dir = root.join("src");
                let _ = std::fs::create_dir_all(&src_dir);

                write_file_if_absent(
                    &root.join("README.md"),
                    "# React + TypeScript\n\nGenerated by EnvStation\n",
                );
                write_file_if_absent(&root.join("index.html"), "<!doctype html>\n<html>\n  <head>\n    <meta charset=\"UTF-8\" />\n    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\" />\n    <title>React TS App</title>\n  </head>\n  <body>\n    <div id=\"root\"></div>\n    <script type=\"module\" src=\"/src/main.tsx\"></script>\n  </body>\n</html>\n");
                write_file_if_absent(&root.join("package.json"), &format!(
                    "{{\n  \"name\": \"{}\",\n  \"private\": true,\n  \"version\": \"0.1.0\",\n  \"type\": \"module\",\n  \"scripts\": {{\n    \"dev\": \"vite\",\n    \"build\": \"tsc && vite build\",\n    \"preview\": \"vite preview\"\n  }},\n  \"dependencies\": {{\n    \"react\": \"^19.0.0\",\n    \"react-dom\": \"^19.0.0\"\n  }},\n  \"devDependencies\": {{\n    \"@vitejs/plugin-react\": \"^4.0.0\",\n    \"typescript\": \"^5.0.0\",\n    \"vite\": \"^5.0.0\"\n  }}\n}}\n",
                    params.name
                ));
                write_file_if_absent(&root.join("tsconfig.json"), "{\n  \"compilerOptions\": {\n    \"target\": \"ES2020\",\n    \"useDefineForClassFields\": true,\n    \"module\": \"ESNext\",\n    \"lib\": [\"ES2020\", \"DOM\"],\n    \"skipLibCheck\": true,\n    \"moduleResolution\": \"bundler\",\n    \"resolveJsonModule\": true,\n    \"isolatedModules\": true,\n    \"noEmit\": true,\n    \"jsx\": \"react-jsx\"\n  },\n  \"include\": [\"src\"]\n}\n");
                write_file_if_absent(&root.join("vite.config.ts"), "import { defineConfig } from 'vite'\nimport react from '@vitejs/plugin-react'\n\nexport default defineConfig({\n  plugins: [react()],\n})\n");
                write_file_if_absent(&src_dir.join("main.tsx"), "import React from 'react'\nimport ReactDOM from 'react-dom/client'\nimport App from './App'\n\nReactDOM.createRoot(document.getElementById('root')!).render(\n  <React.StrictMode>\n    <App />\n  </React.StrictMode>\n)\n");
                write_file_if_absent(&src_dir.join("App.tsx"), "export default function App() {\n  return <h1>Hello React + TS from EnvStation!</h1>\n}\n");
                write_file_if_absent(&root.join(".gitignore"), "# Node\nnode_modules/\ndist/\n.npm/\n.pnpm-store/\n\n# Editors\n.vscode/\n.idea/\n");
                let exts = [
                    "dbaeumer.vscode-eslint",
                    "esbenp.prettier-vscode",
                    "ms-vscode.vscode-typescript-next",
                ];
                let install_fragment = if !manifest.system_packages.is_empty() {
                    format!("dnf install -y {}", manifest.system_packages.join(" "))
                } else {
                    String::new()
                };
                let mut post_cmd = String::new();
                if !install_fragment.is_empty() {
                    post_cmd.push_str(&install_fragment);
                    post_cmd.push_str(" && ");
                }
                post_cmd.push_str("npm install -g typescript typescript-language-server pnpm yarn eslint prettier && npm install");
                let post = Some(post_cmd.as_str());
                if let Ok(mut files) = write_devcontainer_files(
                    root,
                    &params.name,
                    "registry.fedoraproject.org/fedora-toolbox:latest",
                    post,
                    &exts,
                ) {
                    devcontainer_files.append(&mut files);
                }
            }
            "cpp" => {
                let src_dir = root.join("src");
                let _ = std::fs::create_dir_all(&src_dir);
                write_file_if_absent(
                    &root.join("README.md"),
                    "# C/C++ Project\n\nGenerated by EnvStation\n",
                );
                write_file_if_absent(&root.join("CMakeLists.txt"), "cmake_minimum_required(VERSION 3.16)\nproject(app LANGUAGES C CXX)\nset(CMAKE_CXX_STANDARD 17)\nadd_executable(app src/main.cpp)\n");
                write_file_if_absent(&src_dir.join("main.cpp"), "#include <iostream>\nint main(){ std::cout << \"Hello C++ from EnvStation!\\n\"; return 0; }\n");
                write_file_if_absent(&root.join(".gitignore"), "# CMake\n/build/\nCMakeCache.txt\nCMakeFiles/\n\n# Editors\n.vscode/\n.idea/\n");

                // Generate CMakePresets.json at the project root to provide a
                // silent, reproducible configure preset that uses Ninja and the
                // container's gcc/g++ compilers. This avoids Kit selection popups.
                let cmake_presets_path = root.join("CMakePresets.json");
                let cmake_presets_content = r#"{
"version": 3,
"configurePresets": [
  {
    "name": "default",
    "hidden": false,
    "generator": "Ninja",
    "binaryDir": "${sourceDir}/build",
    "cacheVariables": {
      "CMAKE_EXPORT_COMPILE_COMMANDS": "ON",
      "CMAKE_C_COMPILER": "gcc",
      "CMAKE_CXX_COMPILER": "g++"
    }
  }
],
"buildPresets": [
  {
    "name": "default",
    "configurePreset": "default"
  }
]
}"#;
                match tokio::fs::write(&cmake_presets_path, cmake_presets_content).await {
                    Ok(_) => {
                        async_files.push(cmake_presets_path.display().to_string());
                    }
                    Err(e) => {
                        progress(ProgressUpdate {
                            stage: "scaffolding",
                            message: format!("Could not write {}: {}", cmake_presets_path.display(), e),
                            kind: ProgressKind::Error,
                        });
                    }
                }

                // Create .vscode and write c_cpp_properties.json to point
                // IntelliSense at the compile_commands.json generated by CMake.
                let vs_dir = root.join(".vscode");
                match tokio::fs::create_dir_all(&vs_dir).await {
                    Ok(_) => {
                        let c_cpp_path = vs_dir.join("c_cpp_properties.json");
                        let c_cpp_content = r#"{
"configurations": [
  {
    "name": "Linux",
    "includePath": [
      "${workspaceFolder}/**"
    ],
    "compileCommands": "${workspaceFolder}/build/compile_commands.json",
    "compilerPath": "/usr/bin/g++",
    "cStandard": "c11",
    "cppStandard": "c++17",
    "intelliSenseMode": "linux-gcc-x64"
  }
],
"version": 4
}"#;
                        match tokio::fs::write(&c_cpp_path, c_cpp_content).await {
                            Ok(_) => {
                                async_files.push(c_cpp_path.display().to_string());
                            }
                            Err(e) => {
                                progress(ProgressUpdate {
                                    stage: "scaffolding",
                                    message: format!("Could not write {}: {}", c_cpp_path.display(), e),
                                    kind: ProgressKind::Error,
                                });
                            }
                        }
                    }
                    Err(e) => {
                        progress(ProgressUpdate {
                            stage: "scaffolding",
                            message: format!("Could not create {}: {}", vs_dir.display(), e),
                            kind: ProgressKind::Error,
                        });
                    }
                }

                let exts = ["ms-vscode.cpptools", "ms-vscode.cmake-tools"];
                let install_fragment = if !manifest.system_packages.is_empty() {
                    format!("dnf install -y {}", manifest.system_packages.join(" "))
                } else {
                    String::new()
                };
                let mut post_cmd = String::new();
                if !install_fragment.is_empty() {
                    post_cmd.push_str(&install_fragment);
                }
                let post = if post_cmd.is_empty() { None } else { Some(post_cmd.as_str()) };
                if let Ok(mut files) = write_devcontainer_files(
                    root,
                    &params.name,
                    "registry.fedoraproject.org/fedora-toolbox:latest",
                    post,
                    &exts,
                ) {
                    devcontainer_files.append(&mut files);
                }
            }
            "rust" => {
                let src_dir = root.join("src");
                let _ = std::fs::create_dir_all(&src_dir);
                write_file_if_absent(
                    &root.join("README.md"),
                    "# Rust Project\n\nGenerated by EnvStation\n",
                );
                write_file_if_absent(&root.join("Cargo.toml"), &format!(
                    "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n",
                    params.name
                ));
                write_file_if_absent(
                    &src_dir.join("main.rs"),
                    "fn main() {\n    println!(\"Hello Rust from EnvStation!\");\n}\n",
                );
                write_file_if_absent(
                    &root.join(".gitignore"),
                    "# Rust\n/target\n**/*.rs.bk\n\n# Editors\n.vscode/\n.idea/\n",
                );
                let exts = ["rust-lang.rust-analyzer"];
                let install_fragment = if !manifest.system_packages.is_empty() {
                    format!("dnf install -y {}", manifest.system_packages.join(" "))
                } else {
                    String::new()
                };
                let mut post_cmd = String::new();
                if !install_fragment.is_empty() {
                    post_cmd.push_str(&install_fragment);
                    post_cmd.push_str(" && ");
                }
                post_cmd.push_str("curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && . $HOME/.cargo/env && rustup component add rust-analyzer && cargo fetch");
                let post = Some(post_cmd.as_str());
                if let Ok(mut files) = write_devcontainer_files(
                    root,
                    &params.name,
                    "registry.fedoraproject.org/fedora-toolbox:latest",
                    post,
                    &exts,
                ) {
                    devcontainer_files.append(&mut files);
                }
            }
            "csharp" => {
                let src_dir = root.join("src");
                let _ = std::fs::create_dir_all(&src_dir);

                write_file_if_absent(
                    &root.join("README.md"),
                    "# C# Project\n\nGenerated by EnvStation\n",
                );

                write_file_if_absent(&root.join(format!("{}.csproj", params.name)), &format!(
                    "<Project Sdk=\"Microsoft.NET.Sdk\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <TargetFramework>net8.0</TargetFramework>\n    <ImplicitUsings>enable</ImplicitUsings>\n    <Nullable>enable</Nullable>\n  </PropertyGroup>\n</Project>\n"
                ));

                write_file_if_absent(&src_dir.join("Program.cs"), "using System;\n\nConsole.WriteLine(\"Hello C# from EnvStation!\");\n");

                write_file_if_absent(&root.join(".gitignore"), "# C#\n/bin/\n/obj/\n\n# Editors\n.vscode/\n.idea/\n");

                let exts = ["ms-dotnettools.csharp", "ms-dotnettools.csdevkit"];
                let install_fragment = if !manifest.system_packages.is_empty() {
                    // For Debian-based images, prefer apt-get install. We'll attempt apt-get
                    // to be conservative; if apt is unavailable the command may fail harmlessly.
                    format!("apt-get update && apt-get install -y {}", manifest.system_packages.join(" "))
                } else {
                    String::new()
                };
                let mut post_cmd = String::new();
                if !install_fragment.is_empty() {
                    post_cmd.push_str(&install_fragment);
                    post_cmd.push_str(" && ");
                }
                post_cmd.push_str("dotnet restore || true");
                let post = Some(post_cmd.as_str());
                if let Ok(mut files) = write_devcontainer_files(
                    root,
                    &params.name,
                    "mcr.microsoft.com/dotnet/sdk:8.0",
                    post,
                    &exts,
                ) {
                    devcontainer_files.append(&mut files);
                }
            },

            "java" => {
                let pkg = params.name.replace('-', "");
                let group = "com.example";
                let src_main = root.join("src/main/java").join(group.replace('.', "/"));
                let src_test = root.join("src/test/java").join(group.replace('.', "/"));
                let _ = std::fs::create_dir_all(&src_main);
                let _ = std::fs::create_dir_all(&src_test);

                write_file_if_absent(
                    &root.join("README.md"),
                    "# Java Project\n\nGenerated by EnvStation\n",
                );
                write_file_if_absent(&root.join("pom.xml"), &format!(
                    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<project xmlns=\"http://maven.apache.org/POM/4.0.0\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:schemaLocation=\"http://maven.apache.org/POM/4.0.0 http://maven.apache.org/xsd/maven-4.0.0.xsd\">\n  <modelVersion>4.0.0</modelVersion>\n  <groupId>{}</groupId>\n  <artifactId>{}</artifactId>\n  <version>0.1.0-SNAPSHOT</version>\n  <properties>\n    <maven.compiler.source>21</maven.compiler.source>\n    <maven.compiler.target>21</maven.compiler.target>\n    <project.build.sourceEncoding>UTF-8</project.build.sourceEncoding>\n  </properties>\n  <dependencies>\n    <dependency>\n      <groupId>org.junit.jupiter</groupId>\n      <artifactId>junit-jupiter</artifactId>\n      <version>5.10.2</version>\n      <scope>test</scope>\n    </dependency>\n  </dependencies>\n  <build>\n    <plugins>\n      <plugin>\n        <groupId>org.apache.maven.plugins</groupId>\n        <artifactId>maven-surefire-plugin</artifactId>\n        <version>3.2.5</version>\n      </plugin>\n    </plugins>\n  </build>\n</project>\n",
                    group, pkg
                ));
                write_file_if_absent(&src_main.join("App.java"), &format!(
                    "package {}\n\npublic class App {{\n    public static void main(String[] args) {{\n        System.out.println(\"Hello Java from EnvStation!\");\n    }}\n}}\n",
                    group
                ));
                write_file_if_absent(&src_test.join("AppTest.java"), &format!(
                    "package {}\n\nimport org.junit.jupiter.api.Test;\nimport static org.junit.jupiter.api.Assertions.*;\n\npublic class AppTest {{\n    @Test\n    void basic() {{\n        assertEquals(2, 1 + 1);\n    }}\n}}\n",
                    group
                ));
                write_file_if_absent(
                    &root.join(".gitignore"),
                    "# Maven\n/target\n\n# Editors\n.vscode/\n.idea/\n",
                );
                let exts = [
                    "redhat.java",
                    "vscjava.vscode-java-debug",
                    "vscjava.vscode-maven",
                ];
                let install_fragment = if !manifest.system_packages.is_empty() {
                    format!("dnf install -y {}", manifest.system_packages.join(" "))
                } else {
                    String::new()
                };
                let mut post_cmd = String::new();
                if !install_fragment.is_empty() {
                    post_cmd.push_str(&install_fragment);
                    post_cmd.push_str(" && ");
                }
                post_cmd.push_str("mvn -q -DskipTests dependency:go-offline || true");
                let post = Some(post_cmd.as_str());
                if let Ok(mut files) = write_devcontainer_files(
                    root,
                    &params.name,
                    "registry.fedoraproject.org/fedora-toolbox:latest",
                    post,
                    &exts,
                ) {
                    devcontainer_files.append(&mut files);
                }
            }
            _ => {
                let exts: [&str; 0] = [];
                // default: build post from manifest if any
                let mut post_cmd_string = String::new();
                if !manifest.system_packages.is_empty() {
                    post_cmd_string = format!("dnf install -y {}", manifest.system_packages.join(" "));
                }
                let post = if post_cmd_string.is_empty() { None } else { Some(post_cmd_string.as_str()) };
                if let Ok(mut files) = write_devcontainer_files(
                    root,
                    &params.name,
                    "registry.fedoraproject.org/fedora-toolbox:latest",
                    post,
                    &exts,
                ) {
                    devcontainer_files.append(&mut files);
                }
            }
        }

        let vs_dir = root.join(".vscode");
        let _ = std::fs::create_dir_all(&vs_dir);
        let ext_json = serde_json::json!({
            "recommendations": ["ms-vscode-remote.remote-containers"]
        });
        let ext_path = vs_dir.join("extensions.json");
        if let Ok(data) = serde_json::to_string_pretty(&ext_json) {
            write_file_if_absent(&ext_path, &data);
        }
    }

    // note: manifest was created earlier to avoid day-one drift
    if let Some(home_root) = &chosen_home {
        // Security/UX: prevent GNOME Tracker from indexing newly created
        // project home directories and avoid excessive I/O on developer machines.
        // An empty .trackerignore is sufficient for Tracker to skip the directory.
        let tracker_path = Path::new(home_root).join(".trackerignore");
        match std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .create_new(false)
            .open(&tracker_path)
        {
            Ok(mut f) => {
                // Ensure the file exists; keep it empty. Record it in created_files.
                let _ = f.write_all(b"");
                created_files.push(tracker_path.display().to_string());
            }
            Err(e) => {
                // Non-fatal: report as scaffolding warning
                write_errors.push(format!("Could not create {}: {}", tracker_path.display(), e));
            }
        }

        // Optionally create a .nomedia to help other indexers/mobile viewers skip the folder.
        let nomedia_path = Path::new(home_root).join(".nomedia");
        if !nomedia_path.exists() {
            if let Err(e) = std::fs::write(&nomedia_path, "") {
                write_errors.push(format!("Could not write {}: {}", nomedia_path.display(), e));
            } else {
                created_files.push(nomedia_path.display().to_string());
            }
        }
    }
    created_files.extend(devcontainer_files);
    // Merge in any files written by the scaffolding closure. We keep this separate
    // to avoid mutable-borrow conflicts while the closure was active.
    created_files.extend(extra_files);
    // Also include any files created with async tokio I/O.
    created_files.extend(async_files);

    // Report any write errors that were collected during scaffolding.
    for err in write_errors.into_iter() {
        progress(ProgressUpdate {
            stage: "scaffolding",
            message: err,
            kind: ProgressKind::Error,
        });
    }

    progress(ProgressUpdate {
        stage: "complete",
        message: "Environment created successfully".to_string(),
        kind: ProgressKind::Info,
    });

    let mut msg = format!(
        "✅ Environment '{}' ({}) was created and initialized.",
        params.name, params.template
    );
    if !created_files.is_empty() {
        msg.push_str(&format!(
            "\n📄 Project scaffold created ({} files).",
            created_files.len()
        ));
    }
    if let Some(dir) = &created_home_dir {
        msg.push_str(&format!(
            "\n📁 Home mount was created automatically: {}",
            dir
        ));
    }
    if let Some(ch) = &chosen_home {
        msg.push_str(&format!("\n🏠 Home mount used: {}", ch));
    }

    Ok(CreateEnvironmentResult {
        message: msg,
        created_files,
        created_home_dir,
        chosen_home,
    })
}
