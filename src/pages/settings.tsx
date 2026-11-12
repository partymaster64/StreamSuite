import { useEffect, useState } from "react";
import { toast } from "sonner";
import { ChevronDownIcon, MoonIcon, SunIcon } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { ExternalLink, PageHeader, PlatformIcon } from "@/components/common";
import { api, errorText } from "@/lib/api";
import { useAppData } from "@/lib/store";
import { useTheme } from "@/lib/theme";
import { cn } from "@/lib/utils";

function AdvancedCredentials() {
  const { settings, setSettings } = useAppData();
  const [open, setOpen] = useState(false);
  const [twitch, setTwitch] = useState("");
  const [gId, setGId] = useState("");
  const [gSecret, setGSecret] = useState("");
  const [euler, setEuler] = useState("");

  useEffect(() => {
    if (!settings) return;
    setTwitch(settings.twitch.client_id);
    setGId(settings.youtube.client_id);
    setGSecret(settings.youtube.client_secret);
    setEuler(settings.tiktok.api_key);
  }, [settings]);

  const save = async () => {
    try {
      setSettings(
        await api.saveSettings({
          twitch: { client_id: twitch.trim() },
          youtube: { client_id: gId.trim(), client_secret: gSecret.trim() },
          tiktok: { api_key: euler.trim() },
        }),
      );
      toast.success("Zugangsdaten gespeichert");
    } catch (e) {
      toast.error(errorText(e));
    }
  };

  const embedded = (p: "twitch" | "youtube" | "tiktok") =>
    settings?.embedded[p] ? (
      <Badge variant="outline" className="h-4 px-1 text-[10px]">
        in der App enthalten
      </Badge>
    ) : (
      <Badge variant="destructive" className="h-4 px-1 text-[10px]">
        fehlt
      </Badge>
    );

  return (
    <Card>
      <CardHeader>
        <button className="flex w-full items-center justify-between text-left" onClick={() => setOpen((o) => !o)}>
          <div className="grid gap-1">
            <CardTitle>Erweitert: eigene API-Zugangsdaten</CardTitle>
            <CardDescription>Optional. Leer lassen, um die in der App enthaltenen Zugangsdaten zu nutzen.</CardDescription>
          </div>
          <ChevronDownIcon className={cn("size-4 transition-transform", open && "rotate-180")} />
        </button>
      </CardHeader>
      {open && (
        <CardContent className="grid gap-4">
          <div className="grid gap-1.5">
            <Label className="flex items-center gap-2">
              <PlatformIcon platform="twitch" /> Twitch Client-ID {embedded("twitch")}
            </Label>
            <Input value={twitch} onChange={(e) => setTwitch(e.target.value)} placeholder="leer = Standard" />
            <p className="text-xs text-muted-foreground">
              <ExternalLink href="https://dev.twitch.tv/console/apps/create">dev.twitch.tv</ExternalLink> → Client-Typ „Öffentlich“, Redirect-URL{" "}
              <code>http://localhost</code>
            </p>
          </div>
          <div className="grid gap-4 sm:grid-cols-2">
            <div className="grid gap-1.5">
              <Label className="flex items-center gap-2">
                <PlatformIcon platform="youtube" /> Google Client-ID {embedded("youtube")}
              </Label>
              <Input value={gId} onChange={(e) => setGId(e.target.value)} placeholder="leer = Standard" />
            </div>
            <div className="grid gap-1.5">
              <Label>Google Client-Secret</Label>
              <Input type="password" value={gSecret} onChange={(e) => setGSecret(e.target.value)} placeholder="leer = Standard" />
            </div>
          </div>
          <div className="grid gap-1.5">
            <Label className="flex items-center gap-2">
              <PlatformIcon platform="tiktok" /> EulerStream API-Key {embedded("tiktok")}
            </Label>
            <Input type="password" value={euler} onChange={(e) => setEuler(e.target.value)} placeholder="leer = Standard" />
            <p className="text-xs text-muted-foreground">
              Kostenlos auf <ExternalLink href="https://www.eulerstream.com">eulerstream.com</ExternalLink> (Community-Tarif)
            </p>
          </div>
          <Button className="w-fit" onClick={save}>
            Zugangsdaten speichern
          </Button>
        </CardContent>
      )}
    </Card>
  );
}

