/**
 * src/utils/dirSizeQueue.ts
 *
 * Small concurrency limiter and Tauri IPC helpers for requesting directory
 * size calculations from the Rust backend.
 *
 * Rationale and developer notes:
 * - Calculating directory sizes can be an expensive, long-running operation on
 *   the backend. We limit parallel requests from the UI (default max = 2) to
 *   avoid overwhelming the backend with concurrent walkers.
 * - Requests are performed via Tauri's invoke API. The backend command names
 *   used are:
 *     - "get_dir_size": invoked with payload { path: string }
 *         * The backend does NOT return the computed size directly. Instead it
 *           will emit a "size-update" event (handled by SpaceCacheContext) with
 *           payload { path: string, size: number } when the calculation completes.
 *         * This fire-and-forget pattern keeps the UI responsive and relies on
 *           the event bus for results and atomic updates.
 *     - "cancel_dir_size_jobs": invoked with no payload to request that the
 *         backend abort its active walkers.
 * - The frontend queue supports cancelAll() which both clears the local queue
 *   and issues the cancel request to the backend.
 */

import { invoke } from "@tauri-apps/api/core";

class ConcurrencyQueue {
  private max: number;
  private active = 0;
  private q: Array<{ gen: number; fn: () => Promise<any>; resolve: (v: any) => void; reject: (e: any) => void }>; 
  private generation = 0;

  constructor(max: number) {
    this.max = max;
    this.q = [];
  }

  enqueue<T>(fn: () => Promise<T>): Promise<T> {
    const gen = this.generation;
    return new Promise<T>((resolve, reject) => {
      this.q.push({ gen, fn: fn as any, resolve, reject });
      this.run();
    });
  }

  cancelAll(reason: any = new Error("dirSizeQueue: cancelled")) {
    // Invalidate all pending jobs and clear queue
    this.generation++;
    const q = this.q.splice(0, this.q.length);
    for (const item of q) {
      try { item.reject(reason); } catch {}
    }
  }

  private run() {
    while (this.active < this.max && this.q.length > 0) {
      const item = this.q.shift()!;
      // Drop job if generation was invalidated before it started
      if (item.gen !== this.generation) {
        item.reject(new Error("dirSizeQueue: stale job dropped"));
        continue;
      }
      this.active++;
      item.fn()
        .then((v) => {
          // Ignore result if generation changed mid-flight
          if (item.gen === this.generation) item.resolve(v);
          else item.reject(new Error("dirSizeQueue: stale result"));
        })
        .catch((e) => item.reject(e))
        .finally(() => {
          this.active--;
          this.run();
        });
    }
  }
}

const dirSizeLimiter = new ConcurrencyQueue(2);

/**
 * Request that the backend begins computing the size for `path`.
 *
 * Payload sent to backend command "get_dir_size": { path: string }
 * Expected result: none (the backend will emit a "size-update" event with
 *    payload { path: string, size: number } when the calculation completes).
 */
export function requestDirSize(path: string): Promise<void> {
  return dirSizeLimiter.enqueue(() => invoke<void>("get_dir_size", { path }));
}

/**
 * Cancel all pending frontend requests and ask the backend to stop its workers.
 * This function is tolerant to failures of the backend cancel command.
 */
export async function cancelAllDirSizeJobs() {
  // Cancel frontend queue immediately
  dirSizeLimiter.cancelAll();
  // Ask backend to abort heavy walkers
  try {
    await invoke("cancel_dir_size_jobs");
  } catch {
    // ignore
  }
}
