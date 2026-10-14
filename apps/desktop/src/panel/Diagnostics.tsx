import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

/** Docking numbers for checking DPI scaling: window rect vs. monitor work area, in physical px. */
export function Diagnostics() {
  const [info, setInfo] = useState<unknown>(null);
  useEffect(() => {
    invoke("panel_diagnostics").then(setInfo, (e) => setInfo(String(e)));
  }, []);

  return (
    <pre className="mx-6 mb-2 max-h-56 overflow-auto rounded-lg bg-hover p-3 text-xs text-muted">
      {JSON.stringify(info, null, 2)}
    </pre>
  );
}
