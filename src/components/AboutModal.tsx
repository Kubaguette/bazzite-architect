/**
 * src/components/AboutModal.tsx
 *
 * Simple About dialog. Offers a link to the project repository which is
 * opened via the Tauri opener plugin (openUrl). Falls back to window.open if
 * the plugin call fails (useful for development in non-Tauri environments).
 *
 * Props:
 * - onClose(): callback invoked when the modal requests to be closed.
 */

import { useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import EnvLogo from "../../git_src/assets/EnvStation.svg";

const REPO_URL = "https://github.com/Kubaguette/envstation";

export default function AboutModal({ onClose }: { onClose: () => void }) {
  const [show, setShow] = useState(false);
  const [closing, setClosing] = useState(false);
  const [spinning, setSpinning] = useState(false);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") startClose(); };
    document.addEventListener("keydown", onKey, true);
    // trigger enter animation
    requestAnimationFrame(() => setShow(true));
    return () => document.removeEventListener("keydown", onKey, true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const openRepo = async () => {
    try {
      await openUrl(REPO_URL);
    } catch {
      try { window.open(REPO_URL, "_blank"); } catch { location.href = REPO_URL; }
    }
  };

  const onLogoClick = () => {
    // brief spin easter egg: ignore while spinning
    if (spinning) return;
    setSpinning(true);
    // clear spinning state after animation completes (match CSS duration)
    setTimeout(() => setSpinning(false), 720);
  };

  const startClose = () => {
    if (closing) return;
    setClosing(true);
    // wait for CSS transition to finish before invoking parent's onClose
    setTimeout(() => onClose(), 220);
  };

  return (
    <div className={`modal-backdrop ${show ? "show" : ""} ${closing ? "closing" : ""}`} onClick={startClose} data-tauri-drag-region="none">
      <div className="modal-card" role="dialog" aria-modal="true" aria-labelledby="about-title" onClick={(e) => e.stopPropagation()}>
        <header id="about-title">About EnvStation</header>
        <div className="body">
          <img src={EnvLogo} alt="EnvStation logo" className={`about-logo ${spinning ? "spin" : ""}`} draggable={false} onClick={onLogoClick} data-tauri-drag-region="none" title="Click me!" />
          <p>EnvStation – Portable Dev Environments.</p>
          <p>Version: 1.0.0</p>
          <p>License & contributors are listed in the project repository.</p>
        </div>
        <div className="footer">
          <button onClick={openRepo} data-tauri-drag-region="none">Open repository</button>
          <button onClick={startClose} data-tauri-drag-region="none">Close</button>
        </div>
      </div>
    </div>
  );
}