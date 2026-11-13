import { useState, type ReactNode } from "react";
import { CheckIcon, CopyIcon, ExternalLinkIcon, HeartIcon } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { formatAlert, PLATFORM_META, type ConnStatus, type EventPlatform, type SSEvent } from "@/lib/api";

export function PlatformIcon({ platform, className }: { platform: EventPlatform; className?: string }) {
  const color = PLATFORM_META[platform].color;
  const cls = cn("size-4 shrink-0", className);
  switch (platform) {
    case "twitch":
      return (
        <svg viewBox="0 0 24 24" fill={color} className={cls} aria-label="Twitch">
          <path d="M4.3 3 3 6.4v13.3h4.6V22h2.6l2.3-2.3H16l5-5V3H4.3Zm15 10.8-2.9 2.9h-4.6l-2.4 2.3v-2.3H5.6V4.7h13.7v9.1ZM16.4 7.8v4.7h-1.7V7.8h1.7Zm-4.6 0v4.7h-1.7V7.8h1.7Z" />
        </svg>
      );
    case "youtube":
      return (
        <svg viewBox="0 0 24 24" fill={color} className={cls} aria-label="YouTube">
          <path d="M23 7.2a3 3 0 0 0-2.1-2.1C19 4.6 12 4.6 12 4.6s-7 0-8.9.5A3 3 0 0 0 1 7.2 31 31 0 0 0 .5 12 31 31 0 0 0 1 16.8a3 3 0 0 0 2.1 2.1c1.9.5 8.9.5 8.9.5s7 0 8.9-.5a3 3 0 0 0 2.1-2.1c.4-1.6.5-3.2.5-4.8s-.1-3.2-.5-4.8ZM9.7 15.1V8.9l5.8 3.1-5.8 3.1Z" />
        </svg>
      );
    case "tiktok":
      return (
        <svg viewBox="0 0 24 24" className={cls} aria-label="TikTok">
          <path
            fill="currentColor"
            d="M16.6 5.8A4.3 4.3 0 0 1 15.5 3h-3.2v12.7a2.6 2.6 0 1 1-2.6-2.6c.3 0 .5 0 .8.1V9.9a5.9 5.9 0 1 0 5 5.8V9.3a7.5 7.5 0 0 0 4.4 1.4V7.5a4.3 4.3 0 0 1-3.3-1.7Z"
          />
        </svg>
      );
    default:
      return <HeartIcon className={cn(cls, "text-pink-500")} />;
  }
}

const STATE_LABEL: Record<ConnStatus["state"], string> = {
  unconfigured: "Nicht eingerichtet",
  disconnected: "Getrennt",
  connecting: "Verbinde …",
  connected: "Verbunden",
  error: "Fehler",
};

export function StatusBadge({ status }: { status?: ConnStatus }) {
  const state = status?.state ?? "unconfigured";
  const dot = {
    unconfigured: "bg-muted-foreground/40",
    disconnected: "bg-muted-foreground",
    connecting: "bg-amber-500 animate-pulse",
    connected: "bg-emerald-500",
    error: "bg-red-500",
  }[state];
  return (
    <div className="flex items-center gap-1.5">
      <Badge variant="outline" className="gap-1.5">
        <span className={cn("size-2 rounded-full", dot)} />
        {STATE_LABEL[state]}
      </Badge>
      {status?.live && <Badge className="bg-red-600 text-white">LIVE</Badge>}
    </div>
  );
}

export function PageHeader({ title, description, children }: { title: string; description?: string; children?: ReactNode }) {
  return (
    <div className="flex flex-wrap items-end justify-between gap-3 border-b px-6 py-4">
      <div>
        <h1 className="text-lg font-semibold tracking-tight">{title}</h1>
        {description && <p className="text-sm text-muted-foreground">{description}</p>}
      </div>
      {children && <div className="flex flex-wrap items-center gap-2">{children}</div>}
    </div>
  );
}

