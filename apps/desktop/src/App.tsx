import { useEffect, useState } from "react";
import { EditorContent, useEditor } from "@tiptap/react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Info, Minus, Monitor, PanelLeft, PanelRight } from "lucide-react";
import { extensions } from "./editor/extensions";
import { useSession } from "./session";
import { discardDraft, loadDraft, useAutosave, type Draft } from "./autosave";
import { Header } from "./panel/Header";
import { ContinueSuggestion, PreviouslyBox } from "./panel/Continue";
import { SourceSection } from "./editor/sourceSection";
import { FinishSheet } from "./panel/FinishSheet";
import { SavedToast } from "./panel/SavedToast";
import { Diagnostics } from "./panel/Diagnostics";
import { RecoveryDialog } from "./panel/RecoveryDialog";
import { ResizeHandle, screenLabel, useDock, useScreens, type Screen } from "./panel/Dock";
import { useFollowSource, useSources } from "./useFollowSource";
import { usePlayback } from "./usePlayback";
import { VideoControls } from "./panel/VideoControls";

export default function App() {
  const markStarted = useSession((s) => s.markStarted);
  const sessionId = useSession((s) => s.id);
  // A draft left by a crash: undefined while checking, null once there's none (or it was handled).
  const [recovery, setRecovery] = useState<Draft | null>();
  const [restored, setRestored] = useState<Draft | null>(null);
  // A new editor per session, so undo can't bring back an already saved note.
  const editor = useEditor(
    {
      // Section headings after a source switch; only the live editor needs them.
      extensions: [...extensions, SourceSection.configure({ take: () => useSession.getState().takeSection() })],
      autofocus: "end",
      content: restored?.id === sessionId ? restored.doc : undefined,
      onUpdate: ({ editor }) => {
        if (!editor.isEmpty) markStarted();
      },
    },
    [sessionId],
  );
  const [finishing, setFinishing] = useState(false);
  const [savedPath, setSavedPath] = useState<string | null>(null);
  const [showDiagnostics, setShowDiagnostics] = useState(false);
  const [hotkeyWarning, setHotkeyWarning] = useState<string | null>(null);
  const [dock, applyDock] = useDock();
  const [screens, moveToNextScreen] = useScreens();
  const sources = useSources();
  useFollowSource(sources);
  useAutosave(editor, recovery === null);
  const playback = usePlayback(editor, sources);

  useEffect(() => {
    loadDraft().then(setRecovery, () => setRecovery(null));
    invoke<string | null>("hotkey_warning").then(setHotkeyWarning);
  }, []);

  // Ctrl+Alt+N brought the panel forward: put the cursor in the editor.
  useEffect(() => {
    const unlisten = listen("focus-editor", () => editor?.commands.focus());
    return () => void unlisten.then((f) => f());
  }, [editor]);

  // The new session id gives a fresh editor, created with the draft's content.
  function resume(draft: Draft) {
    setRestored(draft);
    const segments = draft.segments ?? [];
    useSession.setState((s) => ({
      id: draft.id,
      title: draft.title,
      titleEdited: true,
      folder: draft.folder ?? s.folder,
      startedAt: draft.startedAt,
      source: draft.source ?? null,
      segments,
      // Notes typed after recovery continue the last section; no heading is pending.
      sections: { writingKey: segments[segments.length - 1]?.key ?? null, pending: null, labelled: segments.length > 1 },
      continuing: draft.continuing ?? null,
    }));
    setRecovery(null);
  }

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!e.ctrlKey || e.key !== "Enter") return;
      e.preventDefault(); // otherwise the editor inserts a hard break first
      e.stopPropagation();
      setFinishing(true);
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, []);

  return (
    <div className="relative flex h-screen flex-col bg-surface text-ink">
      {dock && <ResizeHandle dock={dock} onResize={(widthDip) => applyDock({ ...dock, widthDip })} />}
      <div className="flex justify-end gap-1 px-3 pt-2 text-muted">
        {dock && (
          <button
            className="icon-btn"
            title={`Dock on the ${dock.edge === "right" ? "left" : "right"}`}
            onClick={() => applyDock({ ...dock, edge: dock.edge === "right" ? "left" : "right" })}
          >
            {dock.edge === "right" ? <PanelLeft size={16} /> : <PanelRight size={16} />}
          </button>
        )}
        {screens.length > 1 && <ScreenButton screens={screens} onMove={moveToNextScreen} />}
        <button className="icon-btn" title="Docking diagnostics" onClick={() => setShowDiagnostics((v) => !v)}>
          <Info size={16} />
        </button>
        <button className="icon-btn" title="Hide panel (Ctrl+Alt+N or the tray icon brings it back)" onClick={() => void invoke("hide_panel")}>
          <Minus size={16} />
        </button>
      </div>
      {hotkeyWarning && (
        <p className="mx-6 mb-2 flex justify-between gap-2 rounded-lg bg-hover px-3 py-2 text-sm text-muted">
          {hotkeyWarning}
          <button className="hover:text-ink" title="Dismiss" onClick={() => setHotkeyWarning(null)}>
            ×
          </button>
        </p>
      )}
      {showDiagnostics && <Diagnostics sources={sources} />}

      <Header sources={sources} />
      <ContinueSuggestion />
      <PreviouslyBox />
      <EditorContent editor={editor} className="min-h-0 flex-1 overflow-y-auto px-6" />

      <footer className="flex items-center justify-between border-t border-line px-6 py-3">
        <VideoControls
          source={playback.source}
          onToggle={playback.toggle}
          pauseWhileTyping={playback.pauseWhileTyping}
          onTogglePauseWhileTyping={playback.togglePauseWhileTyping}
          lastFailure={playback.lastFailure}
        />
        <Elapsed />
        <button className="btn-primary" title="Finish (Ctrl+Enter)" onClick={() => setFinishing(true)}>
          Finish
        </button>
      </footer>

      {finishing && editor && (
        <FinishSheet
          editor={editor}
          onClose={() => {
            setFinishing(false);
            editor.commands.focus();
          }}
          onSaved={(path) => {
            discardDraft();
            setFinishing(false);
            setSavedPath(path);
            editor.commands.focus();
          }}
        />
      )}
      {savedPath && <SavedToast path={savedPath} onDismiss={() => setSavedPath(null)} />}
      {recovery && (
        <RecoveryDialog
          draft={recovery}
          onResume={() => resume(recovery)}
          onSaveNow={() => {
            resume(recovery);
            setFinishing(true);
          }}
          onDiscard={() => {
            discardDraft();
            setRecovery(null);
          }}
        />
      )}
    </div>
  );
}

function ScreenButton({ screens, onMove }: { screens: Screen[]; onMove: () => void }) {
  const current = Math.max(0, screens.findIndex((s) => s.current));
  const next = (current + 1) % screens.length;
  return (
    <button className="icon-btn flex items-center gap-1" title={`Move to ${screenLabel(screens[next], next)}`} onClick={onMove}>
      <Monitor size={16} />
      <span className="text-xs tabular-nums">{current + 1}</span>
    </button>
  );
}

function Elapsed() {
  const startedAt = useSession((s) => s.startedAt);
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, []);

  if (!startedAt) return <span className="text-sm text-faint">Start typing to begin</span>;
  const total = Math.max(0, Math.floor((now - startedAt) / 1000));
  const [h, m, s] = [Math.floor(total / 3600), Math.floor(total / 60) % 60, total % 60];
  const pad = (n: number) => String(n).padStart(2, "0");
  return <span className="text-sm tabular-nums text-muted">{h ? `${h}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`}</span>;
}
