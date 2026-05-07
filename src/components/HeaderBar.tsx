/**
 * src/components/HeaderBar.tsx
 *
 * Header bar component used across the app. It follows GNOME/Libadwaita
 * visual placement by showing the primary application menu next to the native
 * window controls. The header also contains the view switcher navigation.
 *
 * Behavior notes:
 * - Drag handling: clicking and dragging on the header starts a native window
 *   drag via the Tauri command "drag_window". This avoids WebView drag lag on
 *   Wayland. The invoke call is fire-and-forget and expects no payload or
 *   response.
 * - The component toggles visibility of advanced views based on a value
 *   persisted in localStorage under "advancedMode".
 */

import { useEffect, useState } from "react";
import type { MouseEventHandler } from "react";
import { NavLink } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import WindowControls from "./WindowControls";
import PrimaryMenu from "./PrimaryMenu";
import EnvLogo from "../../git_src/assets/EnvStation.svg";

export default function HeaderBar() {
  const [advanced, setAdvanced] = useState<boolean>(() => typeof localStorage !== "undefined" && localStorage.getItem("advancedMode") === "1");

  useEffect(() => {
    const handler = () => setAdvanced(localStorage.getItem("advancedMode") === "1");
    window.addEventListener("advanced-mode-changed", handler as EventListener);
    return () => window.removeEventListener("advanced-mode-changed", handler as EventListener);
  }, []);

  const onMouseDown: MouseEventHandler<HTMLDivElement> = (e) => {
    if (e.button !== 0) return;
    const target = e.target as HTMLElement;
    // Do NOT start dragging when clicking interactive elements inside the header
    // Check for common interactive selectors rather than relying only on the data attribute,
    // because overlapping elements with the attribute may prevent dragging unintentionally.
    if (target.closest('button, a, input, textarea, select, [data-tauri-drag-region="none"], .view-btn, .win-btn, .primary-menu-trigger')) {
      return;
    }
    // Native drag via Rust command to avoid WebView lag on Wayland
    invoke("drag_window").catch(() => {});
  };

  return (
    <div className="headerbar" data-tauri-drag-region onMouseDown={onMouseDown}>
      <div className="header-title">
        {/* Left: logo/title */}
        <img src={EnvLogo} alt="EnvStation logo" className="app-logo" draggable={false} />
        EnvStation
      </div>
      <nav className="header-nav view-switcher" aria-label="Views">
        <NavLink to="/dashboard" className={({ isActive }) => `view-btn ${isActive ? "active" : ""}`} data-tauri-drag-region="none">
          <span role="img" aria-label="home">🏠</span> <span className="label">Dashboard</span>
        </NavLink>
        <NavLink to="/environments" className={({ isActive }) => `view-btn ${isActive ? "active" : ""}`} data-tauri-drag-region="none">
          <span role="img" aria-label="env">📦</span> <span className="label">Environments</span>
        </NavLink>
        <NavLink to="/storage" className={({ isActive }) => `view-btn ${isActive ? "active" : ""}`} data-tauri-drag-region="none">
          <span role="img" aria-label="storage">💾</span> <span className="label">Storage</span>
        </NavLink>
        <NavLink to="/settings" className={({ isActive }) => `view-btn ${isActive ? "active" : ""}`} data-tauri-drag-region="none">
          <span role="img" aria-label="settings">🛠️</span> <span className="label">Settings</span>
        </NavLink>
        {advanced && (
          <NavLink to="/logs" className={({ isActive }) => `view-btn ${isActive ? "active" : ""}`} data-tauri-drag-region="none">
            <span role="img" aria-label="logs">📋</span> <span className="label">Logs</span>
          </NavLink>
        )}
      </nav>
      <div className="header-controls" data-tauri-drag-region="none">
        {/* Primary menu placed just before native window controls (GNOME style) */}
        <PrimaryMenu />
        <WindowControls />
      </div>
    </div>
  );
}
