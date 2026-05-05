/**
 * src/components/EnvironmentRow.tsx
 *
 * Row component that renders a single managed environment (container) and
 * exposes controls to start/stop/delete the container, open it in VS Code and
 * calculate/manage disk usage related to the associated project.
 *
 * Props:
 * - env: basic environment metadata (name, image, status, container_id)
 * - base: optional precomputed space information provided by the backend
 *   (project_path, project_bytes, container_size_rw). When project_bytes is
 *   present we seed the SpaceCache to avoid unnecessary recalculation.
 * - onOpenVSCode(name): callback to open the given environment in VS Code
 * - onDelete(name): callback to initiate deletion flow for the environment
 *
 * IPC and backend interactions (commands used and expected payloads):
 * - "resolve_project_path" invoked with { name } and expected to return
 *   string | null (path to the project on the host) useful to seed the
 *   SpaceCache and calculate project size.
 * - "start_environment" invoked with { name } and expected to return a
 *   short message string on success. The UI sends client-side logs via
 *   "client_log" before/after the operation; client_log payload shape:
 *   { source: 'ui', level: 'INFO'|'ERROR', message: string }.
 * - "stop_environment" invoked with { name } with the same expectations as
 *   start_environment.
 *
 * The component uses the SpaceCache context (see src/context/SpaceCacheContext)
 * to request and coalesce project size calculations. When a size is missing it
 * calls requestSize(path) which triggers a backend calculation via
 * src/utils/dirSizeQueue.requestDirSize and the result is delivered through
 * a "size-update" event emitted by the backend.
 */

import { useEffect, useMemo, useCallback, memo, useState, useRef } from "react";
import { useSpaceCache } from "../context/SpaceCacheContext";
import { useEnvironments } from "../context/EnvironmentsContext";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import ManagePackagesModal from "./ManagePackagesModal";

export interface EnvironmentInfo {
  name: string;
  image: string;
  status: string;
  container_id: string;
}

export interface EnvironmentSpaceInfo {
  project_path: string | null;
  project_bytes: number | null;
  container_size_rw: number | null;
}

interface Props {
  env: EnvironmentInfo;
  base: EnvironmentSpaceInfo | undefined;
  onOpenVSCode: (name: string) => void;
  onDelete: (name: string) => void;
}

