import { useCallback, useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { InfoIcon, Loader2Icon, RefreshCwIcon, SearchIcon, XIcon } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import { PageHeader, PlatformIcon } from "@/components/common";
import { api, errorText, PLATFORM_META, type Category, type Platform, type StreamInfoResponse } from "@/lib/api";
import { useAppData } from "@/lib/store";

function TwitchCategoryPicker({ value, onChange }: { value: Category | null; onChange: (c: Category | null) => void }) {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<Category[]>([]);
  const [open, setOpen] = useState(false);
  const [loading, setLoading] = useState(false);
  const timer = useRef<number>(undefined);

  useEffect(() => {
    window.clearTimeout(timer.current);
    if (!query.trim()) {
      setResults([]);
      return;
    }
    timer.current = window.setTimeout(async () => {
      setLoading(true);
      try {
        setResults(await api.searchTwitchCategories(query));
        setOpen(true);
      } catch (e) {
        toast.error(errorText(e));
      } finally {
        setLoading(false);
      }
    }, 300);
  }, [query]);

  if (value) {
    return (
      <div className="flex h-8 items-center justify-between rounded-lg border px-2.5 text-sm">
        <span>{value.name}</span>
        <Button variant="ghost" size="icon-xs" onClick={() => onChange(null)} title="Andere Kategorie wählen">
          <XIcon />
        </Button>
      </div>
    );
  }

  return (
    <div className="relative">
      <SearchIcon className="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
      <Input
        className="pl-8"
        placeholder="Kategorie suchen, z. B. Just Chatting, Minecraft …"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        onFocus={() => results.length && setOpen(true)}
        onBlur={() => setTimeout(() => setOpen(false), 150)}
      />
      {loading && <Loader2Icon className="absolute right-2.5 top-1/2 size-4 -translate-y-1/2 animate-spin text-muted-foreground" />}
      {open && results.length > 0 && (
        <div className="absolute z-50 mt-1 max-h-72 w-full overflow-y-auto rounded-lg border bg-popover p-1 shadow-md">
          {results.map((c) => (
            <button
              key={c.id}
              type="button"
              className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-muted"
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => {
                onChange(c);
                setQuery("");
                setOpen(false);
              }}
            >
              {c.image && <img src={c.image} alt="" className="h-9 w-[26px] rounded-sm object-cover" />}
              {c.name}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

export function StreamInfoPage() {
  const { statuses } = useAppData();
  const [info, setInfo] = useState<StreamInfoResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [twitchCat, setTwitchCat] = useState<Category | null>(null);
  const [ytCats, setYtCats] = useState<Category[]>([]);
  const [ytCat, setYtCat] = useState("");
  const [targets, setTargets] = useState<Record<"twitch" | "youtube", boolean>>({ twitch: true, youtube: true });

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const res = await api.getStreamInfo();
      setInfo(res);
      const tw = res.twitch.info;
      const yt = res.youtube.info;
      setTitle(tw?.title || yt?.title || "");
      setDescription(yt?.description ?? "");
      setTwitchCat(tw?.category_id ? { id: tw.category_id, name: tw.category_name } : null);
      setYtCat(yt?.category_id ?? "");
      setTargets({ twitch: !!res.twitch.connected, youtube: !!res.youtube.connected && !!yt });
      if (res.youtube.connected) {
        api.youtubeCategories().then(setYtCats).catch(() => {});
      }
    } catch (e) {
      toast.error(errorText(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const save = async () => {
    const platforms = (Object.keys(targets) as ("twitch" | "youtube")[]).filter((p) => targets[p]);
    if (!platforms.length) return toast.error("Keine Plattform ausgewählt");
    setSaving(true);
    try {
      const res = await api.updateStreamInfo({
        platforms,
        title,
        description,
        twitch_category_id: twitchCat?.id,
        youtube_category_id: ytCat,
      });
      for (const [p, r] of Object.entries(res)) {
        const name = PLATFORM_META[p as Platform].name;
        if (r.ok) toast.success(`${name} aktualisiert`);
        else toast.error(`${name}: ${r.error}`);
      }
    } catch (e) {
      toast.error(errorText(e));
    } finally {
      setSaving(false);
    }
  };

  const twConnected = !!info?.twitch.connected;
  const ytConnected = !!info?.youtube.connected;
  const ytTarget = info?.youtube.info;

  return (
    <>
      <PageHeader title="Stream-Info" description="Titel, Kategorie und Beschreibung auf allen Plattformen gleichzeitig ändern.">
        <Button variant="outline" onClick={() => void load()} disabled={loading}>
          <RefreshCwIcon className={loading ? "animate-spin" : ""} /> Aktuelle Werte laden
        </Button>
      </PageHeader>

      <div className="grid gap-4 p-6 lg:grid-cols-[1fr_300px]">
        <Card>
          <CardHeader>
            <CardTitle>Stream-Details</CardTitle>
            <CardDescription>Felder, die eine Plattform nicht unterstützt, werden dort ignoriert.</CardDescription>
          </CardHeader>
          <CardContent className="grid gap-5">
            <div className="grid gap-1.5">
              <div className="flex items-center justify-between">
                <Label>Titel</Label>
                <span className="text-xs text-muted-foreground tabular-nums">{title.length}/100</span>
              </div>
              <Input value={title} maxLength={140} onChange={(e) => setTitle(e.target.value)} placeholder="Worum geht es heute?" />
              {title.length > 100 && <p className="text-xs text-amber-500">YouTube erlaubt max. 100 Zeichen, Twitch max. 140.</p>}
            </div>

            <div className="grid gap-4 md:grid-cols-2">
              <div className="grid gap-1.5">
                <Label className="flex items-center gap-1.5">
                  <PlatformIcon platform="twitch" /> Twitch-Kategorie
                </Label>
                <TwitchCategoryPicker value={twitchCat} onChange={setTwitchCat} />
              </div>
              <div className="grid gap-1.5">
                <Label className="flex items-center gap-1.5">
                  <PlatformIcon platform="youtube" /> YouTube-Kategorie
                </Label>
                <Select value={ytCat} onValueChange={setYtCat} disabled={!ytCats.length}>
                  <SelectTrigger className="w-full">
                    <SelectValue placeholder={ytConnected ? "Kategorie wählen" : "YouTube nicht verbunden"} />
                  </SelectTrigger>
                  <SelectContent>
                    {ytCats.map((c) => (
                      <SelectItem key={c.id} value={c.id}>
                        {c.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            </div>

            <div className="grid gap-1.5">
              <Label className="flex items-center gap-1.5">
                <PlatformIcon platform="youtube" /> Beschreibung
              </Label>
              <Textarea
                rows={7}
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder="Beschreibung des Livestreams (nur YouTube – Twitch und TikTok haben kein Beschreibungsfeld in ihrer API)"
              />
            </div>

            <div className="flex justify-end">
              <Button onClick={save} disabled={saving || (!targets.twitch && !targets.youtube)}>
                {saving && <Loader2Icon className="animate-spin" />}
                Auf ausgewählten Plattformen übernehmen
              </Button>
            </div>
          </CardContent>
        </Card>

        <div className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle>Plattformen</CardTitle>
            </CardHeader>
            <CardContent className="grid gap-4">
              <label className="flex items-start justify-between gap-3">
                <span className="grid gap-0.5">
                  <span className="flex items-center gap-2 text-sm font-medium">
                    <PlatformIcon platform="twitch" /> Twitch
                  </span>
                  <span className="text-xs text-muted-foreground">
                    {twConnected ? (info?.twitch.error ?? "Titel & Kategorie") : "Nicht verbunden"}
                  </span>
                </span>
                <Switch
                  checked={targets.twitch && twConnected}
                  disabled={!twConnected}
                  onCheckedChange={(v) => setTargets((t) => ({ ...t, twitch: v }))}
                />
              </label>
              <label className="flex items-start justify-between gap-3">
                <span className="grid gap-0.5">
                  <span className="flex items-center gap-2 text-sm font-medium">
                    <PlatformIcon platform="youtube" /> YouTube
                    {ytTarget && (
                      <Badge variant="outline" className="h-4 px-1 text-[10px]">
                        {ytTarget.status === "live" ? "Live" : "Geplant"}
                      </Badge>
                    )}
                  </span>
                  <span className="text-xs text-muted-foreground">
                    {!ytConnected
                      ? "Nicht verbunden"
                      : info?.youtube.error
                        ? info.youtube.error
                        : ytTarget
                          ? "Titel, Beschreibung & Kategorie"
                          : "Kein aktiver oder geplanter Livestream"}
                  </span>
                </span>
                <Switch
                  checked={targets.youtube && !!ytTarget}
                  disabled={!ytTarget}
                  onCheckedChange={(v) => setTargets((t) => ({ ...t, youtube: v }))}
                />
              </label>
              <div className="flex items-start justify-between gap-3 opacity-60">
                <span className="grid gap-0.5">
                  <span className="flex items-center gap-2 text-sm font-medium">
                    <PlatformIcon platform="tiktok" /> TikTok
                  </span>
                  <span className="text-xs text-muted-foreground">
                    {statuses.tiktok?.live ? "Live – " : ""}Keine API zum Ändern des Titels
                  </span>
                </span>
                <Switch checked={false} disabled />
              </div>
            </CardContent>
          </Card>
          <Alert>
            <InfoIcon />
            <AlertTitle>Hinweis zu TikTok</AlertTitle>
            <AlertDescription>
              TikTok stellt keine offizielle Schnittstelle zum Ändern von LIVE-Titel oder -Thema bereit. Lege diese vor dem Start in der TikTok
              LIVE Studio App bzw. der App fest.
            </AlertDescription>
          </Alert>
        </div>
      </div>
    </>
  );
}
