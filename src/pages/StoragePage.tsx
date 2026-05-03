/**
 * src/pages/StoragePage.tsx
 *
 * Storage management UI. Interacts with the backend through several Tauri
 * commands to scan available drives, read/apply the active storage path, and
 * update storage configuration.
 *
 * Commands used:
 * - "scan_drives": returns DriveInfo[]
 * - "get_active_storage_path": returns string
 * - "apply_storage_setup": invoked with { targetPath } and returns a message
 * - "client_log": used to forward backend messages to the central logs
 */

import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useBusy } from "../context/BusyContext";
import { toast } from "sonner";

interface DriveInfo {
  name: string;
  file_system: string;
  mount_point: string;
  available_gb: number;
  total_gb: number;
}

const normalizePath = (p: string) => p.replace("/var/home", "/home").replace(/\/$/, "");
const isActivePath = (drivePath: string, activePath: string) => {
  if (!drivePath || !activePath) return false;
  const nDrive = normalizePath(drivePath);
  const nActive = normalizePath(activePath);
  return nActive === nDrive || nActive.startsWith(nDrive + "/");
};

export default function StoragePage() {
  const [drivesList, setDrivesList] = useState<DriveInfo[] | null>(null);
  const [activeStoragePath, setActiveStoragePath] = useState<string>("");
  const [storageMsg, setStorageMsg] = useState<string>("");
  const [confirmTarget, setConfirmTarget] = useState<string | null>(null);
  const [confirmClosing, setConfirmClosing] = useState(false);
  const { startBusy, endBusy } = useBusy();

  const startConfirm = (doApply: boolean) => {
    if (!confirmTarget) return;
    if (confirmClosing) return;
    setConfirmClosing(true);
    const t = confirmTarget;
    setTimeout(() => {
      setConfirmClosing(false);
      setConfirmTarget(null);
      if (doApply) proceedApplyStorage(t);
    }, 220);
  };

  const fetchActiveStorage = async () => {
    startBusy();
    try {
      const path = await invoke<string>("get_active_storage_path");
      setActiveStoragePath(path);
    } catch (e) {
      setActiveStoragePath("");
    } finally {
      endBusy();
    }
  };

  const scanDrives = async () => {
    setStorageMsg("");
    startBusy();
    try {
      const list = await invoke<DriveInfo[]>("scan_drives");
      setDrivesList(list);
      await fetchActiveStorage();
    } catch (e) {
      setStorageMsg(`Scan error: ${String(e)}`);
      setDrivesList(null);
    } finally {
      endBusy();
    }
  };

  const proceedApplyStorage = async (mountPoint: string) => {
    startBusy();
    setStorageMsg("Updating storage configuration...");

    try {
      const msg = await invoke<string>("apply_storage_setup", { targetPath: mountPoint });
      // Log detailed backend response to Logs page and show a minimal confirmation here
      try { await invoke("client_log", { source: "ui", level: "INFO", message: msg }); } catch {}
      // Update local status box
      setStorageMsg("Configuration updated.");
      await fetchActiveStorage();

      // Show a toast to inform the user. Use German if the user's language is German, otherwise English.
      const isGerman = typeof navigator !== "undefined" && navigator.language?.startsWith("de");
      if (isGerman) {
        toast.success("Neuer Speicherort gesetzt! Hinweis: Bereits existierende Container wurden nicht verschoben. Neue Umgebungen werden ab sofort hier gespeichert.");
      } else {
        toast.success("New storage location set! Note: Existing containers were not moved. New environments will be stored here from now on.");
      }
    } catch (e) {
      const errMsg = String(e);
      setStorageMsg(`Error: ${errMsg}`);
      try { await invoke("client_log", { source: "ui", level: "ERROR", message: `apply_storage_setup failed for '${mountPoint}': ${errMsg}` }); } catch {}
      // Also show an error toast
      toast.error(errMsg || "Storage update failed");
    } finally {
      endBusy();
    }
  };

  return (
    <section>
      <h1>Storage</h1>
      <div style={{ display: "flex", gap: 8 }}>
        <button onClick={scanDrives}>Scan drives</button>
        <button onClick={fetchActiveStorage}>Read active storage location</button>
      </div>
      {activeStoragePath && (
        <p style={{ marginTop: 6 }}>
          Active GraphRoot: <code>{activeStoragePath}</code>
        </p>
      )}
      {storageMsg && (
        <div className="status-box" style={{ marginTop: 10 }}>
          <p>{storageMsg}</p>
        </div>
      )}

      <div style={{ marginTop: 12, display: "flex", flexDirection: "column", gap: 12 }}>
        {drivesList ? (
          drivesList.map((d, i) => (
            <div key={i} style={{
              padding: 12,
              background: "#222",
              borderRadius: 8,
              border: "1px solid #444",
              textAlign: "left",
            }}>
              <strong>📍 {d.mount_point}</strong>
              <div style={{ fontSize: 12, color: "#aaa" }}>
                {d.name} ({d.file_system}) – {d.available_gb} GB free of {d.total_gb} GB
              </div>
              {isActivePath(d.mount_point, activeStoragePath) ? (
                <div style={{
                  marginTop: 8,
                  padding: 8,
                  background: "#064e3b",
                  color: "#34d399",
                  borderRadius: 6,
                  border: "1px solid #059669",
                  textAlign: "center",
                }}>
                  ✅ Active Podman storage
                </div>
              ) : (
                <button
                  onClick={() => setConfirmTarget(d.mount_point)}
                  style={{ marginTop: 8, width: "100%" }}
                >
                  🚀 Configure storage here
                </button>
              )}
            </div>
          ))
        ) : (
          <p style={{ color: "#888" }}>Click Scan to load drives.</p>
        )}
      </div>
      {confirmTarget && (
        <div className={`modal-backdrop ${confirmClosing ? "closing" : "show"}`}>
          <div className="modal-card" style={{ maxWidth: 460 }}>
            <header>Configure storage here?</header>
            <div className="body">
              <p>
                Configure storage location? Existing containers will not be migrated (MVP).
              </p>
              <div style={{ display: "flex", gap: 8, marginTop: 12 }}>
                <button onClick={() => startConfirm(false)} style={{ flex: 1, background: "#374151" }}>Cancel</button>
                <button onClick={() => startConfirm(true)} style={{ flex: 1 }}>Confirm</button>
              </div>
            </div>
          </div>
        </div>
      )}
    </section>
  );
}
