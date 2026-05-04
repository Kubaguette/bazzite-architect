/**
 * src/components/CreateEnvModal.tsx
 *
 * Modal used to create a new environment from a selected template. This
 * component coordinates with the backend via Tauri IPC and listens for
 * progress events to show live feedback.
 *
 * Props:
 * - onClose: called when the modal should be closed
 * - onCreated: optional callback invoked after successful creation
 * - defaultTemplate/defaultName/defaultHomeMount: optional defaults prefilled in the form
 *
 * IPC and event flow:
 * - The UI subscribes to a Tauri event named "creation-progress" which
 *   carries CreationProgressPayload objects:
 *     { stage: string, message: string, level: "info"|"error", done: boolean, success?: boolean }
 *   The backend is responsible for emitting these events to provide granular
 *   progress updates. The UI resolves the creation flow when a payload with
 *   done=true is received.
 * - When the user submits the form the UI:
 *    1) Calls "client_log" with a message about the requested creation. Payload: { source: 'ui', level, message }
 *    2) Invokes "create_environment" with payload: { request: { name, template, homeMount } }
 *   The backend should start the creation process and emit "creation-progress"
 *   events. The invoke to create_environment itself is fire-and-forget from the
 *   perspective of progress reporting - completion is communicated via events.
 */

import React, { useMemo, useRef, useEffect, useState, useCallback } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { useBusy } from "../context/BusyContext";

export type TemplateId = "react-ts" | "python" | "cpp" | "rust" | "java" | "csharp";

interface Props {
  onClose: () => void;
  onCreated?: () => void;
  defaultTemplate?: TemplateId;
  defaultName?: string;
  defaultHomeMount?: string;
}

interface CreationProgressPayload {
  stage: string;
  message: string;
  level: "info" | "error";
  done: boolean;
  success?: boolean | null;
}

const PROGRESS_EVENT = "creation-progress";

