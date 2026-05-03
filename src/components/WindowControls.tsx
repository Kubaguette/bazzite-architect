/**
 * src/components/WindowControls.tsx
 *
 * Small set of native window control buttons (minimize, maximize/restore,
 * close). Uses Tauri's window API (getCurrentWindow) to operate on the host
 * window. These operations are performed via the Tauri window object and do
 * not require custom invoke commands.
 */

import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

export default function WindowControls() {
  const [max, setMax] = useState(false);

  useEffect(() => {
    const win = getCurrentWindow();
    win.isMaximized().then(setMax).catch(() => {});
  }, []);

  const minimize = async () => {
    const win = getCurrentWindow();
    await win.minimize();
  };
  const toggleMax = async () => {
    const win = getCurrentWindow();
    await win.toggleMaximize();
    const m = await win.isMaximized();
    setMax(m);
  };
  const close = async () => {
    const win = getCurrentWindow();
    await win.close();
  };

  return (
    <div className="window-controls" data-tauri-drag-region="none">
      <button className="win-btn" title="Minimize" onClick={minimize} data-tauri-drag-region="none" aria-label="Minimize">
        <svg viewBox="0 0 16 16" aria-hidden="true"><rect x="3" y="8" width="10" height="1.6" rx="0.8" fill="#e6e6e6"/></svg>
      </button>
      <button className="win-btn" title={max ? "Restore" : "Maximize"} onClick={toggleMax} data-tauri-drag-region="none" aria-label={max ? "Restore" : "Maximize"}>
        {max ? (
          <svg viewBox="0 0 16 16" aria-hidden="true">
            <path d="M5 6.5a1.5 1.5 0 0 1 1.5-1.5H12V3.5A1.5 1.5 0 0 0 10.5 2H4A2 2 0 0 0 2 4v6.5A1.5 1.5 0 0 0 3.5 12H5V6.5Z" fill="none" stroke="#e6e6e6" strokeWidth="1.2"/>
            <rect x="6.5" y="4.5" width="7.5" height="7.5" rx="1.2" fill="none" stroke="#e6e6e6" strokeWidth="1.2"/>
          </svg>
        ) : (
          <svg viewBox="0 0 16 16" aria-hidden="true"><rect x="3" y="3" width="10" height="10" rx="1.6" fill="none" stroke="#e6e6e6" strokeWidth="1.2"/></svg>
        )}
      </button>
      <button className="win-btn win-close" title="Close" onClick={close} data-tauri-drag-region="none" aria-label="Close">
        <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 4L12 12M12 4L4 12" stroke="#e6e6e6" strokeWidth="1.6" strokeLinecap="round"/></svg>
      </button>
    </div>
  );
}
