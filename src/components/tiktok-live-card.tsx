import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import { Loader2Icon, PlusIcon, RefreshCwIcon, ShieldMinusIcon, XIcon } from "lucide-react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardAction, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import { Switch } from "@/components/ui/switch";
import { PlatformIcon } from "@/components/common";
import { api, errorText, type TiktokBlockedWord, type TiktokModerator, type TiktokRoomInfo } from "@/lib/api";

const n = (v?: number) => (v ?? 0).toLocaleString("de-DE");
const clock = (secs?: number) => (secs ? new Date(secs * 1000).toLocaleTimeString("de-DE", { hour: "2-digit", minute: "2-digit" }) : "–");

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid gap-0.5">
      <span className="text-xs text-muted-foreground">{label}</span>
      <span className="text-sm font-medium tabular-nums">{value}</span>
    </div>
  );
}

/** Steuerung des eigenen TikTok-LIVE: Raum-Infos, Kommentare, Wortfilter, Moderatoren. */
export function TiktokLiveCard({ info, error, onRefresh }: { info?: TiktokRoomInfo; error?: string; onRefresh: () => void }) {
  const [comments, setComments] = useState<boolean | undefined>(info?.comments_enabled);
  const [filter, setFilter] = useState<boolean | undefined>(info?.comment_filter);
  const [busy, setBusy] = useState<string | null>(null);
  const [words, setWords] = useState<TiktokBlockedWord[] | null>(null);
  const [word, setWord] = useState("");
  const [mods, setMods] = useState<TiktokModerator[] | null>(null);
  const [listError, setListError] = useState<string | null>(null);

  const live = !!info?.live;
  const canControl = live && !!info?.logged_in;

  useEffect(() => {
    setComments(info?.comments_enabled);
    setFilter(info?.comment_filter);
  }, [info?.comments_enabled, info?.comment_filter]);

  const run = useCallback(async (key: string, fn: () => Promise<void>) => {
    setBusy(key);
    try {
      await fn();
    } catch (e) {
      toast.error(errorText(e));
    } finally {
      setBusy(null);
    }
  }, []);

  const loadWords = useCallback(() => run("words", async () => setWords(await api.tiktokBlockedWords())), [run]);
  const loadMods = useCallback(() => run("mods", async () => setMods(await api.tiktokModerators())), [run]);

  // Listen automatisch laden, sobald Steuerung möglich ist (Fehler einmalig in der Karte statt als Toasts)
  useEffect(() => {
    setWords(null);
    setMods(null);
    setListError(null);
    if (!canControl) return;
    let cancelled = false;
    (async () => {
      try {
        const w = await api.tiktokBlockedWords();
        const m = await api.tiktokModerators();
        if (!cancelled) {
          setWords(w);
          setMods(m);
        }
      } catch (e) {
        if (!cancelled) setListError(errorText(e));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [canControl, info?.room_id]);

  const toggle = (setting: "comments" | "comment_filter", value: boolean) =>
    run(setting, async () => {
      await api.tiktokSetRoomSwitch(setting, value);
      if (setting === "comments") setComments(value);
      else setFilter(value);
      toast.success(
        setting === "comments"
          ? value
            ? "Kommentare erlaubt"
            : "Kommentare gesperrt"
          : value
            ? "Kommentarfilter an"
            : "Kommentarfilter aus",
      );
    });

  const addWord = (e: React.FormEvent) => {
    e.preventDefault();
    const w = word.trim();
    if (!w) return;
    void run("add-word", async () => {
      const added = await api.tiktokAddBlockedWord(w);
      setWords((list) => [...(list ?? []), added]);
      setWord("");
    });
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <PlatformIcon platform="tiktok" className="size-5" /> TikTok LIVE
          {live && <Badge className="h-4 bg-red-600 px-1 text-[10px] text-white">Live</Badge>}
        </CardTitle>
        <CardDescription>Steuerung deines eigenen LIVE über deine TikTok-Anmeldung – direkt, ohne Zusatzdienst.</CardDescription>
        <CardAction>
          <Button variant="ghost" size="icon-sm" onClick={onRefresh} title="Neu laden">
            <RefreshCwIcon />
          </Button>
        </CardAction>
      </CardHeader>
      <CardContent className="grid gap-5">
        {error && (
          <Alert variant="destructive">
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        )}
        {!live ? (
          <p className="text-sm text-muted-foreground">
            Gerade nicht live. Sobald dein TikTok-LIVE läuft, erscheinen hier Titel, Thema, Zahlen und die Steuerung.
          </p>
        ) : (
          <>
            <div className="grid gap-3 sm:grid-cols-[1fr_auto]">
              <div className="grid gap-0.5">
                <span className="text-xs text-muted-foreground">Titel</span>
                <span className="text-sm font-medium">{info?.title || "–"}</span>
                {info?.topic && <span className="text-xs text-muted-foreground">Thema: {info.topic}</span>}
              </div>
              <div className="grid grid-cols-4 gap-4 sm:grid-cols-4">
                <Stat label="Zuschauer" value={n(info?.viewers)} />
                <Stat label="Likes" value={n(info?.likes)} />
                <Stat label="Gesamt" value={n(info?.total_viewers)} />
                <Stat label="Start" value={clock(info?.started_at)} />
              </div>
            </div>
            <p className="text-xs text-muted-foreground">
              Titel und Thema legt TikTok beim Start des LIVE fest. Für einen laufenden LIVE gibt es keine frei nutzbare Schnittstelle, um
              sie zu ändern.
            </p>

            {!info?.logged_in && (
              <Alert>
                <AlertDescription>Für Kommentare, Wortfilter und Moderation bitte unter Verbindungen bei TikTok anmelden.</AlertDescription>
              </Alert>
            )}

            {listError && (
              <Alert variant="destructive">
                <AlertDescription>{listError}</AlertDescription>
              </Alert>
            )}

            <Separator />

            <div className="grid gap-3 sm:grid-cols-2">
              <label className="flex items-center justify-between gap-3">
                <span className="grid gap-0.5">
                  <span className="text-sm font-medium">Kommentare erlauben</span>
                  <span className="text-xs text-muted-foreground">Chat für alle Zuschauer an/aus</span>
                </span>
                {busy === "comments" ? (
                  <Loader2Icon className="size-4 animate-spin" />
                ) : (
                  <Switch checked={!!comments} disabled={!canControl || !!busy} onCheckedChange={(v) => void toggle("comments", v)} />
                )}
              </label>
              <label className="flex items-center justify-between gap-3">
                <span className="grid gap-0.5">
                  <span className="text-sm font-medium">Kommentarfilter</span>
                  <span className="text-xs text-muted-foreground">Möglicherweise unangemessene Kommentare ausblenden</span>
                </span>
                {busy === "comment_filter" ? (
                  <Loader2Icon className="size-4 animate-spin" />
                ) : (
                  <Switch checked={!!filter} disabled={!canControl || !!busy} onCheckedChange={(v) => void toggle("comment_filter", v)} />
                )}
              </label>
            </div>

            <Separator />

            <div className="grid gap-2">
              <div className="flex items-center justify-between">
                <Label>Gesperrte Wörter</Label>
                <Button variant="ghost" size="xs" onClick={() => void loadWords()} disabled={!canControl || busy === "words"}>
                  {busy === "words" ? <Loader2Icon className="animate-spin" /> : <RefreshCwIcon />} Laden
                </Button>
              </div>
              <form onSubmit={addWord} className="flex gap-2">
                <Input
                  value={word}
                  maxLength={30}
                  onChange={(e) => setWord(e.target.value)}
                  placeholder="Wort, das im Chat ausgeblendet wird"
                  disabled={!canControl}
                />
                <Button type="submit" variant="outline" disabled={!canControl || !word.trim() || busy === "add-word"}>
                  {busy === "add-word" ? <Loader2Icon className="animate-spin" /> : <PlusIcon />} Sperren
                </Button>
              </form>
              <div className="flex flex-wrap gap-1.5">
                {words?.length === 0 && <span className="text-xs text-muted-foreground">Keine gesperrten Wörter.</span>}
                {words?.map((w) => (
                  <Badge key={w.id} variant="secondary" className="gap-1 pr-0.5">
                    {w.word}
                    <button
                      type="button"
                      className="rounded-sm p-0.5 hover:bg-muted-foreground/20"
                      title="Freigeben"
                      disabled={!!busy}
                      onClick={() =>
                        void run(`del-${w.id}`, async () => {
                          await api.tiktokDeleteBlockedWord(w.id);
                          setWords((list) => (list ?? []).filter((x) => x.id !== w.id));
                        })
                      }
                    >
                      {busy === `del-${w.id}` ? <Loader2Icon className="size-3 animate-spin" /> : <XIcon className="size-3" />}
                    </button>
                  </Badge>
                ))}
              </div>
            </div>

            <Separator />

            <div className="grid gap-2">
              <div className="flex items-center justify-between">
                <Label>Moderatoren</Label>
                <Button variant="ghost" size="xs" onClick={() => void loadMods()} disabled={!canControl || busy === "mods"}>
                  {busy === "mods" ? <Loader2Icon className="animate-spin" /> : <RefreshCwIcon />} Laden
                </Button>
              </div>
              <p className="text-xs text-muted-foreground">
                Neue Moderatoren ernennst du im Multichat über das Menü (⋯) an einer TikTok-Nachricht. Dort kannst du Zuschauer auch
                stummschalten oder aus dem LIVE entfernen.
              </p>
              <div className="grid gap-1">
                {mods?.length === 0 && <span className="text-xs text-muted-foreground">Keine Moderatoren.</span>}
                {mods?.map((m) => (
                  <div key={m.id} className="flex items-center gap-2 rounded-md px-1 py-0.5 hover:bg-muted/50">
                    {m.avatar ? (
                      <img src={m.avatar} alt="" className="size-6 rounded-full" />
                    ) : (
                      <div className="size-6 rounded-full bg-muted" />
                    )}
                    <span className="text-sm">{m.display_name}</span>
                    <span className="text-xs text-muted-foreground">@{m.name}</span>
                    <Button
                      variant="ghost"
                      size="icon-xs"
                      className="ml-auto"
                      title="Moderator entfernen"
                      disabled={!!busy}
                      onClick={() =>
                        void run(`mod-${m.id}`, async () => {
                          await api.tiktokSetModerator(m.id, false);
                          setMods((list) => (list ?? []).filter((x) => x.id !== m.id));
                          toast.success(`${m.display_name} ist kein Moderator mehr`);
                        })
                      }
                    >
                      {busy === `mod-${m.id}` ? <Loader2Icon className="animate-spin" /> : <ShieldMinusIcon />}
                    </Button>
                  </div>
                ))}
              </div>
            </div>
          </>
        )}
      </CardContent>
    </Card>
  );
}