export default function CreateEnvModal({ onClose, onCreated, defaultTemplate, defaultName, defaultHomeMount }: Props) {
  const { startBusy, endBusy } = useBusy();
  const [name, setName] = useState<string>(defaultName ?? "");
  const [template, setTemplate] = useState<TemplateId>(defaultTemplate ?? "react-ts");
  const [homeMount, setHomeMount] = useState<string>(defaultHomeMount ?? "");
  const [err, setErr] = useState<string>("");
  const [submitting, setSubmitting] = useState<boolean>(false);
  const [progress, setProgress] = useState<CreationProgressPayload | null>(null);
  const [showProgress, setShowProgress] = useState<boolean>(false);
  const unlistenRef = useRef<UnlistenFn | null>(null);

  const nameRef = useRef<HTMLInputElement>(null);
  useEffect(() => {
    nameRef.current?.focus();
  }, []);

  useEffect(() => {
    if (defaultName) setName(defaultName);
    if (defaultTemplate) setTemplate(defaultTemplate);
    if (defaultHomeMount) setHomeMount(defaultHomeMount);
  }, [defaultName, defaultTemplate, defaultHomeMount]);

  useEffect(() => {
    return () => {
      unlistenRef.current?.();
      unlistenRef.current = null;
    };
  }, []);

  // animation states for entry/exit
  const [show, setShow] = useState(false);
  const [closing, setClosing] = useState(false);
  useEffect(() => {
    requestAnimationFrame(() => setShow(true));
  }, []);

  const canSubmit = useMemo(() => name.trim().length > 0 && !submitting, [name, submitting]);

  const handleClose = useCallback(() => {
    unlistenRef.current?.();
    unlistenRef.current = null;
    if (closing) return;
    setClosing(true);
    setTimeout(() => onClose(), 220);
  }, [onClose, closing]);

  const handleProgress = useCallback(
    (payload: CreationProgressPayload) => {
      setProgress(payload);
      if (payload.done) {
        setSubmitting(false);
        endBusy();
        unlistenRef.current?.();
        unlistenRef.current = null;
        if (payload.success) {
          onCreated?.();
          setTimeout(() => handleClose(), 500);
        } else {
          setErr(payload.message);
        }
      }
    },
    [endBusy, handleClose, onCreated]
  );

  const submit = async () => {
    setErr("");
    if (!name.trim()) {
      setErr("Please enter a name.");
      return;
    }
    const hm = homeMount.trim();
    if (hm && !(hm.startsWith("/") || hm.startsWith("$HOME/") || hm.startsWith("~/"))) {
      setErr("Path must be absolute (starting with '/', '$HOME/' or '~/'). Or leave empty for default: $HOME/" + name.trim());
      return;
    }

    setSubmitting(true);
    startBusy();

    try {
      const unlisten = await listen<CreationProgressPayload>(PROGRESS_EVENT, (event) => handleProgress(event.payload));
      unlistenRef.current = unlisten;
      setShowProgress(true);
      setProgress({
        stage: "init",
        message: "Preparing environment…",
        level: "info",
        done: false,
        success: null,
      });

      await invoke("client_log", { source: "ui", level: "INFO", message: `Create environment: name='${name.trim()}', template='${template}', home='${homeMount.trim() || "$HOME/<name>"}'` });

      await invoke("create_environment", {
        request: {
          name: name.trim(),
          template,
          homeMount: homeMount.trim() || null,
        },
      });
    } catch (e) {
      unlistenRef.current?.();
      unlistenRef.current = null;
      const errMsg = String(e);
      setProgress({
        stage: "error",
        message: errMsg,
        level: "error",
        done: true,
        success: false,
      });
      setShowProgress(true);
      setErr(errMsg);
      setSubmitting(false);
      endBusy();
      try {
        await invoke("client_log", { source: "ui", level: "ERROR", message: `create_environment failed: ${errMsg}` });
      } catch {}
    }
  };

  const focusFirstOnPanelClick: React.MouseEventHandler<HTMLDivElement> = (e) => {
    if (e.target === e.currentTarget) {
      nameRef.current?.focus();
    }
  };

  const dismissProgress = () => {
    setShowProgress(false);
    setProgress(null);
  };

  return (
    <div className={`modal-backdrop ${show ? "show" : ""} ${closing ? "closing" : ""}`} data-tauri-drag-region="none">
      <div className="modal-card" onMouseDown={focusFirstOnPanelClick} data-tauri-drag-region="none">
        <header>Create new environment</header>
        <div className="body" style={{ display: "flex", flexDirection: "column", gap: 12 }}>
          <label style={{ display: "flex", flexDirection: "column", gap: 4 }}>
            <span>Name</span>
            <input
              ref={nameRef}
              style={{ padding: "8px 10px", borderRadius: 6, border: "1px solid #374151", background: "#111827", color: "#e5e7eb" }}
              placeholder="e.g. my-project"
              value={name}
              onChange={(e) => setName(e.target.value)}
              data-tauri-drag-region="none"
              disabled={submitting}
              onMouseDown={(e) => { e.stopPropagation(); /* ensure clicks reach input even if something overlaps */ }}
              onPointerDown={(e) => { e.stopPropagation(); }}
            />
          </label>

          <fieldset style={{ border: "1px solid #374151", borderRadius: 8, padding: 10 }}>
            <legend>Stack</legend>
            <div style={{ display: "flex", gap: 12, flexWrap: "wrap" }}>
              <label style={{ display: "flex", alignItems: "center", gap: 6 }}>
                <input type="radio" name="template" value="react-ts" checked={template === "react-ts"} onChange={() => setTemplate("react-ts")} data-tauri-drag-region="none" disabled={submitting} />
                <span>TypeScript / React</span>
              </label>
              <label style={{ display: "flex", alignItems: "center", gap: 6 }}>
                <input type="radio" name="template" value="python" checked={template === "python"} onChange={() => setTemplate("python")} data-tauri-drag-region="none" disabled={submitting} />
                <span>Python</span>
              </label>
              <label style={{ display: "flex", alignItems: "center", gap: 6 }}>
                <input type="radio" name="template" value="cpp" checked={template === "cpp"} onChange={() => setTemplate("cpp")} data-tauri-drag-region="none" disabled={submitting} />
                <span>C/C++</span>
              </label>
              <label style={{ display: "flex", alignItems: "center", gap: 6 }}>
                <input type="radio" name="template" value="rust" checked={template === "rust"} onChange={() => setTemplate("rust")} data-tauri-drag-region="none" disabled={submitting} />
                <span>Rust</span>
              </label>
              <label style={{ display: "flex", alignItems: "center", gap: 6 }}>
                <input type="radio" name="template" value="csharp" checked={template === "csharp"} onChange={() => setTemplate("csharp")} data-tauri-drag-region="none" disabled={submitting} />
                <span>C#</span>
              </label>
              <label style={{ display: "flex", alignItems: "center", gap: 6 }}>
                <input type="radio" name="template" value="java" checked={template === "java"} onChange={() => setTemplate("java")} data-tauri-drag-region="none" disabled={submitting} />
                <span>Java</span>
              </label>
            </div>
          </fieldset>

          <label style={{ display: "flex", flexDirection: "column", gap: 4 }}>
            <span>Project path (optional)</span>
            <input
              style={{ padding: "8px 10px", borderRadius: 6, border: "1px solid #374151", background: "#111827", color: "#e5e7eb" }}
              placeholder={name.trim() ? `Default: $HOME/${name.trim()}` : "Leave empty for default: $HOME/<name>"}
              value={homeMount}
              onChange={(e) => setHomeMount(e.target.value)}
              data-tauri-drag-region="none"
              disabled={submitting}
              onMouseDown={(e) => { e.stopPropagation(); }}
              onPointerDown={(e) => { e.stopPropagation(); }}
            />
            <small style={{ color: "#9ca3af" }}>Leave empty for default: $HOME/&lt;name&gt;. If set, it must be an absolute path.</small>
          </label>

          {err && <div className="status-box" style={{ color: "#fca5a5" }}>{err}</div>}

          <div style={{ display: "flex", gap: 8, marginTop: 8, justifyContent: "flex-end" }}>
            <button onClick={handleClose} style={{ background: "#374151" }} data-tauri-drag-region="none" disabled={submitting}>Cancel</button>
            <button onClick={submit} disabled={!canSubmit} data-tauri-drag-region="none">Create</button>
          </div>
        </div>
      </div>
      {showProgress && (
        <CreationProgressModal
          progress={progress}
          onDismiss={() => {
            if (!progress?.success) {
              dismissProgress();
            }
          }}
        />
      )}
    </div>
  );
}

