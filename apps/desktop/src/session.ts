import { create } from "zustand";
import { LazyStore } from "@tauri-apps/plugin-store";
import { open } from "@tauri-apps/plugin-dialog";

const settings = new LazyStore("settings.json");

type Session = {
  id: string;
  title: string;
  /** Where the next note is saved; remembered across sessions. */
  folder: string | null;
  /** Set on the first keystroke, so a panel left open all day doesn't skew the times. */
  startedAt: number | null;
  setTitle: (title: string) => void;
  setFolder: (folder: string) => void;
  markStarted: () => void;
  reset: () => void;
};

export const useSession = create<Session>()((set) => ({
  id: crypto.randomUUID(),
  title: "",
  folder: null,
  startedAt: null,
  setTitle: (title) => set({ title }),
  setFolder: (folder) => {
    set({ folder });
    void settings.set("lastFolder", folder);
  },
  markStarted: () => set((s) => (s.startedAt ? s : { startedAt: Date.now() })),
  reset: () => set({ id: crypto.randomUUID(), title: "", startedAt: null }),
}));

void settings.get<string>("lastFolder").then((folder) => {
  if (folder) useSession.setState((s) => (s.folder ? s : { folder }));
});

export async function pickFolder(): Promise<string | null> {
  const current = useSession.getState().folder;
  const picked = await open({ directory: true, defaultPath: current ?? undefined, title: "Folder for your notes" });
  if (typeof picked !== "string") return null;
  useSession.getState().setFolder(picked);
  return picked;
}
