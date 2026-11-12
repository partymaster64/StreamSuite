import { useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import { BellIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { AlertLine, EmptyState, PageHeader, PlatformIcon } from "@/components/common";
import { api, errorText, type EventPlatform } from "@/lib/api";
import { useAppData } from "@/lib/store";

const TYPES: { value: string; label: string }[] = [
  { value: "all", label: "Alle Alerts" },
  { value: "follow", label: "Follows" },
  { value: "subscribe", label: "Abos / Abonnenten" },
  { value: "resub", label: "Resubs / Meilensteine" },
  { value: "gift_sub", label: "Geschenkte Abos" },
  { value: "membership", label: "Mitgliedschaften" },
  { value: "gift", label: "TikTok-Geschenke" },
  { value: "like", label: "Likes (kombiniert)" },
  { value: "cheer", label: "Bits" },
  { value: "superchat", label: "Super Chats" },
  { value: "share", label: "Shares" },
  { value: "raid", label: "Raids" },
];

const TESTS: { kind: string; platform: EventPlatform; label: string }[] = [
  { kind: "follow", platform: "twitch", label: "Follow" },
  { kind: "follow", platform: "tiktok", label: "Follow" },
  { kind: "subscribe", platform: "youtube", label: "Abonnent" },
  { kind: "subscribe", platform: "twitch", label: "Abo" },
  { kind: "gift", platform: "tiktok", label: "Geschenk" },
  { kind: "like", platform: "system", label: "Likes" },
  { kind: "superchat", platform: "youtube", label: "Super Chat" },
  { kind: "membership", platform: "youtube", label: "Mitglied" },
  { kind: "cheer", platform: "twitch", label: "Bits" },
  { kind: "raid", platform: "twitch", label: "Raid" },
  { kind: "gift_sub", platform: "twitch", label: "Sub-Gift" },
  { kind: "share", platform: "tiktok", label: "Share" },
  { kind: "chat", platform: "youtube", label: "Chat" },
];

function AlertSettingsCard() {
  const { settings, setSettings } = useAppData();
  const [interval, setInterval] = useState(15);
  const [minLikes, setMinLikes] = useState(1);
  const [minCoins, setMinCoins] = useState(1);

  useEffect(() => {
    if (!settings) return;
    setInterval(settings.alerts.like_interval_secs);
    setMinLikes(settings.alerts.like_min_count);
    setMinCoins(settings.alerts.tiktok_min_coins);
  }, [settings]);

  const save = async () => {
    try {
      setSettings(
        await api.saveSettings({
          alerts: {
            like_interval_secs: Math.max(3, interval),
            like_min_count: Math.max(1, minLikes),
            tiktok_min_coins: Math.max(0, minCoins),
          },
        }),
      );
      toast.success("Alert-Einstellungen gespeichert");
    } catch (e) {
      toast.error(errorText(e));
    }
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle>Einstellungen</CardTitle>
        <CardDescription>
          Likes von TikTok und YouTube werden gesammelt und als ein kombinierter Like-Alert ausgegeben.
        </CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4">
        <div className="grid gap-1.5">
          <Label>Like-Intervall (Sekunden)</Label>
          <Input type="number" min={3} value={interval} onChange={(e) => setInterval(Number(e.target.value))} />
        </div>
        <div className="grid gap-1.5">
          <Label>Mindestanzahl Likes pro Alert</Label>
          <Input type="number" min={1} value={minLikes} onChange={(e) => setMinLikes(Number(e.target.value))} />
        </div>
        <div className="grid gap-1.5">
          <Label>Min. TikTok-Coins für Geschenk-Alert</Label>
          <Input type="number" min={0} value={minCoins} onChange={(e) => setMinCoins(Number(e.target.value))} />
        </div>
        <Button onClick={save}>Speichern</Button>
      </CardContent>
    </Card>
  );
}

export function AlertsPage() {
  const { alerts } = useAppData();
  const [type, setType] = useState("all");
  const list = useMemo(
    () => [...alerts].reverse().filter((a) => type === "all" || a.type === type || (type === "superchat" && a.type === "supersticker")),
    [alerts, type],
  );

  return (
    <div className="flex h-full flex-col">
      <PageHeader title="Alerts" description="Follows, Abos, Likes, TikTok-Geschenke und mehr von allen Plattformen.">
        <Select value={type} onValueChange={setType}>
          <SelectTrigger className="w-52">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {TYPES.map((t) => (
              <SelectItem key={t.value} value={t.value}>
                {t.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </PageHeader>
      <div className="grid min-h-0 flex-1 gap-4 p-6 lg:grid-cols-[1fr_320px]">
        <div className="min-h-0 space-y-2 overflow-y-auto pr-1">
          {list.length === 0 ? (
            <EmptyState icon={<BellIcon />} title="Noch keine Alerts">
              Löse rechts einen Test-Alert aus, um deine Overlays zu prüfen.
            </EmptyState>
          ) : (
            list.map((ev) => <AlertLine key={ev.id} ev={ev} />)
          )}
        </div>
        <div className="space-y-4 overflow-y-auto">
          <Card>
            <CardHeader>
              <CardTitle>Test-Alerts</CardTitle>
              <CardDescription>Werden an die App und alle Overlays gesendet.</CardDescription>
            </CardHeader>
            <CardContent className="grid grid-cols-2 gap-2">
              {TESTS.map((t) => (
                <Button
                  key={t.kind + t.platform}
                  variant="outline"
                  size="sm"
                  className="justify-start"
                  onClick={() => api.testEvent(t.kind, t.platform).catch((e) => toast.error(errorText(e)))}
                >
                  <PlatformIcon platform={t.platform} /> {t.label}
                </Button>
              ))}
            </CardContent>
          </Card>
          <AlertSettingsCard />
        </div>
      </div>
    </div>
  );
}