export function SettingsPage() {
  const { settings, setSettings, server, clearHistory } = useAppData();
  const { theme, setTheme } = useTheme();
  const [port, setPort] = useState(4848);
  const [pollMs, setPollMs] = useState(8000);
  const [subSecs, setSubSecs] = useState(60);

  useEffect(() => {
    if (!settings) return;
    setPort(settings.server_port);
    setPollMs(settings.youtube.min_chat_poll_ms);
    setSubSecs(settings.youtube.subscriber_poll_secs);
  }, [settings]);

  const save = async () => {
    try {
      setSettings(
        await api.saveSettings({
          server_port: Math.min(65535, Math.max(1024, port)),
          youtube: { min_chat_poll_ms: Math.max(2000, pollMs), subscriber_poll_secs: Math.max(30, subSecs) },
        }),
      );
      toast.success(port !== server?.port ? "Gespeichert – der neue Port gilt nach einem Neustart der App" : "Gespeichert");
    } catch (e) {
      toast.error(errorText(e));
    }
  };

  const setCloseToTray = async (v: boolean) => {
    try {
      setSettings(await api.saveSettings({ close_to_tray: v }));
    } catch (e) {
      toast.error(errorText(e));
    }
  };

  const unitsPerHour = Math.round((3600_000 / Math.max(2000, pollMs)) * 5 + 120 + 3600 / Math.max(30, subSecs));

  return (
    <>
      <PageHeader title="Einstellungen" />
      <div className="grid max-w-3xl gap-4 p-6">
        <Card>
          <CardHeader>
            <CardTitle>Allgemein</CardTitle>
          </CardHeader>
          <CardContent className="grid gap-5">
            <label className="flex items-start justify-between gap-4">
              <span className="grid gap-1">
                <span className="text-sm font-medium">Im Hintergrund weiterlaufen</span>
                <span className="text-xs text-muted-foreground">
                  Beim Schließen des Fensters laufen Chat, Alerts und Overlays weiter.{" "}
                  {server?.tray
                    ? "Über das Symbol in der Taskleiste/Menüleiste öffnest du StreamSuite wieder."
                    : "Starte StreamSuite erneut, um das Fenster wieder zu öffnen (für ein Tray-Symbol unter Linux libayatana-appindicator installieren)."}
                </span>
              </span>
              <Switch checked={!!settings?.close_to_tray} onCheckedChange={setCloseToTray} />
            </label>
            <div className="flex items-center justify-between gap-4">
              <span className="text-sm font-medium">Darstellung</span>
              <div className="flex gap-2">
                <Button size="sm" variant={theme === "dark" ? "default" : "outline"} onClick={() => setTheme("dark")}>
                  <MoonIcon /> Dunkel
                </Button>
                <Button size="sm" variant={theme === "light" ? "default" : "outline"} onClick={() => setTheme("light")}>
                  <SunIcon /> Hell
                </Button>
              </div>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Overlay-Server</CardTitle>
            <CardDescription>
              Läuft lokal auf <code>127.0.0.1</code>. Aktuell: Port {server?.port ?? "–"}. Ist der Port belegt, wird automatisch der nächste freie
              verwendet.
            </CardDescription>
          </CardHeader>
          <CardContent className="grid gap-1.5">
            <Label>Bevorzugter Port</Label>
            <Input type="number" min={1024} max={65535} value={port} onChange={(e) => setPort(Number(e.target.value))} className="w-40" />
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>YouTube-API</CardTitle>
            <CardDescription>
              Die YouTube-API hat ein Tageskontingent (Standard 10.000 Einheiten). Geschätzter Verbrauch: ca.{" "}
              <b className="text-foreground">{unitsPerHour.toLocaleString("de-DE")} Einheiten pro Stunde</b> Livestream.
            </CardDescription>
          </CardHeader>
          <CardContent className="grid gap-4 sm:grid-cols-2">
            <div className="grid gap-1.5">
              <Label>Min. Chat-Abfrageintervall (ms)</Label>
              <Input type="number" min={2000} step={500} value={pollMs} onChange={(e) => setPollMs(Number(e.target.value))} />
            </div>
            <div className="grid gap-1.5">
              <Label>Abonnenten-Abfrage (Sekunden)</Label>
              <Input type="number" min={30} value={subSecs} onChange={(e) => setSubSecs(Number(e.target.value))} />
            </div>
          </CardContent>
        </Card>

        <div className="flex gap-2">
          <Button onClick={save}>Speichern</Button>
          <Button
            variant="outline"
            onClick={async () => {
              await clearHistory();
              toast.success("Chat- und Alert-Verlauf geleert");
            }}
          >
            Verlauf leeren
          </Button>
        </div>

        <AdvancedCredentials />
      </div>
    </>
  );
}
