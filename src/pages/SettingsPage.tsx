/**
 * src/pages/SettingsPage.tsx
 *
 * Settings view. Allows running a backend system check and toggling the
 * "Advanced Mode" flag persisted in localStorage. The system check is
 * performed by invoking the Tauri command "system_check" which returns a
 * SystemCheckResult describing availability/version of required host tools.
 * UI-originating actions are logged via the "client_log" backend command.
 */

import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import PageHeader from "../components/PageHeader";

interface SystemCheckResult {
  podman_ok: boolean;
  podman_version: string | null;
  distrobox_ok: boolean;
  distrobox_version: string | null;
}

export default function SettingsPage() {
  const [system, setSystem] = useState<SystemCheckResult | null>(null);
  const [msg, setMsg] = useState<string>("");
  const [advanced, setAdvanced] = useState<boolean>(() => {
    return localStorage.getItem("advancedMode") === "1";
  });

  // EnvShare (GitHub PAT) state
  const [githubPat, setGithubPat] = useState<string>("");
  const [patLoaded, setPatLoaded] = useState<boolean>(false);
  const [infoOpen, setInfoOpen] = useState<boolean>(false);

  const runSystemCheck = async () => {
    setMsg("Checking...");
    try {
      // Log start of system check to Logs page
      await invoke("client_log", { source: "ui", level: "INFO", message: "system_check requested" });
      const result = await invoke<SystemCheckResult>("system_check");
      setSystem(result);
      setMsg("");
      // Log summarized results
      try {
        const msg = `system_check result: podman_ok=${result.podman_ok}${result.podman_version ? ` (${result.podman_version})` : ""}, distrobox_ok=${result.distrobox_ok}${result.distrobox_version ? ` (${result.distrobox_version})` : ""}`;
        await invoke("client_log", { source: "ui", level: "INFO", message: msg });
      } catch {}
    } catch (e) {
      const errMsg = String(e);
      setMsg(`Error: ${errMsg}`);
      setSystem(null);
      try { await invoke("client_log", { source: "ui", level: "ERROR", message: `system_check failed: ${errMsg}` }); } catch {}
    }
  };

  useEffect(() => {
    localStorage.setItem("advancedMode", advanced ? "1" : "0");
    window.dispatchEvent(new Event("advanced-mode-changed"));
  }, [advanced]);

  // Load existing PAT on mount via backend command
  useEffect(() => {
    (async () => {
      try {
        const val = await invoke<string | null>("get_github_pat");
        if (val) setGithubPat(val);
      } catch (e) {
        console.error("Error reading github_pat from backend:", e);
      } finally {
        setPatLoaded(true);
      }
    })();
  }, []);

  // Design notes: follow a loose golden-ratio spacing system (base 12px, then ~20px gaps)
  const base = 12;
  const golden = Math.round(base * 1.618);

  // Save PAT helper (on blur) — use backend commands to persist
  const savePat = async (value?: string) => {
    const v = value !== undefined ? value : githubPat;
    try {
      await invoke("set_github_pat", { pat: v || "" });
    } catch (e) {
      console.error("Error saving github_pat via backend:", e);
    }
  };

  return (
    <section>
      <PageHeader title="Settings" />

      <div style={{ display: 'grid', gridTemplateColumns: '1fr', gap: golden }}>
        <div style={{ padding: 16, background: '#0f1724', borderRadius: 12, border: '1px solid rgba(255,255,255,0.04)', boxShadow: '0 6px 18px rgba(2,6,23,0.6)' }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 12 }}>
            <div>
              <div style={{ fontWeight: 700, color: '#ffffff' }}>System Check</div>
              <div style={{ marginTop: 6, color: '#e5e7eb', fontSize: 13 }}>Validate host tooling required by the app</div>
            </div>
            <div>
              <button
                onClick={runSystemCheck}
                className={`system-check-btn ${msg === 'Checking...' || system ? 'active' : ''}`}
                style={{ background: '#2563eb', color: '#fff', border: 'none', padding: '8px 12px', borderRadius: 8, cursor: 'pointer', fontWeight: 700 }}
              >
                Check
              </button>
            </div>
          </div>

          <div style={{ marginTop: 12 }}>
            <div className={`expanding-box ${msg || system ? 'open' : ''}`}>
              {msg ? <div className="status-box" style={{ padding: 10, marginTop: 8 }}>{msg}</div> : null}
              {system ? (
                <div className="status-box" style={{ textAlign: 'left', padding: 12, marginTop: msg ? 8 : 6 }}>
                  <p style={{ margin: 0 }}>
                    <strong>Podman:</strong> {system.podman_ok ? " ✅" : " ❌"}
                    {system.podman_version ? ` – ${system.podman_version}` : ""}
                  </p>
                  <p style={{ marginTop: 6, marginBottom: 0 }}>
                    <strong>Distrobox:</strong> {system.distrobox_ok ? " ✅" : " ❌"}
                    {system.distrobox_version ? ` – ${system.distrobox_version}` : ""}
                  </p>
                </div>
              ) : null}
            </div>
          </div>
        </div>

        <div style={{ padding: 16, background: '#0f1724', borderRadius: 12, border: '1px solid rgba(255,255,255,0.04)', boxShadow: '0 6px 18px rgba(2,6,23,0.6)' }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 12 }}>
            <div>
              <div style={{ fontWeight: 700, color: '#ffffff' }}>Advanced</div>
              <div style={{ marginTop: 6, color: '#e5e7eb', fontSize: 13 }}>Expose advanced features and developer controls</div>
            </div>
            <div style={{ minWidth: 140, textAlign: 'right' }}>
              <label style={{ display: 'inline-flex', alignItems: 'center', gap: 8 }}>
                <input
                  type="checkbox"
                  checked={advanced}
                  onChange={(e) => setAdvanced(e.target.checked)}
                  style={{ width: 16, height: 16, boxSizing: 'border-box', padding: 0, margin: 0, accentColor: 'var(--primary-blue)' }}
                />
                <span style={{ color: '#e5e7eb' }}>Show</span>
              </label>
            </div>
          </div>
        </div>

        {/* GitHub Integration (EnvShare) card */}
        <div style={{ padding: 16, background: '#0f1724', borderRadius: 12, border: '1px solid rgba(255,255,255,0.04)', boxShadow: '0 6px 18px rgba(2,6,23,0.6)' }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 12 }}>
            <div>
              <div style={{ fontWeight: 700, color: '#ffffff', display: 'flex', alignItems: 'center', gap: 8 }}>
                <span>GitHub Integration (EnvShare)</span>
              </div>
              <div style={{ marginTop: 6, color: '#e5e7eb', fontSize: 13 }}>Store a limited-scope GitHub Personal Access Token locally to enable creating public Gists.</div>
            </div>
          </div>

          <div style={{ marginTop: 12 }}>
            <div className={`status-box`} style={{ padding: 12 }}>
              <label style={{ display: 'block', fontSize: 13, marginBottom: 8, color: '#e5e7eb' }}>
                Personal Access Token
                <span
                  onClick={() => setInfoOpen(true)}
                  title="Info"
                  style={{ marginLeft: 8, cursor: 'pointer', display: 'inline-flex', alignItems: 'center', justifyContent: 'center', width: 18, height: 18, borderRadius: 9, background: '#111827', color: '#9ca3af', fontSize: 12, marginTop: -2 }}
                >
                  ?
                </span>
              </label>

              <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                <input
                  type="password"
                  value={githubPat}
                  onChange={(e) => setGithubPat(e.target.value)}
                  placeholder={patLoaded ? "Paste your GitHub PAT here" : "Loading..."}
                  style={{
                    flex: 1,
                    padding: '8px 10px',
                    borderRadius: 8,
                    border: '1px solid rgba(255,255,255,0.04)',
                    background: '#0b1220',
                    color: '#e5e7eb',
                    height: 36,
                    boxSizing: 'border-box'
                  }}
                />
                <button
                  onClick={async () => { await savePat(); }}
                  style={{
                    background: '#2563eb',
                    color: '#fff',
                    border: 'none',
                    padding: '8px 12px',
                    borderRadius: 8,
                    cursor: 'pointer',
                    fontWeight: 700,
                    height: 36,
                    display: 'inline-flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    transform: 'translateY(-5px)'
                  }}
                >
                  Save
                </button>
              </div>
            </div>
          </div>
        </div>

      </div>

      {infoOpen && (
        <div className={`modal-backdrop show`}>
          <div className="modal-card" style={{ maxWidth: 620 }}>
            <header>Sharing Environments via GitHub</header>
            <div className="body">
              <p>To share your EnvStation setups with others via a simple link, the app creates a "Gist" (a public text snippet) on your GitHub account.</p>
              <h4 style={{ marginTop: 8 }}>How to get your PAT (Personal Access Token):</h4>
              <ol>
                <li>Go to GitHub: Settings ➔ Developer Settings ➔ Personal access tokens (classic).</li>
                <li>Click Generate new token.</li>
                <li>Give it a name (e.g., "EnvStation").</li>
                <li><strong>Important:</strong> Check only the gist scope box.</li>
                <li>Generate the token and paste it here.</li>
              </ol>
              <p style={{ marginTop: 8 }}><strong>🔒 Security Guarantee:</strong> Your token is stored strictly locally on your machine. EnvStation only uses it to communicate directly with the official GitHub API. It is never sent to any third-party servers, and because of the restricted scope, it cannot access your private repositories. You can verify this by checking the source code.</p>
            </div>
            <div className="footer">
              <button onClick={() => setInfoOpen(false)} style={{ background: '#374151', color: '#fff', border: 'none', padding: '8px 12px', borderRadius: 8, cursor: 'pointer' }}>Close</button>
            </div>
          </div>
        </div>
      )}

    </section>
  );
}
