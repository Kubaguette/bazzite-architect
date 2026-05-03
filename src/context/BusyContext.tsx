/**
 * src/context/BusyContext.tsx
 *
 * Lightweight global busy indicator implemented as a simple reference
 * counter. Components call startBusy() when beginning long-running UI tasks
 * (for example initiating backend operations) and call endBusy() when the
 * task finishes. The derived `isBusy` boolean is true whenever the reference
 * count is greater than zero.
 *
 * This pattern is helpful to coalesce multiple overlapping operations and to
 * avoid flicker from rapidly toggled spinners. The API is intentionally tiny:
 * - startBusy(): increment counter
 * - endBusy(): decrement counter (clamped at 0)
 * - isBusy: boolean computed from counter
 *
 * Use the useBusy() hook to access this context; it throws if used outside the
 * BusyProvider.
 */

import { createContext, useContext, useMemo, useState, useCallback, ReactNode } from "react";

type BusyCtx = {
  isBusy: boolean;
  startBusy: () => void;
  endBusy: () => void;
};

const Ctx = createContext<BusyCtx | null>(null);

export function BusyProvider({ children }: { children: ReactNode }) {
  const [count, setCount] = useState(0);

  // Stable callback identities prevent effect loops when used in dependency arrays
  const startBusy = useCallback(() => setCount((c) => c + 1), []);
  const endBusy = useCallback(() => setCount((c) => Math.max(0, c - 1)), []);

  const value = useMemo(
    () => ({
      isBusy: count > 0,
      startBusy,
      endBusy,
    }),
    [count, startBusy, endBusy]
  );

  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useBusy(): BusyCtx {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("useBusy must be used within BusyProvider");
  return ctx;
}
