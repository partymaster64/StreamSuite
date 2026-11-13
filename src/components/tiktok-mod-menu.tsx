import { toast } from "sonner";
import { BanIcon, MicOffIcon, MoreHorizontalIcon, ShieldCheckIcon, ShieldMinusIcon, UndoIcon, Volume2Icon } from "lucide-react";
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
import { api, errorText, type SSUser } from "@/lib/api";

const MUTE_OPTIONS: { label: string; seconds: number }[] = [
  { label: "5 Sekunden", seconds: 5 },
  { label: "30 Sekunden", seconds: 30 },
  { label: "1 Minute", seconds: 60 },
  { label: "5 Minuten", seconds: 300 },
  { label: "Für den ganzen LIVE", seconds: 0 },
];

async function act(success: string, fn: () => Promise<unknown>) {
  const id = toast.loading("Sende an TikTok …");
  try {
    await fn();
    toast.success(success, { id });
  } catch (e) {
    toast.error(errorText(e), { id });
  }
}

/** Moderationsmenü für eine TikTok-Chatnachricht (eigener LIVE, eingeloggt). */
export function TiktokModMenu({ user }: { user: SSUser }) {
  const name = user.display_name || user.name;
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          variant="ghost"
          size="icon-xs"
          className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100 data-[state=open]:opacity-100"
          title="TikTok-Moderation"
        >
          <MoreHorizontalIcon />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-56">
        <DropdownMenuLabel className="truncate">{user.name ? `@${user.name}` : name}</DropdownMenuLabel>
        <DropdownMenuSub>
          <DropdownMenuSubTrigger>
            <MicOffIcon /> Stummschalten
          </DropdownMenuSubTrigger>
          <DropdownMenuSubContent>
            {MUTE_OPTIONS.map((o) => (
              <DropdownMenuItem
                key={o.seconds}
                onSelect={() => void act(`${name} stummgeschaltet`, () => api.tiktokModerate(user.id, "mute", o.seconds))}
              >
                {o.label}
              </DropdownMenuItem>
            ))}
          </DropdownMenuSubContent>
        </DropdownMenuSub>
        <DropdownMenuItem onSelect={() => void act(`Stummschaltung für ${name} aufgehoben`, () => api.tiktokModerate(user.id, "unmute"))}>
          <Volume2Icon /> Stummschaltung aufheben
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        <DropdownMenuItem
          variant="destructive"
          onSelect={() => void act(`${name} aus dem LIVE entfernt`, () => api.tiktokModerate(user.id, "block"))}
        >
          <BanIcon /> Aus dem LIVE entfernen
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={() => void act(`${name} wieder zugelassen`, () => api.tiktokModerate(user.id, "unblock"))}>
          <UndoIcon /> Wieder zulassen
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        {user.is_mod ? (
          <DropdownMenuItem onSelect={() => void act(`${name} ist kein Moderator mehr`, () => api.tiktokSetModerator(user.id, false))}>
            <ShieldMinusIcon /> Moderator entfernen
          </DropdownMenuItem>
        ) : (
          <DropdownMenuItem onSelect={() => void act(`${name} ist jetzt Moderator`, () => api.tiktokSetModerator(user.id, true))}>
            <ShieldCheckIcon /> Zum Moderator machen
          </DropdownMenuItem>
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
