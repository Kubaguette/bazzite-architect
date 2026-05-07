/**
 * src/components/PrimaryMenu.tsx
 *
 * Primary application menu (hamburger). Keeps responsibilities small: it
 * dispatches high-level user actions (open docs, report bug, show About).
 * Where possible it uses the Tauri opener plugin (openUrl) to open links in
 * the user's default browser. Fallbacks to window.open / location.href are
 * provided for environments where the plugin might not be available.
 *
 * Props:
 * - onAbout?: optional callback invoked when the About item is selected.
 */

import { useEffect, useRef, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";

const DOC_URL = "https://github.com/Kubaguette/envstation/blob/main/ARCHITECTURE.md";
const BUG_URL = "https://github.com/Kubaguette/envstation/issues";

export default function PrimaryMenu({ onAbout }: { onAbout?: () => void }) {
  // 'open' represents logical open state; 'mounted' controls DOM presence to allow exit animation
  const [open, setOpen] = useState(false);
  const [mounted, setMounted] = useState(false);
  const [closing, setClosing] = useState(false);
  const wrapRef = useRef<HTMLDivElement | null>(null);

  // Close on outside click or Escape
  useEffect(() => {
    if (!open) return;
    const onDocClick = (e: MouseEvent) => {
      const target = e.target as Node | null;
      if (!wrapRef.current) return;
      if (!wrapRef.current.contains(target)) {
        // trigger close flow
        handleClose();
      }
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") handleClose();
    };
    document.addEventListener("mousedown", onDocClick, true);
    document.addEventListener("keydown", onKey, true);
    return () => {
      document.removeEventListener("mousedown", onDocClick, true);
      document.removeEventListener("keydown", onKey, true);
    };
  }, [open]);

  const MENU_EXIT_MS = 260; // must match CSS pop-out duration

  const onToggle = () => {
    if (!open) {
      // opening: ensure mounted then open
      setMounted(true);
      setClosing(false);
      setOpen(true);
    } else {
      // closing sequence
      handleClose();
    }
  };

  const handleClose = () => {
    setOpen(false);
    setClosing(true);
    // leave mounted for exit animation then unmount
    setTimeout(() => {
      setClosing(false);
      setMounted(false);
    }, MENU_EXIT_MS);
  };

  const onSelect = (what: "docs" | "bug" | "about") => {
    // close with animation
    handleClose();
    if (what === "about") {
      // Trigger global About modal or callback
      if (typeof onAbout === "function") onAbout();
      window.dispatchEvent(new CustomEvent("open-about"));
      return;
    }
    if (what === "docs") {
      window.dispatchEvent(new CustomEvent("open-docs"));
      // Open the ARCHITECTURE.md in the user's default browser using Tauri opener plugin
      try {
        openUrl(DOC_URL);
      } catch {
        try { window.open(DOC_URL, "_blank"); } catch { location.href = DOC_URL; }
      }
      return;
    }
    if (what === "bug") {
      window.dispatchEvent(new CustomEvent("open-bug-report"));
      try {
        openUrl(BUG_URL);
      } catch {
        try { window.open(BUG_URL, "_blank"); } catch { location.href = BUG_URL; }
      }
      return;
    }
  };

  const triggerClass = mounted && !closing ? 'primary-menu-trigger open' : 'primary-menu-trigger';

  return (
    <div className="primary-menu" ref={wrapRef} data-tauri-drag-region="none">
      <button
        className={triggerClass}
        aria-label="Primary menu"
        aria-haspopup="menu"
        aria-expanded={open}
        data-tauri-drag-region="none"
        onClick={onToggle}
        title="Menu"
      >
        {/* clean hamburger glyph per spec */}
        <span aria-hidden>☰</span>
      </button>
      {mounted && (
        <div className={`menu-popover ${closing ? 'closing' : 'open'}`} role="menu" aria-label="Primary menu" data-tauri-drag-region="none">
          <button className="menu-item" role="menuitem" onClick={() => onSelect("docs")} data-tauri-drag-region="none">
            <span className="icon" aria-hidden>📖</span>
            <span className="text">Documentation</span>
          </button>
          <button className="menu-item" role="menuitem" onClick={() => onSelect("bug")} data-tauri-drag-region="none">
            <span className="icon" aria-hidden>🐞</span>
            <span className="text">Report a bug</span>
          </button>
          <div className="menu-divider" role="separator" />
          <button className="menu-item" role="menuitem" onClick={() => onSelect("about")} data-tauri-drag-region="none">
            <span className="icon" aria-hidden>ℹ️</span>
            <span className="text">About EnvStation</span>
          </button>
        </div>
      )}
    </div>
  );
}
