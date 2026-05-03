/**
 * src/App.tsx
 *
 * Root React application. Wires together top-level providers and global
 * event listeners for Tauri-driven notifications.
 *
 * Providers used:
 * - BusyProvider: global busy indicator (reference-counted)
 * - EnvironmentsProvider: fetches and caches the list of environments
 * - SpaceCacheProvider: frontend cache/request-coalescing for project sizes
 *
 * Global event listeners:
 * - "app-notification": lightweight channel for the backend to surface
 *    transient notifications of shape { message, type: 'success'|'info'|'error' }
 * - "size-update": also listened to here to show a brief toast when a
 *    project size is updated; the authoritative handling of size updates is
 *    implemented in SpaceCacheContext which updates the cached values.
 */

import { HashRouter, Routes, Route, Navigate } from "react-router-dom";
import { useEffect, useState } from "react";
import "./App.css";
import { BusyProvider, useBusy } from "./context/BusyContext";
import HeaderBar from "./components/HeaderBar";
import DashboardPage from "./pages/DashboardPage";
import EnvironmentsPage from "./pages/EnvironmentsPage";
import StoragePage from "./pages/StoragePage";
import SettingsPage from "./pages/SettingsPage";
import LogsPage from "./pages/LogsPage";
import { EnvironmentsProvider } from "./context/EnvironmentsContext";
import { SpaceCacheProvider } from "./context/SpaceCacheContext";
import { Toaster, toast } from "sonner";
import { listen } from "@tauri-apps/api/event";
import AboutModal from "./components/AboutModal";

function Layout() {
  const { isBusy } = useBusy();
  const [showAbout, setShowAbout] = useState(false);
  useEffect(() => {
    const handler = () => setShowAbout(true);
    window.addEventListener("open-about", handler as any);
    return () => window.removeEventListener("open-about", handler as any);
  }, []);
  return (
    <div className={`layout ${isBusy ? "busy" : ""}`}>
      <HeaderBar />
      <div className="content">
        <div className="clamp">
          <Routes>
            <Route path="/" element={<Navigate to="/dashboard" replace />} />
            <Route path="/dashboard" element={<DashboardPage />} />
            <Route path="/environments" element={<EnvironmentsPage />} />
            <Route path="/storage" element={<StoragePage />} />
            <Route path="/settings" element={<SettingsPage />} />
            <Route path="/logs" element={<LogsPage />} />
          </Routes>
        </div>
      </div>
      {showAbout && <AboutModal onClose={() => setShowAbout(false)} />}
    </div>
  );
}

export default function App() {
  useEffect(() => {
    // Prevent duplicate listener registration (React StrictMode / hot reload)
    const g = globalThis as any;
    if (g.__appNotificationListenerSet) return;
    g.__appNotificationListenerSet = true;

    (async () => {
      await listen<{ message: string; type: "success" | "info" | "error" }>(
        "app-notification",
        (evt) => {
          const p = evt.payload;
          if (!p) return;
          if (p.type === "success") toast.success(p.message);
          else if (p.type === "error") toast.error(p.message);
          else toast.info(p.message);
        }
      );

      // Also listen for size updates and show a brief toast
      await listen<{ path: string; size: number }>("size-update", (evt) => {
        const fmt = (n: number) => {
          const units = ["B", "KB", "MB", "GB", "TB"]; let i = 0; let v = n;
          while (v >= 1024 && i < units.length - 1) { v /= 1024; i++; }
          return `${v.toFixed(1)} ${units[i]}`;
        };
        const s = evt.payload?.size;
        if (typeof s === "number") toast.info(`Project size updated: ${fmt(s)}`);
      });
    })();
  }, []);

  return (
    <BusyProvider>
      <EnvironmentsProvider>
        <SpaceCacheProvider>
          <HashRouter>
            <Layout />
            <Toaster position="bottom-right" theme="dark" richColors closeButton />
          </HashRouter>
        </SpaceCacheProvider>
      </EnvironmentsProvider>
    </BusyProvider>
  );
}
