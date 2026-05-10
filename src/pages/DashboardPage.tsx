/**
 * src/pages/DashboardPage.tsx
 *
 * Dashboard landing page. Provides quick actions (create environment) and
 * showcases featured stacks via the FeaturedCarousel. Uses CreateEnvModal to
 * start the creation flow and delegates list refresh to EnvironmentsContext.
 */

import { useState, useRef, useEffect } from "react";
import { useBusy } from "../context/BusyContext";
import { useEnvironments } from "../context/EnvironmentsContext";
import CreateEnvModal, { TemplateId, CreationProgressModal } from "../components/CreateEnvModal";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import FeaturedCarousel from "../components/FeaturedCarousel";

type CreateDefaults = { template: TemplateId; name: string; home: string } | null;

export default function DashboardPage() {
  const [showCreate, setShowCreate] = useState<boolean>(false);
  const [defaults, setDefaults] = useState<CreateDefaults>(null);
  const { startBusy, endBusy } = useBusy();
  const { refresh } = useEnvironments();
  const [showImport, setShowImport] = useState<boolean>(false);
  const [gistUrl, setGistUrl] = useState<string>('');
  const [targetDir, setTargetDir] = useState<string>(`$HOME/EnvStation/Projects/New_Project`);
  const [importing, setImporting] = useState<boolean>(false);
  const [importProgress, setImportProgress] = useState<any | null>(null);
  const [showImportProgress, setShowImportProgress] = useState<boolean>(false);
  const unlistenRef = useRef<UnlistenFn | null>(null);

  const fetchList = async () => {
    startBusy();
    try {
      await refresh();
    } finally {
      endBusy();
    }
  };

  // Ensure we cleanup any event listener when modal closes/unmounts
  useEffect(() => {
    return () => {
      try { unlistenRef.current?.(); } catch {}
      unlistenRef.current = null;
    };
  }, [showImport]);

  const openCreateWith = (t: TemplateId) => {
    const nameMap: Record<TemplateId, string> = {
      "react-ts": "ReactTS_Project",
      python: "Python_Project",
      cpp: "CPP_Project",
      rust: "Rust_Project",
      java: "Java_Project",
      csharp: "CSharp_Project",
    };
    const suggestedName = nameMap[t];
    const home = `$HOME/EnvStation/Projects/${suggestedName}`; // will be expanded by the backend
    setDefaults({ template: t, name: suggestedName, home });
    setShowCreate(true);
  };

  return (
    <section className="dashboard-split">
      <div className="actions actions-top" data-tauri-drag-region="none" style={{ gap: 4 }}>
        <button className="action-banner-btn" onClick={() => { setDefaults(null); setShowCreate(true); }} data-tauri-drag-region="none">
          New Environment
        </button>
        <button className="action-banner-btn secondary" onClick={() => setShowImport(true)}>
          Recreate Environment
        </button>
      </div>

      <div className="carousel-wrap">
        <FeaturedCarousel onSelect={(key) => openCreateWith(key as TemplateId)} />
      </div>

      {showCreate && (
        <CreateEnvModal
          defaultTemplate={defaults?.template}
          defaultName={defaults?.name}
          defaultHomeMount={defaults?.home}
          onClose={() => setShowCreate(false)}
          onCreated={() => {
            setShowCreate(false);
            fetchList();
          }}
        />
      )}

      {showImport && (
        <div className={`modal-backdrop show`}>
          <div className="modal-card" style={{ maxWidth: 620 }}>
            <header>Import environment from GitHub Gist</header>
            <div className="body">
              <div style={{ display: 'grid', gap: 8 }}>
                <label style={{ color: '#e5e7eb' }}>GitHub Gist URL</label>
                <input value={gistUrl} onChange={(e) => setGistUrl(e.target.value)} placeholder="https://gist.github.com/.../abcdef" style={{ padding: '8px 10px', borderRadius: 8, border: '1px solid rgba(255,255,255,0.04)', background: '#0b1220', color: '#e5e7eb' }} />
                <label style={{ color: '#e5e7eb' }}>Target Directory</label>
                <input value={targetDir} onChange={(e) => setTargetDir(e.target.value)} style={{ padding: '8px 10px', borderRadius: 8, border: '1px solid rgba(255,255,255,0.04)', background: '#0b1220', color: '#e5e7eb' }} />

                {showImportProgress && (
                  <CreationProgressModal progress={importProgress} onDismiss={() => { /* no-op: user cannot dismiss while building */ }} />
                )}
              </div>
            </div>
            <div className="footer">
              <button onClick={() => {
                // if build in progress, disallow closing
                if (showImportProgress) return;
                // cleanup listener if any
                try { unlistenRef.current?.(); } catch {}
                unlistenRef.current = null;
                setImporting(false);
                setShowImportProgress(false);
                setImportProgress(null);
                setShowImport(false);
              }} style={{ background: '#374151', color: '#fff', border: 'none', padding: '8px 12px', borderRadius: 8, cursor: 'pointer' }}>Cancel</button>
              <button onClick={async () => {
                // start listening for creation progress events
                try {
                  const unlisten = await listen<any>('creation-progress', (evt) => {
                    const payload = evt.payload as any;
                    setImportProgress(payload);
                    setShowImportProgress(true);
                    if (payload.done) {
                      // cleanup listener
                      unlistenRef.current?.();
                      unlistenRef.current = null;
                      setShowImportProgress(false);
                      setImportProgress(null);
                      setImporting(false);
                      if (payload.success) {
                        toast.success('Environment successfully imported and built!');
                        setShowImport(false);
                        fetchList();
                      } else {
                        toast.error(payload.message || 'Environment creation failed');
                        invoke('client_log', { source: 'ui', level: 'ERROR', message: `import_environment build failed: ${payload.message}` }).catch(() => {});
                      }
                    }
                  });
                  unlistenRef.current = unlisten;

                  setImporting(true);
                  setShowImportProgress(true);
                  setImportProgress({ message: 'Starting import…' });

                  await invoke('import_environment', { gistUrl, targetDir });

                } catch (e) {
                  const err = String(e);
                  setImporting(false);
                  setShowImportProgress(false);
                  setImportProgress(null);
                  toast.error(err);
                  try { await invoke('client_log', { source: 'ui', level: 'ERROR', message: `import_environment failed: ${err}` }); } catch {}
                }
              }} disabled={importing} style={{ background: '#2563eb', color: '#fff', border: 'none', padding: '8px 12px', borderRadius: 8, cursor: 'pointer' }}>{importing ? 'Importing…' : 'Import & Build'}</button>
            </div>
          </div>
        </div>
      )}
    </section>
  );
}
