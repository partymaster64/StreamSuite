import { useEffect, useState, type ReactNode } from "react";
import { toast } from "sonner";
import { openUrl } from "@tauri-apps/plugin-opener";
import { AppWindowIcon, Loader2Icon, LogInIcon, LogOutIcon, PlugIcon, PlugZapIcon, TriangleAlertIcon } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardAction, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { PageHeader, PlatformIcon, StatusBadge } from "@/components/common";
import { api, errorText, type DeviceAuth, type Platform } from "@/lib/api";
import { useAppData } from "@/lib/store";

async function run(fn: () => Promise<unknown>) {
  try {
    await fn();
  } catch (e) {
    toast.error(errorText(e));
  }
}

function MissingCredentials() {
  return (
    <Alert>
      <TriangleAlertIcon />
      <AlertTitle>App-Zugangsdaten fehlen</AlertTitle>
      <AlertDescription>
        In diesem Build sind keine Zugangsdaten für diese Plattform hinterlegt. Trage sie unter Einstellungen → Erweitert ein oder baue die App mit
        einer credentials.json (siehe README).
      </AlertDescription>
    </Alert>
  );
}

function PlatformCard({
  platform,
  title,
  description,
  loggedIn,
  children,
}: {
  platform: Platform;
  title: string;
  description: string;
  loggedIn: boolean;
  children?: ReactNode;
}) {
  const { statuses } = useAppData();
  const st = statuses[platform];
  const active = st && ["connected", "connecting", "error"].includes(st.state);
  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <PlatformIcon platform={platform} className="size-5" /> {title}
        </CardTitle>
        <CardDescription>{description}</CardDescription>
        <CardAction>
          <StatusBadge status={st} />
        </CardAction>
      </CardHeader>
      <CardContent className="grid gap-4">
        {loggedIn && (
          <div className="rounded-lg border bg-muted/30 px-3 py-2 text-sm">
            <span className="font-medium">{st?.account ?? "Angemeldet"}</span>
            {st?.message && <span className="text-muted-foreground"> · {st.message}</span>}
          </div>
        )}
        {children}
      </CardContent>
      {loggedIn && (
        <CardFooter className="flex flex-wrap gap-2 border-t">
          {active ? (
            <Button variant="outline" onClick={() => run(() => api.disconnect(platform))}>
              <PlugIcon /> Trennen
            </Button>
          ) : (
            <Button onClick={() => run(() => api.connect(platform))}>
              <PlugZapIcon /> Verbinden
            </Button>
          )}
          <Button variant="ghost" onClick={() => run(() => api.logout(platform))}>
            <LogOutIcon /> Abmelden
          </Button>
        </CardFooter>
      )}
    </Card>
  );
}

// ---------------------------------------------------------------------------

