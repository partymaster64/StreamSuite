import { BellIcon, CoinsIcon, EyeIcon, HeartIcon, MessageSquareIcon, UserPlusIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardAction, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { AlertLine, ChatLine, EmptyState, PageHeader, PlatformIcon, StatusBadge } from "@/components/common";
import { PLATFORM_META, PLATFORMS, type Stats } from "@/lib/api";
import { useAppData } from "@/lib/store";
import type { Page } from "@/App";

function sum(stats: Stats | null, key: "session_follows" | "session_subs" | "session_coins" | "session_messages") {
  return PLATFORMS.reduce((acc, p) => acc + (stats?.platforms[p]?.[key] ?? 0), 0);
}

function StatCard({ icon, label, value, detail }: { icon: React.ReactNode; label: string; value: string; detail?: React.ReactNode }) {
  return (
    <Card size="sm">
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-sm font-medium text-muted-foreground [&_svg]:size-4">
          {icon} {label}
        </CardTitle>
      </CardHeader>
      <CardContent>
        <div className="text-2xl font-semibold tabular-nums">{value}</div>
        {detail && <div className="mt-1 flex flex-wrap gap-x-3 gap-y-0.5 text-xs text-muted-foreground">{detail}</div>}
      </CardContent>
    </Card>
  );
}

export function DashboardPage({ navigate }: { navigate: (p: Page) => void }) {
  const { statuses, stats, chat, alerts, deleted } = useAppData();
  const n = (v: number | null | undefined) => (v == null ? "–" : v.toLocaleString("de-DE"));

  const perPlatform = (key: "viewers" | "total_likes") =>
    PLATFORMS.filter((p) => stats?.platforms[p]?.[key] != null).map((p) => (
      <span key={p} className="flex items-center gap-1">
        <PlatformIcon platform={p} className="size-3" /> {n(stats?.platforms[p]?.[key])}
      </span>
    ));

  return (
    <>
      <PageHeader title="Übersicht" description="Alle Plattformen auf einen Blick." />
      <div className="grid gap-4 p-6">
        <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-5">
          <StatCard icon={<EyeIcon />} label="Zuschauer" value={n(stats?.viewers)} detail={perPlatform("viewers")} />
          <StatCard icon={<HeartIcon />} label="Likes" value={n(stats?.likes)} detail={perPlatform("total_likes")} />
          <StatCard icon={<UserPlusIcon />} label="Neue Follower" value={n(sum(stats, "session_follows"))} detail="in dieser Sitzung" />
          <StatCard icon={<CoinsIcon />} label="TikTok-Coins" value={n(sum(stats, "session_coins"))} detail="in dieser Sitzung" />
          <StatCard icon={<MessageSquareIcon />} label="Nachrichten" value={n(sum(stats, "session_messages"))} detail="in dieser Sitzung" />
        </div>

        <div className="grid gap-4 lg:grid-cols-3">
          <Card>
            <CardHeader>
              <CardTitle>Verbindungen</CardTitle>
              <CardAction>
                <Button variant="ghost" size="sm" onClick={() => navigate("connections")}>
                  Verwalten
                </Button>
              </CardAction>
            </CardHeader>
            <CardContent className="grid gap-3">
              {PLATFORMS.map((p) => (
                <div key={p} className="flex items-center justify-between gap-2">
                  <div className="flex min-w-0 items-center gap-2">
                    <PlatformIcon platform={p} className="size-5" />
                    <div className="min-w-0">
                      <div className="text-sm font-medium">{PLATFORM_META[p].name}</div>
                      <div className="truncate text-xs text-muted-foreground">{statuses[p]?.account ?? "—"}</div>
                    </div>
                  </div>
                  <StatusBadge status={statuses[p]} />
                </div>
              ))}
            </CardContent>
          </Card>

          <Card className="lg:col-span-2">
            <CardHeader>
              <CardTitle>Letzte Alerts</CardTitle>
              <CardAction>
                <Button variant="ghost" size="sm" onClick={() => navigate("alerts")}>
                  Alle anzeigen
                </Button>
              </CardAction>
            </CardHeader>
            <CardContent className="grid max-h-72 gap-2 overflow-y-auto">
              {alerts.length === 0 ? (
                <EmptyState icon={<BellIcon />} title="Noch keine Alerts" />
              ) : (
                [...alerts].reverse().slice(0, 8).map((ev) => <AlertLine key={ev.id} ev={ev} />)
              )}
            </CardContent>
          </Card>
        </div>

        <Card>
          <CardHeader>
            <CardTitle>Chat</CardTitle>
            <CardAction>
              <Button variant="ghost" size="sm" onClick={() => navigate("chat")}>
                Multichat öffnen
              </Button>
            </CardAction>
          </CardHeader>
          <CardContent className="max-h-80 overflow-y-auto">
            {chat.length === 0 ? (
              <EmptyState icon={<MessageSquareIcon />} title="Noch keine Nachrichten" />
            ) : (
              chat.slice(-12).map((ev) => <ChatLine key={ev.id} ev={ev} deleted={deleted.has((ev.data?.message_id as string) ?? "")} />)
            )}
          </CardContent>
        </Card>
      </div>
    </>
  );
}
