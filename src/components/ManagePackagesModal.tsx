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
