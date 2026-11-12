import { useEffect } from "react";
import { toast } from "sonner";
import { FolderOpenIcon, InfoIcon, RefreshCwIcon, RotateCcwIcon } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { CopyField, ExternalLink, PageHeader } from "@/components/common";
import { api, errorText } from "@/lib/api";
import { useAppData } from "@/lib/store";

function Param({ name, children }: { name: string; children: React.ReactNode }) {
  return (
    <li>
      <code className="rounded bg-muted px-1 py-0.5 text-xs text-foreground">{name}</code> – {children}
    </li>
  );
}

export function OverlaysPage() {
  const { server, refreshServer } = useAppData();
  const base = server?.base_url ?? "http://127.0.0.1:4848";

  useEffect(() => {
    void refreshServer();
  }, [refreshServer]);

  return (
    <>
      <PageHeader title="Overlays" description="Lokale Browserquellen und Docks für OBS. Alles läuft auf deinem Rechner (127.0.0.1).">
        <Button variant="outline" onClick={() => api.openOverlaysDir().catch((e) => toast.error(errorText(e)))}>
          <FolderOpenIcon /> Overlay-Ordner öffnen
        </Button>
      </PageHeader>

      <div className="grid gap-4 p-6 xl:grid-cols-2">
        <Card className="xl:col-span-2">
          <CardHeader>
            <CardTitle>Multichat-Dock</CardTitle>
            <CardDescription>
              In OBS: <i>Docks → Benutzerdefinierte Browser-Docks</i> → Name „StreamSuite Chat“ und diese URL eintragen. Mit Filter, Zuschauerzahlen und
              Senden an alle Plattformen.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <CopyField value={`${base}/dock`} />
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Chat-Overlay</CardTitle>
            <CardDescription>Transparente Browserquelle, z. B. 400 × 600 px.</CardDescription>
          </CardHeader>
          <CardContent className="grid gap-3">
            <CopyField value={`${base}/overlay/chat`} />
            <ul className="space-y-1 text-sm text-muted-foreground">
              <Param name="?fade=30">Nachrichten nach N Sekunden ausblenden (0 = nie)</Param>
              <Param name="&platforms=twitch,tiktok">nur bestimmte Plattformen</Param>
              <Param name="&size=18">Schriftgröße in px</Param>
              <Param name="&alerts=1">Alerts zusätzlich im Chat anzeigen</Param>
              <Param name="&avatars=0">Profilbilder ausblenden</Param>
            </ul>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Alert-Overlay</CardTitle>
            <CardDescription>Transparente Browserquelle, z. B. 1920 × 1080 px. Alerts laufen nacheinander durch.</CardDescription>
          </CardHeader>
          <CardContent className="grid gap-3">
            <CopyField value={`${base}/overlay/alerts`} />
            <ul className="space-y-1 text-sm text-muted-foreground">
              <Param name="?duration=6">Anzeigedauer in Sekunden</Param>
              <Param name="&types=follow,gift,like">nur bestimmte Alert-Typen</Param>
              <Param name="&platforms=tiktok,system">Plattformen (system = kombinierte Likes)</Param>
              <Param name="&sound=/custom/sounds/ding.mp3">Sound aus deinem Overlay-Ordner</Param>
              <Param name="&size=28">Basis-Schriftgröße</Param>
            </ul>
          </CardContent>
        </Card>

        <Card className="xl:col-span-2">
          <CardHeader>
            <CardTitle className="flex items-center justify-between">
              Eigene Templates
              <span className="flex gap-1">
                <Button variant="ghost" size="sm" onClick={() => void refreshServer()}>
                  <RefreshCwIcon /> Aktualisieren
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={async () => {
                    await api.restoreTemplates();
                    await refreshServer();
                    toast.success("Beispiel-Templates wiederhergestellt");
                  }}
                >
                  <RotateCcwIcon /> Beispiele wiederherstellen
                </Button>
              </span>
            </CardTitle>
            <CardDescription>
              Jeder Unterordner mit einer <code>index.html</code> im Overlay-Ordner wird automatisch bereitgestellt. Baue dein eigenes Design mit
              HTML, CSS und JavaScript und nutze das StreamSuite-SDK (<code>/sdk.js</code>) für Chat, Alerts und Statistiken in Echtzeit. Die
              komplette Dokumentation steht in der <code>README.md</code> im Overlay-Ordner.
            </CardDescription>
          </CardHeader>
          <CardContent className="grid gap-4">
            <div className="text-xs text-muted-foreground">
              Ordner: <code className="break-all text-foreground">{server?.overlays_dir}</code>
            </div>
            {server?.custom.length ? (
              <div className="grid gap-3 md:grid-cols-2">
                {server.custom.map((name) => (
                  <div key={name} className="grid gap-1.5">
                    <span className="text-sm font-medium">{name}</span>
                    <CopyField value={`${base}/custom/${encodeURIComponent(name)}/`} />
                  </div>
                ))}
              </div>
            ) : (
              <p className="text-sm text-muted-foreground">Noch keine eigenen Templates gefunden.</p>
            )}
            <pre className="overflow-x-auto rounded-lg bg-muted p-4 text-xs leading-relaxed">{`<script src="/sdk.js"></script>
<script>
  const ss = StreamSuite.connect();
  ss.on("chat",  (ev) => console.log(ev.user.display_name, ev.message));
  ss.on("alert", (ev) => console.log(StreamSuite.helpers.formatAlert(ev)));
  ss.on("gift",  (ev) => console.log(ev.gift_name, ev.coins, "Coins"));
  ss.on("stats", (s)  => console.log(s.viewers, "Zuschauer"));
</script>`}</pre>
          </CardContent>
        </Card>

        <Alert className="xl:col-span-2">
          <InfoIcon />
          <AlertTitle>Multistreaming</AlertTitle>
          <AlertDescription>
            StreamSuite sendet selbst kein Video. Das gleichzeitige Streamen zu Twitch, YouTube und TikTok übernimmt das OBS-Plugin{" "}
            <ExternalLink href="https://aitum.tv/products/multi">Aitum Multistream</ExternalLink>. StreamSuite bündelt Chat, Alerts und Stream-Infos aller
            Plattformen.
          </AlertDescription>
        </Alert>
      </div>
    </>
  );
}
