/**
 * src/components/ManagePackagesModal.tsx
 *
 * Modal UI to view and install system packages declared in an environment's
 * manifest. This component delegates package installation to the Rust backend
 * via Tauri IPC and displays backend-provided manifest data.
 *
 * Props:
 * - isOpen: whether the modal is visible
 * - onClose: callback to request closing the modal
 * - environmentName: environment identifier used when instructing the backend
 * - projectPath: absolute host path to the project (may be null)
 *
 * IPC contract (commands used):
 * - "get_environment_manifest": invoke with { projectPath } and expect an
 *    EnvironmentManifest object shaped as { version, name, stack, system_packages }.
 * - "install_system_package": invoke with { name: environmentName, projectPath, package }
 *    The backend is responsible for performing the installation and returning
 *    a success/failure result (errors are bubbled to the UI). The UI attempts
 *    to extract a useful error message from the thrown payload for display.
 */

import { useEffect, useState, useCallback } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";

export interface EnvironmentManifest {
  version: string;
  name: string;
  stack: string;
  system_packages: string[];
}

interface Props {
  isOpen: boolean;
  onClose: () => void;
  environmentName: string;
  projectPath: string | null;
}

export default function ManagePackagesModal({ isOpen, onClose, environmentName, projectPath }: Props) {
  // Hooks must be declared unconditionally at the top to satisfy React Rules of Hooks
  const [loading, setLoading] = useState(false);
  const [packages, setPackages] = useState<string[]>([]);
  const [newPkg, setNewPkg] = useState("");

  const [driftScanning, setDriftScanning] = useState(false);
  const [driftContainerPackages, setDriftContainerPackages] = useState<string[]>([]);
  const [driftDevcontainerPackages, setDriftDevcontainerPackages] = useState<string[]>([]);
  const [selectedDriftContainer, setSelectedDriftContainer] = useState<string[]>([]);
  const [selectedDriftDevcontainer, setSelectedDriftDevcontainer] = useState<string[]>([]);
  const [fallbackUsed, setFallbackUsed] = useState(false);

  const [show, setShow] = useState(false);
  const [closing, setClosing] = useState(false);

  const loadManifest = useCallback(async () => {
    if (!projectPath) return;
    setLoading(true);
    try {
      const manifest = await invoke<EnvironmentManifest>("get_environment_manifest", { projectPath });
      setPackages(Array.isArray(manifest.system_packages) ? manifest.system_packages : []);
    } catch (e) {
      console.error(e);
      toast.error("Failed to load manifest");
    } finally {
      setLoading(false);
    }
  }, [projectPath]);

  // Keep the effect unconditional; it will react to changes of isOpen/projectPath
  useEffect(() => {
    if (isOpen) {
      setNewPkg("");
      setPackages([]);
      if (projectPath) {
        loadManifest();
      }
    }
  }, [isOpen, projectPath, loadManifest]);

  // Show animation: trigger when modal is opened
  useEffect(() => {
    if (isOpen) {
      requestAnimationFrame(() => setShow(true));
    } else {
      setShow(false);
      setClosing(false);
    }
  }, [isOpen]);

  const onInstall = useCallback(async () => {
    const pkg = (newPkg || "").trim();
    if (!pkg) return;
    if (!projectPath) {
      toast.error("Project path unknown");
      return;
    }

    // Basic client-side validation: allow letters, numbers and a few common package-name characters
    // Adjust the regex if you need to support additional characters for other package systems
    if (!/^[A-Za-z0-9+_.:-]{1,200}$/.test(pkg)) {
      toast.error("Invalid package name (contains forbidden characters)");
      return;
    }

    toast.info(`Installing package '${pkg}' in Bazzite and VS Code… Please wait.`);
    try {
      await invoke("install_system_package", { name: environmentName, projectPath, package: pkg });
      toast.success(`Package '${pkg}' installed`);
      setNewPkg("");
      await loadManifest();
    } catch (e) {
      // Try to extract a useful error message returned from the backend (package manager output)
      let msg = "Installation failed";
      try {
        if (e && typeof e === "object") {
          // common shapes: { message: "..." } or Tauri error payloads
          msg = (e as any)?.message || (e as any)?.payload?.message || JSON.stringify(e);
        } else {
          msg = String(e);
        }
      } catch {
        msg = String(e);
      }
      toast.error(msg || "Installation failed");
    }
  }, [newPkg, environmentName, projectPath, loadManifest]);

  const startClose = useCallback(() => {
    if (closing) return;
    setClosing(true);
    setTimeout(() => onClose(), 220);
  }, [closing, onClose]);

  // After all hooks are declared, we may early-return if the modal isn't open
  if (!isOpen) return null;

  // Render modal via a portal to document.body so it's not affected by parent layout/styling
  // (fixed positioning can be influenced by transformed ancestors when the modal is nested).
  const modalContent = (
    <div className={`modal-backdrop ${show ? "show" : ""} ${closing ? "closing" : ""}`} data-tauri-drag-region="none">
      <div className="modal-card" style={{ width: 520, maxWidth: "90%" }} data-tauri-drag-region="none">
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 8 }}>
          <strong>📦 Manage packages – {environmentName}</strong>
          <button onClick={startClose} style={{ background: "transparent", border: "none", color: "#9ca3af", fontSize: 18, cursor: "pointer" }} data-tauri-drag-region="none">✕</button>
        </div>

        <div style={{ marginBottom: 12, fontSize: 12, color: "#9ca3af" }}>
          {projectPath ? <span>Project path: {projectPath}</span> : <span>Project path unknown</span>}
        </div>

        <div style={{ display: "flex", gap: 8, marginBottom: 12 }}>
          <input
            type="text"
            placeholder="e.g. htop"
            value={newPkg}
            onChange={(e) => setNewPkg(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter") onInstall(); }}
            style={{ flex: 1, padding: 8, borderRadius: 6, border: "1px solid #374151", background: "#0b1220", color: "#e5e7eb" }}
            disabled={loading}
            data-tauri-drag-region="none"
            onMouseDown={(e) => { e.stopPropagation(); }}
            onPointerDown={(e) => { e.stopPropagation(); }}
          />
          <button onClick={onInstall} disabled={loading || !newPkg.trim()} style={{ padding: "8px 12px" }}>Install</button>
          <button onClick={async () => {
            if (!projectPath) { toast.error("Project path unknown"); return; }
            setDriftScanning(true);
            setDriftContainerPackages([]);
            setDriftDevcontainerPackages([]);
            setFallbackUsed(false);
            try {
              type DriftResult = { new_in_container: string[]; new_in_devcontainer: string[]; fallback_used?: boolean };
              const res = await invoke<DriftResult>("detect_environment_drift", { name: environmentName, projectPath });
              const container = Array.isArray(res?.new_in_container) ? res.new_in_container : [];
              const dev = Array.isArray(res?.new_in_devcontainer) ? res.new_in_devcontainer : [];
              setDriftContainerPackages(container);
              setDriftDevcontainerPackages(dev);
              setSelectedDriftContainer([]);
              setSelectedDriftDevcontainer([]);
              setFallbackUsed(Boolean((res as any)?.fallback_used));
              const total = container.length + dev.length;
              if (total === 0) {
                toast.success("No drift detected");
              } else {
                toast.info(`Detected ${total} drifted package(s)`);
              }
            } catch (e) {
              console.error(e);
              toast.error("Failed to scan for drift");
            } finally {
              setDriftScanning(false);
            }
          }} disabled={driftScanning} style={{ padding: "8px 12px" }}>{driftScanning ? "Scanning…" : "Scan for Drift"}</button>
        </div>


        <div style={{ maxHeight: 280, overflow: "auto", borderTop: "1px solid #1f2937", paddingTop: 8 }}>
          {loading ? (
            <div style={{ color: "#9ca3af", fontSize: 14 }}>Loading…</div>
          ) : packages.length === 0 ? (
            <div style={{ color: "#9ca3af", fontSize: 14 }}>No packages installed yet.</div>
          ) : (
            <ul style={{ margin: 0, padding: 0, listStyle: "none", display: "flex", flexWrap: "wrap", gap: 8 }}>
              {packages.map((p) => (
                <li key={p} style={{ background: "#0b1220", border: "1px solid #1f2937", padding: "6px 10px", borderRadius: 999, fontSize: 12 }}>{p}</li>
              ))}
            </ul>
          )}

          {/* Drift results */}
          {(driftContainerPackages && driftContainerPackages.length > 0) || (driftDevcontainerPackages && driftDevcontainerPackages.length > 0) ? (
            <div style={{ marginTop: 12 }}>
              {fallbackUsed && (
                <div style={{ display: 'flex', gap: 8, alignItems: 'flex-start', background: 'rgba(245,158,11,0.08)', border: '1px solid #f59e0b', color: '#92400e', padding: 12, borderRadius: 6, marginBottom: 12 }}>
                  <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="#f59e0b" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"></path><line x1="12" y1="9" x2="12" y2="13"></line><line x1="12" y1="17" x2="12.01" y2="17"></line></svg>
                  <div style={{ color: '#92400e', fontSize: 13 }}>
                    <strong>Warning:</strong> Primary package query failed. A fallback was used, so the list below may include automatically installed dependencies.
                  </div>
                </div>
              )}
              {driftContainerPackages && driftContainerPackages.length > 0 && (
                <div style={{ marginBottom: 8 }}>
                  <div style={{ color: "#9ca3af", marginBottom: 8 }}>New in Container (installed in Distrobox, missing from manifest):</div>
                  <ul style={{ margin: 0, padding: 0, listStyle: "none", display: "flex", flexDirection: "column", gap: 6 }}>
                    {driftContainerPackages.map((p) => (
                      <li key={p} style={{ display: "flex", alignItems: "center", gap: 8 }}>
                        <input type="checkbox" checked={selectedDriftContainer.includes(p)} onChange={() => {
                          setSelectedDriftContainer((prev) => prev.includes(p) ? prev.filter(x=>x!==p) : [...prev, p]);
                        }} />
                        <span style={{ background: "#0b1220", border: "1px solid #1f2937", padding: "6px 10px", borderRadius: 999, fontSize: 12 }}>{p}</span>
                      </li>
                    ))}
                  </ul>
                </div>
              )}

              {driftDevcontainerPackages && driftDevcontainerPackages.length > 0 && (
                <div style={{ marginBottom: 8 }}>
                  <div style={{ color: "#9ca3af", marginBottom: 8 }}>New in DevContainer (declared in .devcontainer, missing from manifest):</div>
                  <ul style={{ margin: 0, padding: 0, listStyle: "none", display: "flex", flexDirection: "column", gap: 6 }}>
                    {driftDevcontainerPackages.map((p) => (
                      <li key={p} style={{ display: "flex", alignItems: "center", gap: 8 }}>
                        <input type="checkbox" checked={selectedDriftDevcontainer.includes(p)} onChange={() => {
                          setSelectedDriftDevcontainer((prev) => prev.includes(p) ? prev.filter(x=>x!==p) : [...prev, p]);
                        }} />
                        <span style={{ background: "#0b1220", border: "1px solid #1f2937", padding: "6px 10px", borderRadius: 999, fontSize: 12 }}>{p}</span>
                      </li>
                    ))}
                  </ul>
                </div>
              )}

              <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 8 }}>
                <button onClick={async () => {
                  if (!projectPath) { toast.error("Project path unknown"); return; }
                  const toAdopt = [...selectedDriftContainer, ...selectedDriftDevcontainer];
                  if (!toAdopt || toAdopt.length === 0) { toast.error("No packages selected"); return; }
                  toast.info(`Adopting ${toAdopt.length} package(s)…`);
                  for (const pkg of toAdopt) {
                    try {
                      // eslint-disable-next-line no-await-in-loop
                      await invoke("install_system_package", { name: environmentName, projectPath, package: pkg });
                      toast.success(`Adopted ${pkg}`);
                    } catch (e) {
                      console.error(e);
                      toast.error(`Failed to adopt ${pkg}`);
                    }
                  }
                  await loadManifest();
                  setDriftContainerPackages([]);
                  setDriftDevcontainerPackages([]);
                  setSelectedDriftContainer([]);
                  setSelectedDriftDevcontainer([]);
                  setFallbackUsed(false);
                }}>Adopt selected</button>
                <button onClick={() => { setDriftContainerPackages([]); setDriftDevcontainerPackages([]); setSelectedDriftContainer([]); setSelectedDriftDevcontainer([]); setFallbackUsed(false); }}>Dismiss</button>
                <button onClick={async () => {
                  // Sync All: adopt all detected items
                  if (!projectPath) { toast.error("Project path unknown"); return; }
                  const all = Array.from(new Set([...(driftContainerPackages||[]), ...(driftDevcontainerPackages||[])]));
                  if (all.length === 0) { toast.info("Nothing to sync"); return; }
                  toast.info(`Syncing ${all.length} package(s)…`);
                  for (const pkg of all) {
                    try {
                      // eslint-disable-next-line no-await-in-loop
                      await invoke("install_system_package", { name: environmentName, projectPath, package: pkg });
                      toast.success(`Synced ${pkg}`);
                    } catch (e) {
                      console.error(e);
                      toast.error(`Failed to sync ${pkg}`);
                    }
                  }
                  await loadManifest();
                  setDriftContainerPackages([]);
                  setDriftDevcontainerPackages([]);
                  setSelectedDriftContainer([]);
                  setSelectedDriftDevcontainer([]);
                  setFallbackUsed(false);
                }}>Sync All</button>
              </div>
            </div>
          ) : null}

        </div>

        <div style={{ display: "flex", justifyContent: "flex-end", marginTop: 12 }}>
          <button onClick={startClose} style={{ padding: "8px 12px" }}>Close</button>
        </div>
      </div>
    </div>
  );

  // Render into the app's .layout element so the modal backdrop is clipped by the rounded window
  if (typeof document !== "undefined") {
    const host = document.querySelector('.layout') || document.body;
    return createPortal(modalContent, host);
  }

  return modalContent; // fallback
}
