import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Snapshot } from "../sources";

/** Docking numbers for checking DPI scaling: window rect vs. monitor work area, in physical px. */
export function Diagnostics({ sources }: { sources: Snapshot }) {
  const [info, setInfo] = useState<unknown>(null);
  const [probe, setProbe] = useState<unknown>(null);
  const [media, setMedia] = useState<unknown>(null);
  useEffect(() => {
    invoke("panel_diagnostics").then(setInfo, (e) => setInfo(String(e)));
  }, []);

  return (
    <div className="mx-6 mb-2 space-y-2">
      <pre className="max-h-56 overflow-auto rounded-lg bg-hover p-3 text-xs text-muted">{JSON.stringify(info, null, 2)}</pre>
      <div>
        <p className="mb-1 text-xs text-muted">
          Source probe: every window of Chrome, Edge, Zoom and Teams with its class and title. Run it during a call to
          check the Zoom/Teams rules in title_rules.toml.
        </p>
        <button className="btn" onClick={() => invoke("probe_windows").then(setProbe, (e) => setProbe(String(e)))}>
          Probe windows
        </button>
        <pre className="mt-2 max-h-56 overflow-auto rounded-lg bg-hover p-3 text-xs text-muted">
          {JSON.stringify({ detected: sources, windows: probe }, null, 2)}
        </pre>
      </div>
      <div>
        <p className="mb-1 text-xs text-muted">
          Media probe: the Windows media sessions (title, app, playing) and why each source did or didn't get one.
          Run it while the video plays if play / pause or "pause while I type" does nothing.
        </p>
        <button className="btn" onClick={() => invoke("probe_media").then(setMedia, (e) => setMedia(String(e)))}>
          Probe media
        </button>
        <pre className="mt-2 max-h-56 overflow-auto rounded-lg bg-hover p-3 text-xs text-muted">{JSON.stringify(media, null, 2)}</pre>
      </div>
    </div>
  );
}
