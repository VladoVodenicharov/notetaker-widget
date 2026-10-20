import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useSession } from "./session";
import { emptySnapshot, winnerOf, type Snapshot } from "./sources";
import { step, type Switcher } from "./sourceSwitching";

/** The latest detected sources, pushed by the Rust side whenever they change. */
export function useSources(): Snapshot {
  const [snapshot, setSnapshot] = useState<Snapshot>(emptySnapshot);
  useEffect(() => {
    let cancelled = false;
    // Listen first so a change between the two calls isn't lost.
    const unlisten = listen<Snapshot>("sources-changed", (e) => setSnapshot(e.payload));
    invoke<Snapshot>("get_sources").then((s) => !cancelled && setSnapshot((current) => (current === emptySnapshot ? s : current)));
    return () => {
      cancelled = true;
      void unlisten.then((f) => f());
    };
  }, []);
  return snapshot;
}

/**
 * Before writing starts, the header follows the likely source at once.
 * Once writing has started, a new source takes over only after it stays the likely one
 * (and is playing or in front) for 8 seconds, unless the source is pinned.
 */
export function useFollowSource(snapshot: Snapshot) {
  const sessionId = useSession((s) => s.id);
  const started = useSession((s) => s.startedAt !== null);
  const pinned = useSession((s) => s.pinned);
  const switcher = useRef<Switcher>({ current: null, pending: null });
  const latest = useRef(snapshot);
  latest.current = snapshot;

  useEffect(() => {
    if (!started) useSession.getState().follow(winnerOf(snapshot));
  }, [snapshot, sessionId, started]);

  useEffect(() => {
    if (!started || pinned) return;
    switcher.current = { current: useSession.getState().source, pending: null };
    const check = () => {
      const before = switcher.current.current;
      switcher.current = step(switcher.current, winnerOf(latest.current), Date.now());
      const after = switcher.current.current;
      if (after && after.key !== before?.key) useSession.getState().switchTo(after);
    };
    check();
    // The 8 s wait needs a clock: snapshots only arrive when something changes.
    const timer = setInterval(check, 1000);
    return () => clearInterval(timer);
  }, [started, pinned, sessionId]);
}
