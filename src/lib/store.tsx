import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { api, type ConnStatus, type Platform, type ServerInfo, type Settings, type SSEvent, type Stats } from "./api";

const MAX_CHAT = 500;
const MAX_ALERTS = 200;

interface AppData {
  statuses: Partial<Record<Platform, ConnStatus>>;
  chat: SSEvent[];
  alerts: SSEvent[];
  deleted: Set<string>;
  stats: Stats | null;
  settings: Settings | null;
  server: ServerInfo | null;
  setSettings: (s: Settings) => void;
  refreshSettings: () => Promise<void>;
  refreshServer: () => Promise<void>;
  clearHistory: () => Promise<void>;
}

const Ctx = createContext<AppData | null>(null);

export function AppDataProvider({ children }: { children: ReactNode }) {
  const [statuses, setStatuses] = useState<Partial<Record<Platform, ConnStatus>>>({});
  const [chat, setChat] = useState<SSEvent[]>([]);
  const [alerts, setAlerts] = useState<SSEvent[]>([]);
  const [deleted, setDeleted] = useState<Set<string>>(new Set());
  const [stats, setStats] = useState<Stats | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [server, setServer] = useState<ServerInfo | null>(null);

  const refreshSettings = useCallback(async () => setSettings(await api.getSettings()), []);
  const refreshServer = useCallback(async () => setServer(await api.serverInfo()), []);

  useEffect(() => {
    const unlisten: Promise<() => void>[] = [];

    unlisten.push(
      listen<SSEvent>("ss://event", ({ payload: ev }) => {
        if (ev.type === "stats") {
          setStats(ev.data as unknown as Stats);
        } else if (ev.type === "delete") {
          const id = ev.data?.message_id as string | undefined;
          if (id) setDeleted((d) => new Set(d).add(id));
        } else if (ev.category === "chat") {
          setChat((c) => [...c.slice(-(MAX_CHAT - 1)), ev]);
        } else if (ev.category === "alert") {
          setAlerts((a) => [...a.slice(-(MAX_ALERTS - 1)), ev]);
        }
      }),
    );
    unlisten.push(
      listen<ConnStatus>("ss://status", ({ payload }) => setStatuses((s) => ({ ...s, [payload.platform]: payload }))),
    );
    unlisten.push(listen("ss://server", () => void refreshServer()));
    unlisten.push(
      listen<{ platform: Platform; ok: boolean; message: string }>("ss://auth", ({ payload }) => {
        if (payload.ok) toast.success(payload.message);
        else toast.error(payload.message);
        void refreshSettings();
      }),
    );

    // Initialer Zustand (nach dem Registrieren der Listener, damit nichts verloren geht)
    void (async () => {
      const [st, hist, stt] = await Promise.all([api.getStatuses(), api.getHistory(), api.getStats()]);
      setStatuses(Object.fromEntries(st.map((s) => [s.platform, s])));
      setChat(hist.chat.filter((e) => e.type === "chat"));
      setAlerts(hist.alerts);
      setStats(stt);
      await Promise.all([refreshSettings(), refreshServer()]);
    })();

    return () => unlisten.forEach((p) => void p.then((f) => f()));
  }, [refreshServer, refreshSettings]);

  const clearHistory = useCallback(async () => {
    await api.clearHistory();
    setChat([]);
    setAlerts([]);
    setDeleted(new Set());
  }, []);

  const value = useMemo(
    () => ({ statuses, chat, alerts, deleted, stats, settings, server, setSettings, refreshSettings, refreshServer, clearHistory }),
    [statuses, chat, alerts, deleted, stats, settings, server, refreshSettings, refreshServer, clearHistory],
  );
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useAppData() {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("useAppData außerhalb des Providers");
  return ctx;
}
