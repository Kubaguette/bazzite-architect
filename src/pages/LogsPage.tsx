/**
 * src/pages/LogsPage.tsx
 *
 * Power-user logs view. Loads historical logs via the "get_logs_text" command
 * and subscribes to the "app-log" event for live updates. The user may copy
 * or clear logs; clearing is performed by invoking the "clear_logs" command.
 */

import { useEffect, useState, useMemo } from "react";
import PageHeader from "../components/PageHeader";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";

export default function LogsPage() {
  const [raw, setRaw] = useState<string>("");

  const blocks = useMemo(() => {
    if (!raw) return [] as string[];
    // Split into blocks separated by one or more blank lines to preserve grouped messages
    return raw.split(/\n\s*\n/).map(b => b.trim()).filter(Boolean);
  }, [raw]);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    const load = async () => {
      try {
        const text = await invoke<string>("get_logs_text");
        setRaw(text);
      } catch (e) {
        setRaw(String(e));
      }
      unlisten = await listen<string>("app-log", (evt) => {
        setRaw((prev) => prev ? prev + "\n" + evt.payload : evt.payload);
      });
    };
    void load();
    return () => { try { unlisten?.(); } catch { /* ignore */ } };
  }, []);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(raw);
      toast.success("Logs copied!");
    } catch (e) {
      console.error(e);
      toast.error("Copy failed");
    }
  };


  const clear = async () => {
    try {
      await invoke("clear_logs");
      setRaw("");
      toast.success("Logs cleared!");
    } catch (e) {
      console.error(e);
      toast.error("Clear failed");
    }
  };

  const levelFor = (text: string) => {
    const t = text.toLowerCase();
    if (t.includes('error') || t.includes('err')) return 'error';
    if (t.includes('warn')) return 'warn';
    if (t.includes('success') || t.includes('ok') || t.includes('started') || t.includes('launched')) return 'success';
    return 'info';
  };

  const levelStyle = (lvl: string) => {
    switch (lvl) {
      case 'error': return { background: 'rgba(254, 226, 226, 0.06)', borderLeft: '4px solid rgba(239,68,68,0.22)', color: '#fee2e2' };
      case 'warn': return { background: 'rgba(255,245,230,0.04)', borderLeft: '4px solid rgba(250,204,21,0.14)', color: '#fef3c7' };
      case 'success': return { background: 'rgba(220,252,231,0.03)', borderLeft: '4px solid rgba(34,197,94,0.12)', color: '#bbf7d0' };
      default: return { background: 'transparent', borderLeft: '4px solid rgba(255,255,255,0.02)', color: '#e5e7eb' };
    }
  };

  const base = 12;
  const golden = Math.round(base * 1.618);
  const containerMax = 'min(60vh, calc(100vh - 220px))';
  const sectionGap = Math.round(golden * 0.5); // bring title, actions and log container a bit closer

  return (
    <section style={{ display: 'flex', flexDirection: 'column', gap: sectionGap }}>
      <PageHeader title="Logs (Power-User)" />

      <div className="log-actions" style={{ display: 'flex', gap: 8, marginTop: 0 }}>
        <button onClick={copy} style={{ background: '#2563eb', color: '#fff', border: 'none', padding: '8px 12px', borderRadius: 8 }}>Copy</button>
        <button onClick={clear} style={{ background: 'transparent', color: '#e5e7eb', border: '1px solid #374151', padding: '8px 12px', borderRadius: 8 }}>Clear</button>
      </div>

      <div style={{
        maxHeight: containerMax,
        overflowY: 'auto',
        padding: 12,
        borderRadius: 10,
        border: '1px solid rgba(255,255,255,0.04)',
        background: '#0f1724',
        boxShadow: '0 6px 18px rgba(2,6,23,0.6)',
        display: 'flex',
        flexDirection: 'column',
        gap: Math.round(golden * 0.6),
      }}>
        {blocks.length === 0 ? (
          <div style={{ color: '#888' }}>No logs yet.</div>
        ) : (
          blocks.map((b, i) => {
            const lvl = levelFor(b);
            const style = levelStyle(lvl);
            const lines = b.split('\n');
            const header = lines[0] ?? '';
            const rest = lines.slice(1).join('\n');
            return (
              <div key={i} style={{ padding: 10, borderRadius: 8, fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Monaco, "Roboto Mono", "Courier New", monospace', fontSize: 13, whiteSpace: 'pre-wrap', ...style }}>
                <div style={{ fontWeight: 700, marginBottom: rest ? 6 : 0 }}>{header}</div>
                {rest ? <div style={{ opacity: 0.95, whiteSpace: 'pre-wrap', fontSize: 13 }}>{rest}</div> : null}
              </div>
            );
          })
        )}
      </div>

    </section>
  );
}
