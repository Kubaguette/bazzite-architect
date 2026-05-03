/**
 * src/pages/LogsPage.tsx
 *
 * Power-user logs view. Loads historical logs via the "get_logs_text" command
 * and subscribes to the "app-log" event for live updates. The user may copy
 * or clear logs; clearing is performed by invoking the "clear_logs" command.
 */

import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";

export default function LogsPage() {
  const [raw, setRaw] = useState<string>("");

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

  return (
    <section>
      <h1>Logs (Power-User)</h1>
      <div className="log-actions">
        <button onClick={copy}>Copy</button>
        <button onClick={clear}>Clear</button>
      </div>
      <pre className="log-pre">{raw || "No logs yet."}</pre>
    </section>
  );
}
