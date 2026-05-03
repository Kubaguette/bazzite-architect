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

  return (
    <section>
      <h1>Settings</h1>

      <fieldset>
        <legend>System Check</legend>
        <button onClick={runSystemCheck}>Check</button>
        {msg && <p className="status-box">{msg}</p>}
        {system && (
          <div className="status-box" style={{ textAlign: "left" }}>
            <p>
              Podman: {system.podman_ok ? "✅" : "❌"}
              {system.podman_version ? ` – ${system.podman_version}` : ""}
            </p>
            <p>
              Distrobox: {system.distrobox_ok ? "✅" : "❌"}
              {system.distrobox_version ? ` – ${system.distrobox_version}` : ""}
            </p>
          </div>
        )}
      </fieldset>

      <fieldset style={{ marginTop: 16 }}>
        <legend>Advanced</legend>
        <label>
          <input
            type="checkbox"
            checked={advanced}
            onChange={(e) => setAdvanced(e.target.checked)}
          />{" "}
          Show Advanced Mode (Logs in the sidebar)
        </label>
      </fieldset>
    </section>
  );
}
