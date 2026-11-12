import { useState } from "react";
import { BellIcon, LayoutDashboardIcon, LayersIcon, MessagesSquareIcon, PencilLineIcon, PlugIcon, SettingsIcon } from "lucide-react";
import { Toaster } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { PlatformIcon } from "@/components/common";
import { cn } from "@/lib/utils";
import { PLATFORMS } from "@/lib/api";
import { AppDataProvider, useAppData } from "@/lib/store";
import { DashboardPage } from "@/pages/dashboard";
import { ConnectionsPage } from "@/pages/connections";
import { ChatPage } from "@/pages/chat";
import { AlertsPage } from "@/pages/alerts";
import { StreamInfoPage } from "@/pages/stream-info";
import { OverlaysPage } from "@/pages/overlays";
import { SettingsPage } from "@/pages/settings";

export type Page = "dashboard" | "connections" | "chat" | "alerts" | "stream" | "overlays" | "settings";

const NAV: { id: Page; label: string; icon: typeof PlugIcon }[] = [
  { id: "dashboard", label: "Übersicht", icon: LayoutDashboardIcon },
  { id: "connections", label: "Verbindungen", icon: PlugIcon },
  { id: "chat", label: "Multichat", icon: MessagesSquareIcon },
  { id: "alerts", label: "Alerts", icon: BellIcon },
  { id: "stream", label: "Stream-Info", icon: PencilLineIcon },
  { id: "overlays", label: "Overlays", icon: LayersIcon },
];

function Sidebar({ page, setPage }: { page: Page; setPage: (p: Page) => void }) {
  const { statuses } = useAppData();
  const item = (id: Page, label: string, Icon: typeof PlugIcon) => (
    <button
      key={id}
      onClick={() => setPage(id)}
      className={cn(
        "flex w-full items-center gap-2.5 rounded-lg px-2.5 py-1.5 text-sm transition-colors",
        page === id ? "bg-muted font-medium text-foreground" : "text-muted-foreground hover:bg-muted/60 hover:text-foreground",
      )}
    >
      <Icon className="size-4" />
      {label}
    </button>
  );
  return (
    <aside className="flex w-56 shrink-0 flex-col border-r bg-sidebar">
      <div className="flex items-center gap-2 px-4 py-4">
        <div className="grid size-7 place-items-center rounded-lg bg-primary text-primary-foreground">
          <LayersIcon className="size-4" />
        </div>
        <span className="font-semibold tracking-tight">StreamSuite</span>
      </div>
      <nav className="grid gap-0.5 px-2">{NAV.map((n) => item(n.id, n.label, n.icon))}</nav>
      <div className="mt-auto grid gap-2 px-2 pb-3">
        <div className="grid gap-1.5 rounded-lg border px-3 py-2.5">
          {PLATFORMS.map((p) => {
            const st = statuses[p];
            const dot =
              st?.state === "connected" ? "bg-emerald-500" : st?.state === "connecting" ? "bg-amber-500 animate-pulse" : st?.state === "error" ? "bg-red-500" : "bg-muted-foreground/40";
            return (
              <div key={p} className="flex items-center gap-2 text-xs">
                <PlatformIcon platform={p} className="size-3.5" />
                <span className="flex-1 truncate text-muted-foreground">{st?.account ?? "Nicht verbunden"}</span>
                {st?.live && <span className="rounded bg-red-600 px-1 text-[10px] font-semibold text-white">LIVE</span>}
                <span className={cn("size-2 rounded-full", dot)} />
              </div>
            );
          })}
        </div>
        {item("settings", "Einstellungen", SettingsIcon)}
      </div>
    </aside>
  );
}

function Shell() {
  const [page, setPage] = useState<Page>("dashboard");
  return (
    <div className="flex h-screen overflow-hidden bg-background text-foreground">
      <Sidebar page={page} setPage={setPage} />
      <main className="min-w-0 flex-1 overflow-y-auto">
        {page === "dashboard" && <DashboardPage navigate={setPage} />}
        {page === "connections" && <ConnectionsPage />}
        {page === "chat" && <ChatPage />}
        {page === "alerts" && <AlertsPage />}
        {page === "stream" && <StreamInfoPage />}
        {page === "overlays" && <OverlaysPage />}
        {page === "settings" && <SettingsPage />}
      </main>
    </div>
  );
}

export default function App() {
  return (
    <AppDataProvider>
      <TooltipProvider>
        <Shell />
        <Toaster position="bottom-right" richColors />
      </TooltipProvider>
    </AppDataProvider>
  );
}
