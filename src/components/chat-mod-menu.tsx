import { toast } from "sonner";
import {
  BanIcon,
  ClockIcon,
  CrownIcon,
  MicOffIcon,
  MoreHorizontalIcon,
  ShieldCheckIcon,
  ShieldMinusIcon,
  Trash2Icon,
  UndoIcon,
  UserMinusIcon,
  Volume2Icon,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { api, errorText, PLATFORM_META, type ModAction, type SSEvent } from "@/lib/api";

type Duration = { label: string; seconds: number };

const TIMEOUTS: Record<"twitch" | "youtube" | "tiktok", Duration[]> = {
  twitch: [
    { label: "1 Minute", seconds: 60 },
    { label: "10 Minuten", seconds: 600 },
    { label: "1 Stunde", seconds: 3600 },
    { label: "1 Tag", seconds: 86400 },
    { label: "1 Woche", seconds: 604800 },
  ],
  youtube: [
    { label: "10 Sekunden", seconds: 10 },
    { label: "1 Minute", seconds: 60 },
    { label: "5 Minuten", seconds: 300 },
    { label: "30 Minuten", seconds: 1800 },
    { label: "1 Tag", seconds: 86400 },
  ],
  tiktok: [
    { label: "5 Sekunden", seconds: 5 },
    { label: "30 Sekunden", seconds: 30 },
    { label: "1 Minute", seconds: 60 },
    { label: "5 Minuten", seconds: 300 },
    { label: "Für den ganzen LIVE", seconds: 0 },
  ],
};

/** Moderationsmenü (drei Punkte beim Hovern) für eine Chatnachricht auf Twitch, YouTube oder TikTok. */
export function ChatModMenu({ ev }: { ev: SSEvent }) {
  const user = ev.user;
  if (!user || ev.platform === "system") return null;
  const platform = ev.platform;
  const pname = PLATFORM_META[platform].name;
  const name = user.display_name || user.name;
  const messageId = (ev.data?.message_id as string) || undefined;
  const isVip = user.badges.includes("vip");

  const act = async (action: ModAction, success: string, duration?: number) => {
    const id = toast.loading(`Sende an ${pname} …`);
    try {
      await api.moderate({ platform, action, user_id: user.id, message_id: messageId, duration });
      toast.success(success, { id });
    } catch (e) {
      toast.error(errorText(e), { id });
    }
  };

  const tiktok = platform === "tiktok";
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          variant="ghost"
          size="icon-xs"
          className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100 data-[state=open]:opacity-100"
          title={`${pname}-Moderation`}
        >
          <MoreHorizontalIcon />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-60">
        <DropdownMenuLabel className="truncate">
          {pname} · {platform === "youtube" || !user.name ? name : `@${user.name}`}
        </DropdownMenuLabel>
        {!tiktok && messageId && (
          <DropdownMenuItem onSelect={() => void act("delete", "Nachricht gelöscht")}>
            <Trash2Icon /> Nachricht löschen
          </DropdownMenuItem>
        )}
        <DropdownMenuSub>
          <DropdownMenuSubTrigger>
            {tiktok ? <MicOffIcon /> : <ClockIcon />} {tiktok ? "Stummschalten" : "Timeout"}
          </DropdownMenuSubTrigger>
          <DropdownMenuSubContent>
            {TIMEOUTS[platform].map((o) => (
              <DropdownMenuItem
                key={o.seconds}
                onSelect={() => void act("timeout", `${name}: ${tiktok ? "stummgeschaltet" : "Timeout"} (${o.label})`, o.seconds)}
              >
                {o.label}
              </DropdownMenuItem>
            ))}
          </DropdownMenuSubContent>
        </DropdownMenuSub>
        {tiktok && (
          <DropdownMenuItem onSelect={() => void act("untimeout", `Stummschaltung für ${name} aufgehoben`)}>
            <Volume2Icon /> Stummschaltung aufheben
          </DropdownMenuItem>
        )}
        <DropdownMenuSeparator />
        <DropdownMenuItem variant="destructive" onSelect={() => void act("ban", tiktok ? `${name} aus dem LIVE entfernt` : `${name} gebannt`)}>
          <BanIcon /> {tiktok ? "Aus dem LIVE entfernen" : "Bannen"}
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={() => void act("unban", tiktok ? `${name} wieder zugelassen` : `${name} entbannt`)}>
          <UndoIcon /> {tiktok ? "Wieder zulassen" : "Entbannen"}
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        {user.is_mod ? (
          <DropdownMenuItem onSelect={() => void act("unmod", `${name} ist kein Moderator mehr`)}>
            <ShieldMinusIcon /> Moderator entfernen
          </DropdownMenuItem>
        ) : (
          <DropdownMenuItem onSelect={() => void act("mod", `${name} ist jetzt Moderator`)}>
            <ShieldCheckIcon /> Zum Moderator machen
          </DropdownMenuItem>
        )}
        {platform === "twitch" &&
          (isVip ? (
            <DropdownMenuItem onSelect={() => void act("unvip", `${name} ist kein VIP mehr`)}>
              <UserMinusIcon /> VIP entfernen
            </DropdownMenuItem>
          ) : (
            <DropdownMenuItem onSelect={() => void act("vip", `${name} ist jetzt VIP`)}>
              <CrownIcon /> Zum VIP machen
            </DropdownMenuItem>
          ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