function CreationProgressModal({ progress, onDismiss }: { progress: CreationProgressPayload | null; onDismiss: () => void }) {
  // Hooks at top
  const isDone = progress?.done;
  const isSuccess = progress?.success === true;
  const isError = progress?.level === "error" || (isDone && progress?.success === false);
  const title = isSuccess ? "Environment ready" : isError ? "Creation failed" : "Creating environment…";

  const [show, setShow] = useState(false);
  const [closing, setClosing] = useState(false);

  useEffect(() => { requestAnimationFrame(() => setShow(true)); }, [progress]);

  const handleDismiss = useCallback(() => {
    if (closing) return;
    setClosing(true);
    setTimeout(() => onDismiss(), 220);
  }, [closing, onDismiss]);

  const modal = (
    <div className={`modal-backdrop ${show ? "show" : ""} ${closing ? "closing" : ""}`} data-tauri-drag-region="none">
      <div className="modal-card" style={{ width: "min(90vw, 440px)", padding: 24 }} data-tauri-drag-region="none">
        <header style={{ margin: 0 }}>{title}</header>
        <div className="body" style={{ marginTop: 8, color: "#cbd5f5", whiteSpace: "pre-wrap" }}>{progress?.message ?? "Working…"}</div>
        {!isDone && <div style={{ color: "#9ca3af", fontSize: 14, marginTop: 8 }}>This may take a minute. Please wait.</div>}
        {isDone && !isSuccess && (
          <button onClick={handleDismiss} style={{ alignSelf: "flex-end", background: "#ef4444", color: "white", padding: "6px 12px", borderRadius: 6, marginTop: 12 }}>Dismiss</button>
        )}
      </div>
    </div>
  );

  if (typeof document !== "undefined") {
    const host = document.querySelector('.layout') || document.body;
    return createPortal(modal, host);
  }

  return modal;
}