function TwitchCard() {
  const { settings } = useAppData();
  const [device, setDevice] = useState<DeviceAuth | null>(null);
  const [busy, setBusy] = useState(false);
  const loggedIn = !!settings?.twitch.authorized;

  useEffect(() => {
    if (loggedIn) setDevice(null);
  }, [loggedIn, settings]);

  const login = async () => {
    setBusy(true);
    await run(async () => setDevice(await api.twitchDeviceAuth()));
    setBusy(false);
  };

  return (
    <PlatformCard platform="twitch" title="Twitch" description="Chat, Follows, Abos, Bits, Raids, Titel & Kategorie" loggedIn={loggedIn}>
      {settings && !settings.twitch.credentials_ok ? (
        <MissingCredentials />
      ) : (
        !loggedIn && (
          <Button size="lg" className="w-fit" disabled={busy} onClick={login}>
            {busy ? <Loader2Icon className="animate-spin" /> : <LogInIcon />} Mit Twitch anmelden
          </Button>
        )
      )}
      <Dialog open={!!device} onOpenChange={(o) => !o && setDevice(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Bei Twitch anmelden</DialogTitle>
            <DialogDescription>
              In deinem Browser hat sich Twitch geöffnet. Melde dich an und klicke auf <b>Autorisieren</b>. Danach geht es hier automatisch weiter.
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-2 rounded-lg border bg-muted/40 p-4 text-center">
            <span className="text-xs text-muted-foreground">Falls Twitch nach einem Code fragt:</span>
            <span className="font-mono text-2xl font-bold tracking-[0.3em]">{device?.user_code}</span>
          </div>
          <div className="flex items-center justify-between gap-2 text-sm text-muted-foreground">
            <span className="flex items-center gap-2">
              <Loader2Icon className="size-4 animate-spin" /> Warte auf Bestätigung …
            </span>
            <Button variant="outline" onClick={() => device && openUrl(device.verification_uri)}>
              Browser erneut öffnen
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </PlatformCard>
  );
}

// ---------------------------------------------------------------------------

function YoutubeCard() {
  const { settings } = useAppData();
  const [authUrl, setAuthUrl] = useState<string | null>(null);
  const loggedIn = !!settings?.youtube.authorized;

  useEffect(() => {
    if (loggedIn) setAuthUrl(null);
  }, [loggedIn, settings]);

  return (
    <PlatformCard
      platform="youtube"
      title="YouTube"
      description="Live-Chat, Abonnenten, Mitglieder, Super Chats, Likes, Titel & Beschreibung"
      loggedIn={loggedIn}
    >
      {settings && !settings.youtube.credentials_ok ? (
        <MissingCredentials />
      ) : (
        !loggedIn && (
          <div className="flex flex-wrap items-center gap-3">
            <Button size="lg" onClick={() => run(async () => setAuthUrl(await api.youtubeAuth()))}>
              <LogInIcon /> Mit Google anmelden
            </Button>
            {authUrl && (
              <span className="flex items-center gap-2 text-sm text-muted-foreground">
                <Loader2Icon className="size-4 animate-spin" /> Melde dich im Browser an …
                <button className="underline underline-offset-4" onClick={() => openUrl(authUrl)}>
                  erneut öffnen
                </button>
              </span>
            )}
          </div>
        )
      )}
    </PlatformCard>
  );
}

// ---------------------------------------------------------------------------

function TiktokCard() {
  const { settings, statuses } = useAppData();
  const [manual, setManual] = useState(false);
  const [username, setUsername] = useState("");
  const [importOpen, setImportOpen] = useState(false);
  const [sessionId, setSessionId] = useState("");
  const [importing, setImporting] = useState(false);

  const importSession = async (e: React.FormEvent) => {
    e.preventDefault();
    setImporting(true);
    try {
      const user = await api.tiktokImportSession(sessionId);
      toast.success(`Bei TikTok angemeldet als @${user}`);
      setSessionId("");
      setImportOpen(false);
    } catch (err) {
      toast.error(errorText(err));
    } finally {
      setImporting(false);
    }
  };
  const loggedIn = !!settings?.tiktok.authorized;
  const hasUser = !!settings?.tiktok.username;

  return (
    <PlatformCard
      platform="tiktok"
      title="TikTok LIVE"
      description="Chat lesen & schreiben, Moderation, Follows, Geschenke mit Coins, Likes, Shares – direkt verbunden, ohne Zusatzdienst"
      loggedIn={hasUser}
    >
      {!loggedIn && (
        <div className="grid gap-3">
          <div className="flex flex-wrap items-center gap-2">
            <Button size="lg" onClick={() => run(api.tiktokLogin)}>
              <LogInIcon /> Mit TikTok anmelden
            </Button>
            <Button variant="ghost" onClick={() => setImportOpen((o) => !o)}>
              Login klappt nicht?
            </Button>
            {!hasUser && (
              <Button variant="ghost" onClick={() => setManual((m) => !m)}>
                Nur Benutzername eingeben
              </Button>
            )}
          </div>
          {importOpen && (
            <form onSubmit={importSession} className="grid gap-3 rounded-lg border bg-muted/30 p-3">
              <div className="text-sm font-medium">Anmeldung aus deinem Browser übernehmen</div>
              <ol className="list-decimal space-y-1 pl-5 text-sm text-muted-foreground">
                <li>
                  Melde dich in deinem normalen Browser (Chrome, Firefox, Edge …) auf{" "}
                  <button type="button" className="underline underline-offset-4" onClick={() => openUrl("https://www.tiktok.com/login")}>
                    tiktok.com
                  </button>{" "}
                  an.
                </li>
                <li>
                  Drücke <kbd className="rounded border px-1 text-xs">F12</kbd> → Reiter <b>Anwendung</b> (Chrome/Edge) bzw. <b>Web-Speicher</b>{" "}
                  (Firefox) → <b>Cookies</b> → <code>https://www.tiktok.com</code>.
                </li>
                <li>
                  Suche den Eintrag <code>sessionid</code> und kopiere seinen <b>Wert</b> hier hinein.
                </li>
              </ol>
              <div className="flex gap-2">
                <Input
                  type="password"
                  value={sessionId}
                  onChange={(e) => setSessionId(e.target.value)}
                  placeholder="sessionid-Wert"
                  autoComplete="off"
                />
                <Button type="submit" disabled={importing || sessionId.trim().length < 16}>
                  {importing && <Loader2Icon className="animate-spin" />} Übernehmen
                </Button>
              </div>
              <p className="text-xs text-muted-foreground">
                Der Wert bleibt lokal auf diesem Rechner. Er ist wie ein Passwort – gib ihn niemandem weiter.
              </p>
            </form>
          )}
          <p className="text-sm text-muted-foreground">
            Es öffnet sich ein TikTok-Fenster. Am zuverlässigsten ist die Anmeldung per <b>QR-Code</b> mit der TikTok-App. Danach erkennt StreamSuite
            deinen Kanal automatisch. Mit Anmeldung kannst du auch in den TikTok-Chat schreiben, Zuschauer moderieren und Kommentar-Einstellungen
            ändern.
          </p>
          {manual && !hasUser && (
            <form
              className="flex gap-2"
              onSubmit={(e) => {
                e.preventDefault();
                void run(() => api.tiktokSetUsername(username));
              }}
            >
              <Input value={username} onChange={(e) => setUsername(e.target.value)} placeholder="@deinname (nur lesen, ohne Senden & Moderation)" />
              <Button type="submit" disabled={!username.trim()}>
                Übernehmen
              </Button>
            </form>
          )}
        </div>
      )}
      {loggedIn && (
        <div className="flex flex-wrap items-center gap-2">
          <Button variant="outline" onClick={() => run(api.tiktokShowWindow)} disabled={!statuses.tiktok?.live}>
            <AppWindowIcon /> TikTok-LIVE-Fenster anzeigen
          </Button>
          <span className="text-xs text-muted-foreground">Falls TikTok eine Sicherheitsabfrage zeigt oder das Senden nicht klappt.</span>
        </div>
      )}
    </PlatformCard>
  );
}

export function ConnectionsPage() {
  return (
    <>
      <PageHeader title="Verbindungen" description="Einfach anmelden. Alle Anmeldedaten bleiben lokal auf diesem Rechner." />
      <div className="grid gap-4 p-6 xl:grid-cols-3">
        <TwitchCard />
        <YoutubeCard />
        <TiktokCard />
      </div>
    </>
  );
}
