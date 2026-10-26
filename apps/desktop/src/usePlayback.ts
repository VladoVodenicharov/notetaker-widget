import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { LazyStore } from "@tauri-apps/plugin-store";
import type { Editor } from "@tiptap/core";
import { useSession } from "./session";
import { canControl, idlePause, onTick, onTyped, type Step, type TypingPause } from "./playback";
import type { Snapshot, Source } from "./sources";

const settings = new LazyStore("settings.json");

/** Sends play, pause or toggle to the source's own player (Rust `media_command`). */
const command = (action: "play" | "pause" | "toggle", source: Source) =>
  invoke<boolean>("media_command", { action, app: source.app, title: source.title }).catch(() => false);

/** The source's latest state: the session keeps the copy from when it was chosen, the snapshot is live. */
const live = (snapshot: Snapshot, source: Source | null) => snapshot.candidates.find((c) => c.key === source?.key) ?? source;

/**
 * Play / pause for the source being watched (the panel button and Ctrl+Alt+Space),
 * plus the "pause video while I type" option, which is off by default.
 */
export function usePlayback(editor: Editor | null, snapshot: Snapshot) {
  const sessionSource = useSession((s) => s.source);
  const source = live(snapshot, sessionSource);
  const latest = useRef({ snapshot, source });
  latest.current = { snapshot, source };

  const [pauseWhileTyping, setPauseWhileTyping] = useState(false);
  useEffect(() => {
    void settings.get<boolean>("pauseWhileTyping").then((on) => setPauseWhileTyping(on === true));
  }, []);
  const typing = useRef<TypingPause>(idlePause);
  const enabled = useRef(pauseWhileTyping);
  enabled.current = pauseWhileTyping;

  const togglePauseWhileTyping = () => {
    const on = !pauseWhileTyping;
    setPauseWhileTyping(on);
    void settings.set("pauseWhileTyping", on);
    typing.current = idlePause;
  };

  const toggle = useCallback(() => {
    const { source } = latest.current;
    if (canControl(source)) {
      typing.current = idlePause; // the user took over, so nothing is left to resume
      void command("toggle", source);
    }
  }, []);

  // Ctrl+Alt+Space from anywhere.
  useEffect(() => {
    const unlisten = listen("toggle-playback", toggle);
    return () => void unlisten.then((f) => f());
  }, [toggle]);

  useEffect(() => {
    if (!editor) return;
    const run = (step: Step) => {
      typing.current = step.state;
      if (step.send) void command(step.send.action, step.send.source);
    };
    const typed = () => run(onTyped(typing.current, Date.now(), enabled.current, latest.current.source));
    editor.on("update", typed);
    const timer = setInterval(() => {
      const paused = typing.current.paused;
      run(onTick(typing.current, Date.now(), paused ? (live(latest.current.snapshot, paused)?.isPlaying ?? null) : null));
    }, 1000);
    return () => {
      editor.off("update", typed);
      clearInterval(timer);
    };
  }, [editor]);

  return { source, toggle, pauseWhileTyping, togglePauseWhileTyping };
}