function EnvironmentRowImpl({ env, base, onOpenVSCode, onDelete }: Props) {
  const { getCachedSize, isScanning, requestSize, setCachedSize } = useSpaceCache();
  const { refresh } = useEnvironments();
  const [resolvedPath, setResolvedPath] = useState<string | null>(base?.project_path ?? null);
  const [menuOpen, setMenuOpen] = useState(false);
  const [manageOpen, setManageOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement | null>(null);
  const path = resolvedPath;

  // Close menu on outside click
  useEffect(() => {
    if (!menuOpen) return;
    const onDocClick = (e: MouseEvent) => {
      const t = e.target as Node | null;
      if (containerRef.current && t && !containerRef.current.contains(t)) {
        setMenuOpen(false);
      }
    };
    document.addEventListener("mousedown", onDocClick);
    return () => document.removeEventListener("mousedown", onDocClick);
  }, [menuOpen]);

  // Seed cache from backend-provided project_bytes if present
  useEffect(() => {
    if (path && base?.project_bytes != null) {
      setCachedSize(path, base.project_bytes);
    }
  }, [path, base?.project_bytes, setCachedSize]);

  useEffect(() => {
    setResolvedPath(base?.project_path ?? null);
  }, [base?.project_path]);

  const cached = useMemo(() => (path ? getCachedSize(path) : null), [path, getCachedSize]);
  const pending = path ? isScanning(path) : false;

  const [resolving, setResolving] = useState(false);
  const triggerScan = useCallback(async () => {
    toast.info(`Calculating project size…`);
    if (!path) {
      setResolving(true);
      try {
        const p = await invoke<string | null>("resolve_project_path", { name: env.name });
        if (p) {
          setResolvedPath(p);
          await requestSize(p);
        } else {
          toast.error("Could not determine project path");
        }
      } finally {
        setResolving(false);
      }
    } else {
      await requestSize(path);
    }
  }, [path, env.name, requestSize]);

  const fmtSize = useCallback((n?: number | null) => {
    if (n == null) return "?";
    const units = ["B", "KB", "MB", "GB", "TB"]; let i = 0; let v = n as number;
    while (v >= 1024 && i < units.length - 1) { v /= 1024; i++; }
    return `${v.toFixed(1)} ${units[i]}`;
  }, []);

  const statusL = (env.status || "").toLowerCase();
  const isRunning = statusL.includes("up");
  const isStopped = statusL.includes("exited") || statusL.includes("stopped") || statusL.includes("created") || statusL.includes("down");

  const toErrMsg = (e: unknown) => {
    if (e instanceof Error) return e.message || `${e.name}: ${e.message}`;
    try {
      const s = JSON.stringify(e);
      if (s && s !== "{}") return s;
    } catch {}
    return String(e ?? "Unknown error");
  };

  const handleStart = useCallback(async () => {
    setMenuOpen(false);
    toast.info("Starting container…");
    await invoke("client_log", { source: "ui", level: "INFO", message: `start_environment requested for '${env.name}'` }).catch(() => {});
    try {
      const backendMsg = await invoke<string>("start_environment", { name: env.name });
      toast.success(`Environment '${env.name}' started successfully`);
      await invoke("client_log", { source: "ui", level: "INFO", message: `start_environment ok for '${env.name}': ${backendMsg}` }).catch(() => {});
      await refresh();
    } catch (e) {
      const msg = toErrMsg(e);
      await invoke("client_log", { source: "ui", level: "ERROR", message: `start_environment failed for '${env.name}': ${msg}` }).catch(() => {});
      toast.error(msg || "Start failed");
    }
  }, [env.name, refresh]);

  const handleStop = useCallback(async () => {
    setMenuOpen(false);
    toast.info("Stopping container…");
    await invoke("client_log", { source: "ui", level: "INFO", message: `stop_environment requested for '${env.name}'` }).catch(() => {});
    try {
      const backendMsg = await invoke<string>("stop_environment", { name: env.name });
      toast.success(`Environment '${env.name}' stopped successfully`);
      await invoke("client_log", { source: "ui", level: "INFO", message: `stop_environment ok for '${env.name}': ${backendMsg}` }).catch(() => {});
      await refresh();
    } catch (e) {
      const msg = toErrMsg(e);
      await invoke("client_log", { source: "ui", level: "ERROR", message: `stop_environment failed for '${env.name}': ${msg}` }).catch(() => {});
      toast.error(msg || "Stop failed");
    }
  }, [env.name, refresh]);

  const handleDelete = useCallback(() => {
    setMenuOpen(false);
    onDelete(env.name);
  }, [env.name, onDelete]);

  const handleManagePackages = useCallback(async () => {
    setMenuOpen(false);
    if (!resolvedPath) {
      try {
        const p = await invoke<string | null>("resolve_project_path", { name: env.name });
        if (p) setResolvedPath(p);
        else {
          toast.error("Could not determine project path");
          return;
        }
      } catch (e) {
        toast.error("Project path resolution failed");
        return;
      }
    }
    setManageOpen(true);
  }, [resolvedPath, env.name]);

  const handleOpenTerminal = useCallback(async () => {
    toast.info("Opening terminal…");
    await invoke("client_log", { source: "ui", level: "INFO", message: `open_in_terminal requested for '${env.name}'` }).catch(() => {});
    try {
      const backendMsg = await invoke<string>("open_in_terminal", { envName: env.name });
      toast.success("Terminal launched");
      await invoke("client_log", { source: "ui", level: "INFO", message: `open_in_terminal ok for '${env.name}': ${backendMsg}` }).catch(() => {});
    } catch (e) {
      const msg = toErrMsg(e);
      await invoke("client_log", { source: "ui", level: "ERROR", message: `open_in_terminal failed for '${env.name}': ${msg}` }).catch(() => {});
      toast.error(msg || "Open terminal failed");
    }
  }, [env.name]);

  return (
    <div ref={containerRef} style={{
      position: "relative",
      padding: 12,
      background: "#222",
      borderRadius: 8,
      border: "1px solid #444",
      textAlign: "left",
    }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
        <strong>🧪 {env.name}</strong>
        <button
          onClick={() => setMenuOpen((v) => !v)}
          title="Menu"
          aria-label="Actions"
          style={{
            background: "transparent",
            border: "none",
            color: "#aaa",
            fontSize: 18,
            lineHeight: 1,
            padding: "2px 6px",
            borderRadius: 6,
            cursor: "pointer",
          }}
        >
          ⋮
        </button>
      </div>

      {menuOpen && (
        <div
          style={{
            position: "absolute",
            top: 36,
            right: 8,
            zIndex: 1000,
            background: "#1f2937",
            border: "1px solid #374151",
            borderRadius: 8,
            minWidth: 180,
            boxShadow: "0 8px 24px rgba(0,0,0,0.5)",
            overflow: "hidden",
          }}
        >
          <button
            onClick={handleStart}
            disabled={isRunning}
            style={{
              display: "flex",
              width: "100%",
              gap: 8,
              alignItems: "center",
              background: "transparent",
              border: "none",
              color: isRunning ? "#6b7280" : "#e5e7eb",
              padding: "10px 12px",
              cursor: isRunning ? "not-allowed" : "pointer",
              opacity: isRunning ? 0.6 : 1,
            }}
          >
            ▶ Start
          </button>
          <button
            onClick={handleStop}
            disabled={isStopped}
            style={{
              display: "flex",
              width: "100%",
              gap: 8,
              alignItems: "center",
              background: "transparent",
              border: "none",
              color: isStopped ? "#6b7280" : "#e5e7eb",
              padding: "10px 12px",
              cursor: isStopped ? "not-allowed" : "pointer",
              opacity: isStopped ? 0.6 : 1,
            }}
          >
            ■ Stop
          </button>
          <button
            onClick={handleManagePackages}
            style={{
              display: "flex",
              width: "100%",
              gap: 8,
              alignItems: "center",
              background: "transparent",
              border: "none",
              color: "#e5e7eb",
              padding: "10px 12px",
              cursor: "pointer",
            }}
          >
            📦 Manage packages
          </button>
          <div style={{ height: 1, background: "#374151" }} />
          <button
            onClick={handleDelete}
            style={{
              display: "flex",
              width: "100%",
              gap: 8,
              alignItems: "center",
              background: "transparent",
              border: "none",
              color: "#ef4444",
              padding: "10px 12px",
              cursor: "pointer",
            }}
          >
            🗑️ Delete
          </button>
        </div>
      )}

      <div style={{ fontSize: 12, color: "#aaa", display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap", marginTop: 6 }}>
        <span>Status: {env.status} · Image: {env.image}</span>
        <span>
          · Space: {cached != null ? fmtSize(cached) : (pending ? <span className="spinner" /> : "?")} (project)
          {base?.container_size_rw != null ? ` + ${fmtSize(base.container_size_rw)} (container)` : ""}
        </span>
        <button onClick={triggerScan} disabled={pending || resolving} style={{ padding: "2px 6px", fontSize: 12 }} title="Calculate/update size">
          {pending || resolving ? "Calculating…" : (cached != null ? "Refresh" : "Calculate")}
        </button>
      </div>
      <div style={{ display: "flex", gap: 8, marginTop: 8 }}>
        <button onClick={() => onOpenVSCode(env.name)} style={{ flex: 1 }}>
          💻 Open in VS Code (remote)
        </button>
        <button onClick={handleOpenTerminal} style={{ flex: 1, background: '#444', color: '#e5e7eb', border: 'none', borderRadius: 6, padding: '8px 10px' }}>
          🖥️ Open in Terminal
        </button>
      </div>

      <ManagePackagesModal
        isOpen={manageOpen}
        onClose={() => setManageOpen(false)}
        environmentName={env.name}
        projectPath={resolvedPath ?? null}
      />
    </div>
  );
}

const EnvironmentRow = memo(EnvironmentRowImpl);
export default EnvironmentRow;
