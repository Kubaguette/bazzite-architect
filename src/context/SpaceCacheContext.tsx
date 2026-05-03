/**
 * src/context/SpaceCacheContext.tsx
 *
 * React context that provides frontend-side caching and request coalescing for
 * directory/project size calculations. The heavy work of walking the filesystem
 * and computing sizes is performed in the Rust backend; results are delivered
 * to the frontend via a Tauri event ("size-update").
 *
 * Responsibilities and behavior:
 * - Maintains an in-memory cache mapping absolute paths -> last known size (bytes).
 * - Exposes requestSize(path) which:
 *   - Returns a cached value immediately if present.
 *   - Coalesces concurrent requests for the same path: multiple callers
 *     receive the same Promise and are resolved when the backend event arrives.
 *   - Sends a fire-and-forget IPC request (requestDirSize) to the backend when
 *     a fresh calculation is required. The backend MUST emit a "size-update"
 *     event with the payload { path, size } for the result to be applied.
 * - Provides setCachedSize to seed/update the cache from other parts of the
 *   application (for example when the backend includes precomputed sizes).
 * - Listens globally for the "size-update" event and performs the following:
 *   - Atomically updates the cache for the reported path.
 *   - Resolves all coalesced promises for an in-flight request and clears the
 *     in-flight lock for that path.
 * - cancelAll clears any frontend in-flight locks and asks the backend to abort
 *   its running dir-size workers.
 *
 * Note: This module intentionally does not expose the details of the Tauri
 * invoke used to start a size calculation — that call is implemented in
 * src/utils/dirSizeQueue.ts and documented there. The event-based flow keeps
 * the frontend reactive and avoids blocking on long-running backend operations.
 */

import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, ReactNode } from "react";
import { requestDirSize, cancelAllDirSizeJobs } from "../utils/dirSizeQueue";
import { listen } from "@tauri-apps/api/event";

/**
 * Public shape of the space cache context provided to consumers.
 */
interface SpaceCacheCtx {
  /** Return the cached size in bytes for `path`, or null if unknown. */
  getCachedSize: (path: string) => number | null;
  /** Whether a size calculation for `path` is currently in-flight (requested but not yet resolved). */
  isScanning: (path: string) => boolean;
  /** Request a size calculation for `path`. Resolves to the size in bytes when available. */
  requestSize: (path: string) => Promise<number>;
  /** Force-set the cached size for `path` (used to seed cache from backend-provided metadata). */
  setCachedSize: (path: string, size: number) => void;
  /** Cancel all outstanding frontend requests and ask the backend to abort workers. */
  cancelAll: () => Promise<void>;
}

const Ctx = createContext<SpaceCacheCtx | null>(null);

/**
 * Provider that wraps the app and supplies size-caching primitives to descendants.
 *
 * Implementation notes for maintainers:
 * - `cache` stores the last known sizes.
 * - `inflight` maps path -> { promise, resolvers } so multiple callers can be coalesced.
 * - A global Tauri event listener ("size-update") updates cache entries and
 *   resolves coalesced promises. The listener intentionally only reacts to
 *   atomic events and does not attempt to reconcile partial updates.
 */
export function SpaceCacheProvider({ children }: { children: ReactNode }) {
  const [cache, setCache] = useState<Record<string, number>>({});
  const inflight = useRef(new Map<string, { promise: Promise<number>; resolvers: Array<(n: number) => void> }>());

  const setCachedSize = useCallback((path: string, size: number) => {
    setCache(prev => (prev[path] === size ? prev : { ...prev, [path]: size }));
  }, []);

  const getCachedSize = useCallback((path: string) => {
    return Object.prototype.hasOwnProperty.call(cache, path) ? cache[path] : null;
  }, [cache]);

  const isScanning = useCallback((path: string) => inflight.current.has(path), []);

  useEffect(() => {
    // Global listener for atomic size updates from Rust
    let unlisten: (() => void) | null = null;
    (async () => {
      unlisten = await listen<{ path: string; size: number }>("size-update", (e) => {
        const { path, size } = e.payload;
        setCachedSize(path, size);
        const infl = inflight.current.get(path);
        if (infl) {
          for (const r of infl.resolvers) {
            try { r(size); } catch {}
          }
          inflight.current.delete(path);
        }
      });
    })();
    return () => { try { unlisten?.(); } catch {} };
  }, [setCachedSize]);

  const requestSize = useCallback(async (path: string) => {
    // Return cached immediately
    if (Object.prototype.hasOwnProperty.call(cache, path)) {
      return cache[path];
    }
    const existing = inflight.current.get(path);
    if (existing) {
      return existing.promise;
    }
    // Create a promise that will resolve when the event arrives
    let resolveFn: (n: number) => void;
    const promise = new Promise<number>((resolve) => { resolveFn = resolve; });
    inflight.current.set(path, { promise, resolvers: [resolveFn!] });
    // Fire-and-forget request to Rust; result will come via event
    requestDirSize(path).catch(() => {
      // On failure, clear lock to allow retry; do not resolve promise here
      inflight.current.delete(path);
    });
    return promise;
  }, [cache]);

  const cancelAll = useCallback(async () => {
    inflight.current.clear();
    await cancelAllDirSizeJobs();
  }, []);

  const value = useMemo(() => ({ getCachedSize, isScanning, requestSize, setCachedSize, cancelAll }), [getCachedSize, isScanning, requestSize, setCachedSize, cancelAll]);

  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

/**
 * Hook for consumers to access the SpaceCache context. Throws if used outside the provider.
 */
export function useSpaceCache(): SpaceCacheCtx {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("useSpaceCache must be used within SpaceCacheProvider");
  return ctx;
}
