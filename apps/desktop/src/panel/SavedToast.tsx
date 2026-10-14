import { useEffect } from "react";
import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";

export function SavedToast({ path, onDismiss }: { path: string; onDismiss: () => void }) {
  useEffect(() => {
    const timer = setTimeout(onDismiss, 10_000);
    return () => clearTimeout(timer);
  }, [path]); // restart the timer only for a new save

  return (
    <div className="absolute inset-x-4 bottom-20 rounded-xl bg-ink p-4 text-surface shadow-lg">
      <p className="truncate text-sm">
        Saved <b>{path.split(/[\\/]/).pop()}</b>
      </p>
      <div className="mt-2 flex gap-4 text-sm underline-offset-2 [&>button:hover]:underline">
        <button onClick={() => void openPath(path)}>Open</button>
        <button onClick={() => void revealItemInDir(path)}>Show in folder</button>
        <button onClick={() => void navigator.clipboard.writeText(path)}>Copy path</button>
      </div>
    </div>
  );
}
