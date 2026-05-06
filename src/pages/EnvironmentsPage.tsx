/**
 * src/pages/EnvironmentsPage.tsx
 *
 * Page that lists managed environments and provides per-environment actions
 * (start/stop/delete, open in VS Code, manage packages). The page delegates
 * environment listing to EnvironmentsContext and uses the SpaceCacheContext to
 * manage project size calculations. All heavy operations are performed in the
 * backend via Tauri invoke commands; UI shows progress via BusyContext.
 *
 * Important backend commands used here:
 * - "system_check": returns diagnostic info { podman_ok, distrobox_ok, ... }
 * - "open_in_vscode": invoked with { name } and expected to return a short
 *    success message string
 * - "delete_environment": invoked with { request: { name, deleteProject } }
 * - "client_log": helper command used to log UI-originated events on the backend
 */

import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useBusy } from "../context/BusyContext";
import { useEnvironments } from "../context/EnvironmentsContext";
import EnvironmentRow from "../components/EnvironmentRow";
import PageHeader from "../components/PageHeader";
import { useSpaceCache } from "../context/SpaceCacheContext";

interface EnvironmentSpaceInfo {
  project_path: string | null;
  project_bytes: number | null;
  container_size_rw: number | null;
}

export default function EnvironmentsPage() {
  const { envs, loading: envsLoading, refresh } = useEnvironments();
  const [envMsg, setEnvMsg] = useState<string>("");
  const [baseByEnv, setBaseByEnv] = useState<Record<string, EnvironmentSpaceInfo>>({});
  const [deletePromptName, setDeletePromptName] = useState<string | null>(null);
  const [deleteClosing, setDeleteClosing] = useState(false);
  const { startBusy, endBusy } = useBusy();
  const space = useSpaceCache();

  const startDelete = (choice: boolean) => {
    if (!deletePromptName) return;
    if (deleteClosing) return;
    setDeleteClosing(true);
    const name = deletePromptName;
    setTimeout(() => {
      setDeleteClosing(false);
      doDelete(name, choice);
    }, 220);
  };


  useEffect(() => {
    // Log page open
    void invoke("client_log", { source: "ui", level: "INFO", message: "Environments page opened" });
    // On first visit, if nothing has been loaded yet, load once
    if (envs.length === 0 && !envsLoading) {
      void refresh();
    }
    // Cleanup: cancel any ongoing dir size work when leaving this page
    return () => { void space.cancelAll(); };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    // No automatic base scans – keep the page IO-free
    setBaseByEnv({});
  }, [envs]);


  const openVSCode = async (name: string) => {
    const dbg: string[] = [];
    const fmtErr = (e: unknown) => {
      try {
        if (e instanceof Error) return `${e.name}: ${e.message}`;
        return JSON.stringify(e);
      } catch {
        return String(e);
      }
    };

    dbg.push(`→ prepare: name='${name}'`);
    startBusy();
    try {
      try {
        const sys: any = await invoke("system_check");
        dbg.push(`system_check ok: podman_ok=${sys.podman_ok} distrobox_ok=${sys.distrobox_ok}`);
        await invoke("client_log", { source: "ui", level: "INFO", message: `Open VS Code for '${name}'` });
      } catch (e) {
        const msg = fmtErr(e);
        dbg.push(`system_check failed: ${msg}`);
        setEnvMsg(`❌ Backend not reachable or outdated. Details:\n${dbg.join("\n")}`);
        return;
      }

      try {
        const out = await invoke<string>("open_in_vscode", { name });
        dbg.push(`open_in_vscode ok: ${out}`);
        // Log the exact success message to the central Logs page and do NOT show it here
        await invoke("client_log", { source: "ui", level: "INFO", message: out });
      } catch (e) {
        const msg = fmtErr(e);
        dbg.push(`open_in_vscode failed: ${msg}`);
        await invoke("client_log", { source: "ui", level: "ERROR", message: `open_in_vscode failed: ${msg}` });
        // Show a brief error on the Environments page without the large debug dump
        setEnvMsg(`❌ Failed to open: ${msg}`);
      }
    } finally {
      endBusy();
    }
  };

  const deleteEnv = async (name: string) => {
    // Avoid blocking browser dialogs inside the WebKit WebView
    setDeletePromptName(name);
  };

  const doDelete = async (name: string, deleteProject: boolean) => {
    setDeletePromptName(null);
    startBusy();
    setEnvMsg(`Deleting '${name}'...`);
    try {
      const msg = await invoke<string>("delete_environment", {
        request: { name, deleteProject },
      });
      // Log full backend message to Logs page and show a minimal confirmation here
      try { await invoke("client_log", { source: "ui", level: "INFO", message: msg }); } catch {}
      setEnvMsg(`'${name}' deleted.`);
      await refresh();
    } catch (e) {
      const errMsg = String(e);
      setEnvMsg(`❌ Error: ${errMsg}`);
      try { await invoke("client_log", { source: "ui", level: "ERROR", message: `delete_environment failed for '${name}': ${errMsg}` }); } catch {}
    } finally {
      endBusy();
    }
  };


  return (
    <section>
      <PageHeader title="Environments" actions={envsLoading ? <span className="spinner" title="Loading in background" /> : null} />

      {envMsg && (
        <div className="status-box" style={{ marginTop: 10 }}>
          <p>{envMsg}</p>
        </div>
      )}

      <div className="environments-grid">
        {envs.length > 0 ? (
          envs.map((env) => (
            <EnvironmentRow
              key={env.name}
              env={env}
              base={baseByEnv[env.name]}
              onOpenVSCode={openVSCode}
              onDelete={deleteEnv}
            />
          ))
        ) : envsLoading ? (
          <p style={{ color: "#888" }}>Loading environments... <span className="spinner" /></p>
        ) : (
          <p style={{ color: "#888" }}>No environments found.</p>
        )}
      </div>

      {deletePromptName && (
        <div className={`modal-backdrop ${deleteClosing ? "closing" : "show"}`}>
          <div className="modal-card" style={{ maxWidth: 420 }}>
            <header>Also delete project folder?</header>
            <div className="body">
              <p>
                Do you also want to remove the corresponding project folder?
                This deletes the folder on the host (e.g., $HOME/{deletePromptName} oder den gemappten Pfad).
              </p>
              <div style={{ display: "flex", gap: 8, marginTop: 12 }}>
                <button onClick={() => startDelete(false)} style={{ flex: 1, background: "#374151" }}>
                  DO NOT DELETE
                </button>
                <button onClick={() => startDelete(true)} style={{ flex: 1, background: "#7f1d1d" }}>
                  DELETE
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </section>
  );
}
