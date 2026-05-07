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

  // Design notes: follow a loose golden-ratio spacing system (base 12px, then ~20px gaps)
  const base = 12;
  const golden = Math.round(base * 1.618);

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
      </div>

    </section>
  );
}
