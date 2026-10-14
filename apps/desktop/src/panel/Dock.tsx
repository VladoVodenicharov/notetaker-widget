import { useEffect, useState, type PointerEvent as ReactPointerEvent } from "react";
import { invoke } from "@tauri-apps/api/core";

export type Dock = { edge: "left" | "right"; widthDip: number };

/** The panel's side and width. Rust owns them (and saves them); this mirrors what it applied. */
export function useDock() {
  const [dock, setDock] = useState<Dock | null>(null);
  useEffect(() => {
    invoke<Dock | null>("get_dock").then(setDock);
  }, []);
  const applyDock = (next: Dock) => void invoke<Dock>("set_dock", next).then(setDock);
  return [dock, applyDock] as const;
}

// Same range as `clamp_width` in Rust; here only to keep the guide line honest.
const MIN_WIDTH = 320;
const MAX_WIDTH = 720;

/**
 * A 6 px grip on the panel's inner edge. Dragging shows a guide line and the new width;
 * the panel is resized once, on release, so maximized windows re-fit only once.
 */
export function ResizeHandle({ dock, onResize }: { dock: Dock; onResize: (widthDip: number) => void }) {
  const [width, setWidth] = useState<number | null>(null);
  const right = dock.edge === "right";

  function startDrag(e: ReactPointerEvent) {
    // Listen on the window, not the grip: when the same click activates the panel, WebView2
    // never gives the grip pointer capture, so the moves only reach the window.
    e.currentTarget.setPointerCapture(e.pointerId);
    const [x0, width0] = [e.clientX, dock.widthDip];
    let latest = width0;
    // CSS px are DIPs here.
    const move = (ev: PointerEvent) => {
      const delta = right ? x0 - ev.clientX : ev.clientX - x0;
      latest = Math.round(Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, width0 + delta)));
      setWidth(latest);
    };
    const end = (ev: PointerEvent) => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", end);
      window.removeEventListener("pointercancel", end);
      setWidth(null);
      if (ev.type === "pointerup" && latest !== width0) onResize(latest);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", end);
    window.addEventListener("pointercancel", end);
    setWidth(width0);
  }

  // The guide can't be drawn outside our window, so when widening it waits at the edge.
  const guide = width === null ? 0 : right ? window.innerWidth - width : width;

  return (
    <>
      <div
        className={`absolute inset-y-0 z-30 w-1.5 cursor-col-resize hover:bg-line ${right ? "left-0" : "right-0"}`}
        onPointerDown={startDrag}
      />
      {width !== null && (
        <div
          className="pointer-events-none absolute inset-y-0 z-30 w-0.5 bg-muted"
          style={{ left: Math.min(window.innerWidth - 2, Math.max(0, guide)) }}
        >
          <span className={`absolute top-2 rounded bg-ink px-1.5 py-0.5 text-xs text-surface ${right ? "left-1" : "right-1"}`}>
            {width}
          </span>
        </div>
      )}
    </>
  );
}