export function CopyField({ value, className }: { value: string; className?: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <div className={cn("flex gap-1.5", className)}>
      <Input readOnly value={value} className="font-mono text-xs" onFocus={(e) => e.currentTarget.select()} />
      <Button
        variant="outline"
        size="icon"
        title="Kopieren"
        onClick={async () => {
          await navigator.clipboard.writeText(value);
          setCopied(true);
          setTimeout(() => setCopied(false), 1500);
        }}
      >
        {copied ? <CheckIcon /> : <CopyIcon />}
      </Button>
      <Button variant="outline" size="icon" title="Im Browser öffnen" onClick={() => void openUrl(value)}>
        <ExternalLinkIcon />
      </Button>
    </div>
  );
}

export function ExternalLink({ href, children }: { href: string; children: ReactNode }) {
  return (
    <button
      type="button"
      className="inline-flex items-center gap-1 font-medium text-foreground underline underline-offset-4 hover:opacity-80"
      onClick={() => void openUrl(href)}
    >
      {children}
      <ExternalLinkIcon className="size-3" />
    </button>
  );
}

const time = (ts: number) => new Date(ts).toLocaleTimeString("de-DE", { hour: "2-digit", minute: "2-digit" });

const BADGE_LABEL: Record<string, string> = {
  broadcaster: "Streamer",
  owner: "Streamer",
  moderator: "Mod",
  vip: "VIP",
  subscriber: "Sub",
  founder: "Founder",
  member: "Mitglied",
};

export function ChatLine({ ev, deleted, compact, actions }: { ev: SSEvent; deleted?: boolean; compact?: boolean; actions?: ReactNode }) {
  const u = ev.user;
  return (
    <div className={cn("group flex gap-2 rounded-md px-2 py-1 hover:bg-muted/50", deleted && "opacity-40 line-through")}>
      {!compact && <span className="mt-0.5 shrink-0 text-xs tabular-nums text-muted-foreground">{time(ev.timestamp)}</span>}
      <PlatformIcon platform={ev.platform} className="mt-0.5" />
      <div className="min-w-0 break-words text-sm leading-relaxed">
        {u?.badges
          .map((b) => BADGE_LABEL[b])
          .filter(Boolean)
          .map((b) => (
            <Badge key={b} variant="outline" className="mr-1 h-4 px-1 text-[10px]">
              {b}
            </Badge>
          ))}
        <span className="mr-1.5 font-semibold" style={{ color: u?.color }}>
          {u?.display_name}
        </span>
        {ev.parts && ev.parts.length > 0
          ? ev.parts.map((p, i) =>
              p.type === "emote" && p.url ? (
                <img key={i} src={p.url} alt={p.text} title={p.text} className="-my-1 inline h-6 align-middle" />
              ) : (
                <span key={i}>{p.text}</span>
              ),
            )
          : ev.message}
      </div>
      {actions && <div className="ml-auto shrink-0 self-start">{actions}</div>}
    </div>
  );
}

export function AlertLine({ ev }: { ev: SSEvent }) {
  const a = formatAlert(ev);
  return (
    <div
      className="flex items-start gap-3 rounded-lg border-l-2 bg-muted/40 px-3 py-2"
      style={{ borderLeftColor: PLATFORM_META[ev.platform].color }}
    >
      {ev.gift_image ? (
        <img src={ev.gift_image} className="size-8 shrink-0 object-contain" alt="" />
      ) : (
        <PlatformIcon platform={ev.platform} className="mt-0.5 size-5" />
      )}
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">{a.title}</span>
          {ev.test && (
            <Badge variant="outline" className="h-4 px-1 text-[10px]">
              Test
            </Badge>
          )}
          <span className="ml-auto text-xs tabular-nums text-muted-foreground">{time(ev.timestamp)}</span>
        </div>
        <div className="text-sm font-medium">{a.text}</div>
        {a.message && <div className="text-sm text-muted-foreground">{a.message}</div>}
      </div>
    </div>
  );
}

export function EmptyState({ icon, title, children }: { icon: ReactNode; title: string; children?: ReactNode }) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-2 p-8 text-center text-muted-foreground">
      <div className="text-muted-foreground/60 [&_svg]:size-8">{icon}</div>
      <div className="font-medium text-foreground">{title}</div>
      {children && <div className="max-w-sm text-sm">{children}</div>}
    </div>
  );
}
