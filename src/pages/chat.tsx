import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";
import { ArrowDownIcon, BellIcon, MessagesSquareIcon, SendIcon, Trash2Icon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { AlertLine, ChatLine, EmptyState, PageHeader, PlatformIcon } from "@/components/common";
import { TiktokModMenu } from "@/components/tiktok-mod-menu";
import { cn } from "@/lib/utils";
import { api, errorText, PLATFORM_META, PLATFORMS, type Platform, type SSEvent } from "@/lib/api";
import { useAppData } from "@/lib/store";

export function ChatPage() {
  const { chat, alerts, deleted, stats, settings, statuses, clearHistory } = useAppData();
  // Moderation nur im eigenen, laufenden LIVE mit Anmeldung
  const canModerateTiktok = !!settings?.tiktok.authorized && !!statuses.tiktok?.live;
  const modActions = (ev: SSEvent) =>
    canModerateTiktok && ev.platform === "tiktok" && !ev.test && ev.user && !ev.user.is_owner && /^\d+$/.test(ev.user.id) ? (
      <TiktokModMenu user={ev.user} />
    ) : undefined;
  const [filter, setFilter] = useState<Record<Platform, boolean>>({ twitch: true, youtube: true, tiktok: true });
  const [showAlerts, setShowAlerts] = useState(true);
  const [target, setTarget] = useState("all");
  const [text, setText] = useState("");
  const [sending, setSending] = useState(false);
  const [stuck, setStuck] = useState(true);
  const listRef = useRef<HTMLDivElement>(null);

  const items = useMemo(() => {
    const all: SSEvent[] = [
      ...chat.filter((e) => e.platform !== "system" && filter[e.platform]),
      ...(showAlerts ? alerts : []),
    ];
    return all.sort((a, b) => a.timestamp - b.timestamp).slice(-500);
  }, [chat, alerts, filter, showAlerts]);

  useLayoutEffect(() => {
    const el = listRef.current;
    if (el && stuck) el.scrollTop = el.scrollHeight;
  }, [items, stuck]);

  useEffect(() => {
    const el = listRef.current;
    if (!el) return;
    const onScroll = () => setStuck(el.scrollHeight - el.scrollTop - el.clientHeight < 40);
    el.addEventListener("scroll", onScroll);
    return () => el.removeEventListener("scroll", onScroll);
  }, []);

  const send = async (e: React.FormEvent) => {
    e.preventDefault();
    const msg = text.trim();
    if (!msg) return;
    setSending(true);
    try {
      const res = await api.sendChat(target, msg);
      const entries = Object.entries(res);
      if (entries.length === 0) toast.error("Keine verbundene Plattform zum Senden");
      for (const [p, r] of entries) if (!r.ok) toast.error(`${PLATFORM_META[p as Platform].name}: ${r.error}`);
      if (entries.some(([, r]) => r.ok)) setText("");
    } catch (err) {
      toast.error(errorText(err));
    } finally {
      setSending(false);
    }
  };

  return (
    <div className="flex h-full flex-col">
      <PageHeader title="Multichat" description="Alle Chats in einem Fenster.">
        {PLATFORMS.map((p) => (
          <Button
            key={p}
            variant="outline"
            size="sm"
            className={cn(!filter[p] && "opacity-40")}
            onClick={() => setFilter((f) => ({ ...f, [p]: !f[p] }))}
            title={`${PLATFORM_META[p].name} ein-/ausblenden`}
          >
            <PlatformIcon platform={p} />
            <span className="tabular-nums">{stats?.platforms[p]?.viewers?.toLocaleString("de-DE") ?? "–"}</span>
          </Button>
        ))}
        <Button variant="outline" size="sm" className={cn(!showAlerts && "opacity-40")} onClick={() => setShowAlerts((s) => !s)}>
          <BellIcon /> Alerts
        </Button>
        <Button variant="ghost" size="sm" onClick={() => void clearHistory()} title="Verlauf leeren">
          <Trash2Icon />
        </Button>
      </PageHeader>

      <div className="relative min-h-0 flex-1">
        <div ref={listRef} className="h-full space-y-0.5 overflow-y-auto px-4 py-3">
          {items.length === 0 ? (
            <EmptyState icon={<MessagesSquareIcon />} title="Noch keine Nachrichten">
              Sobald eine verbundene Plattform live ist, erscheinen hier alle Chatnachrichten.
            </EmptyState>
          ) : (
            items.map((ev) =>
              ev.category === "alert" ? (
                <div key={ev.id} className="py-1">
                  <AlertLine ev={ev} />
                </div>
              ) : (
                <ChatLine key={ev.id} ev={ev} deleted={deleted.has((ev.data?.message_id as string) ?? "")} actions={modActions(ev)} />
              ),
            )
          )}
        </div>
        {!stuck && (
          <Button
            size="sm"
            variant="secondary"
            className="absolute bottom-3 left-1/2 -translate-x-1/2 shadow"
            onClick={() => {
              setStuck(true);
              listRef.current?.scrollTo({ top: listRef.current.scrollHeight });
            }}
          >
            <ArrowDownIcon /> Neue Nachrichten
          </Button>
        )}
      </div>

      <form onSubmit={send} className="flex gap-2 border-t p-3">
        <Select value={target} onValueChange={setTarget}>
          <SelectTrigger className="w-36">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">An alle</SelectItem>
            <SelectItem value="twitch">Nur Twitch</SelectItem>
            <SelectItem value="youtube">Nur YouTube</SelectItem>
            <SelectItem value="tiktok">Nur TikTok</SelectItem>
          </SelectContent>
        </Select>
        <Input value={text} onChange={(e) => setText(e.target.value)} placeholder="Nachricht an den Chat …" maxLength={200} />
        <Button type="submit" disabled={sending || !text.trim()}>
          <SendIcon /> Senden
        </Button>
      </form>
    </div>
  );
}
