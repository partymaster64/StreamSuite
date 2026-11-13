import { invoke } from "@tauri-apps/api/core";

export type Platform = "twitch" | "youtube" | "tiktok";
export type EventPlatform = Platform | "system";

export interface SSUser {
  id: string;
  name: string;
  display_name: string;
  avatar?: string;
  color?: string;
  badges: string[];
  is_mod: boolean;
  is_subscriber: boolean;
  is_owner: boolean;
}

export interface SSPart {
  type: "text" | "emote";
  text: string;
  url?: string;
}

export interface SSEvent {
  id: string;
  platform: EventPlatform;
  type: string;
  category: "chat" | "alert" | "system";
  timestamp: number;
  user?: SSUser;
  message?: string;
  parts?: SSPart[];
  amount?: number;
  currency?: string;
  amount_display?: string;
  coins?: number;
  gift_name?: string;
  gift_image?: string;
  tier?: string;
  data?: Record<string, unknown>;
  test?: boolean;
}

export interface ConnStatus {
  platform: Platform;
  state: "unconfigured" | "disconnected" | "connecting" | "connected" | "error";
  live: boolean;
  message: string;
  account: string | null;
}

export interface PlatformStats {
  viewers: number | null;
  total_likes: number | null;
  session_likes: number;
  session_follows: number;
  session_subs: number;
  session_coins: number;
  session_bits: number;
  session_messages: number;
}

export interface Stats {
  viewers: number;
  likes: number;
  platforms: Partial<Record<Platform, PlatformStats>>;
}

export interface Settings {
  server_port: number;
  close_to_tray: boolean;
  embedded: Partial<Record<Platform, boolean>>;
  twitch: { enabled: boolean; client_id: string; authorized: boolean; credentials_ok: boolean; login?: string; display_name?: string };
  youtube: {
    enabled: boolean;
    client_id: string;
    client_secret: string;
    authorized: boolean;
    credentials_ok: boolean;
    channel_title?: string;
    min_chat_poll_ms: number;
    subscriber_poll_secs: number;
  };
  tiktok: { enabled: boolean; username: string; authorized: boolean; credentials_ok: boolean };
  alerts: { like_interval_secs: number; like_min_count: number; tiktok_min_coins: number };
}

export interface ServerInfo {
  port: number | null;
  base_url: string | null;
  overlays_dir: string;
  custom: string[];
  tray: boolean;
}

export interface DeviceAuth {
  user_code: string;
  verification_uri: string;
  expires_in: number;
}

export type ResultMap = Partial<Record<Platform, { ok: boolean; error?: string }>>;

export interface TiktokRoomInfo {
  live: boolean;
  room_id?: string;
  title?: string;
  topic?: string | null;
  viewers?: number;
  likes?: number;
  total_viewers?: number;
  started_at?: number;
  share_url?: string | null;
  cover?: string | null;
  comments_enabled?: boolean;
  comment_filter?: boolean;
  logged_in?: boolean;
}

export interface TiktokModerator {
  id: string;
  name: string;
  display_name: string;
  avatar?: string | null;
}

export interface TiktokBlockedWord {
  id: string;
  word: string;
}

export type TiktokModAction = "mute" | "unmute" | "block" | "unblock";
export type ModAction = "delete" | "timeout" | "untimeout" | "ban" | "unban" | "mod" | "unmod" | "vip" | "unvip";
export type ModCaps = Record<"twitch" | "youtube" | "tiktok", boolean>;

export interface StreamInfoResponse {
  twitch: {
    connected: boolean;
    error?: string;
    info?: { title: string; category_id: string; category_name: string };
  };
  youtube: {
    connected: boolean;
    error?: string;
    info?: { video_id: string; status: "live" | "upcoming"; title: string; description: string; category_id: string } | null;
  };
  tiktok: {
    connected: boolean;
    error?: string;
    info?: TiktokRoomInfo;
  };
}

export interface Category {
  id: string;
  name: string;
  image?: string;
}

type DeepPartial<T> = { [K in keyof T]?: T[K] extends object ? DeepPartial<T[K]> : T[K] };

export const api = {
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (patch: DeepPartial<Settings>) => invoke<Settings>("save_settings", { patch }),
  getStatuses: () => invoke<ConnStatus[]>("get_statuses"),
  connect: (platform: Platform) => invoke<void>("connect_platform", { platform }),
  disconnect: (platform: Platform) => invoke<void>("disconnect_platform", { platform }),
  logout: (platform: Platform) => invoke<void>("logout_platform", { platform }),
  twitchDeviceAuth: () => invoke<DeviceAuth>("twitch_device_auth"),
  youtubeAuth: () => invoke<string>("youtube_auth"),
  tiktokLogin: () => invoke<void>("tiktok_login"),
  tiktokShowWindow: () => invoke<void>("tiktok_show_window"),
  tiktokSetUsername: (username: string) => invoke<void>("tiktok_set_username", { username }),
  tiktokImportSession: (sessionId: string) => invoke<string>("tiktok_import_session", { sessionId }),
  tiktokRoomInfo: () => invoke<TiktokRoomInfo>("tiktok_room_info"),
  tiktokSetRoomSwitch: (setting: "comments" | "comment_filter", enabled: boolean) =>
    invoke<void>("tiktok_set_room_switch", { setting, enabled }),
  tiktokModerate: (userId: string, action: TiktokModAction, duration?: number) =>
    invoke<void>("tiktok_moderate", { userId, action, duration }),
  moderate: (request: { platform: Platform; action: ModAction; user_id: string; message_id?: string; duration?: number }) =>
    invoke<void>("moderate", { request }),
  moderationCaps: () => invoke<ModCaps>("moderation_caps"),
  tiktokModerators: () => invoke<TiktokModerator[]>("tiktok_moderators"),
  tiktokSetModerator: (userId: string, enabled: boolean) => invoke<void>("tiktok_set_moderator", { userId, enabled }),
  tiktokBlockedWords: () => invoke<TiktokBlockedWord[]>("tiktok_blocked_words"),
  tiktokAddBlockedWord: (word: string) => invoke<TiktokBlockedWord>("tiktok_add_blocked_word", { word }),
  tiktokDeleteBlockedWord: (wordId: string) => invoke<void>("tiktok_delete_blocked_word", { wordId }),
  getHistory: () => invoke<{ chat: SSEvent[]; alerts: SSEvent[] }>("get_history"),
  getStats: () => invoke<Stats>("get_stats"),
  clearHistory: () => invoke<void>("clear_history"),
  sendChat: (platform: string, message: string) => invoke<ResultMap>("send_chat", { platform, message }),
  getStreamInfo: () => invoke<StreamInfoResponse>("get_stream_info"),
  updateStreamInfo: (update: {
    platforms: Platform[];
    title?: string;
    description?: string;
    twitch_category_id?: string;
    youtube_category_id?: string;
  }) => invoke<ResultMap>("update_stream_info", { update }),
  searchTwitchCategories: (query: string) => invoke<Category[]>("search_twitch_categories", { query }),
  youtubeCategories: () => invoke<Category[]>("youtube_categories"),
  serverInfo: () => invoke<ServerInfo>("server_info"),
  openOverlaysDir: () => invoke<void>("open_overlays_dir"),
  restoreTemplates: () => invoke<void>("restore_templates"),
  testEvent: (kind: string, platform: string) => invoke<void>("test_event", { kind, platform }),
};

export const PLATFORMS: Platform[] = ["twitch", "youtube", "tiktok"];

export const PLATFORM_META: Record<EventPlatform, { name: string; color: string }> = {
  twitch: { name: "Twitch", color: "#9146ff" },
  youtube: { name: "YouTube", color: "#ff0033" },
  tiktok: { name: "TikTok", color: "#25f4ee" },
  system: { name: "Kombiniert", color: "#a3a3a3" },
};

export function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e);
}

/** Deutsche Texte für Alerts (identisch zum Overlay-SDK). */
export function formatAlert(ev: SSEvent): { title: string; text: string; message: string } {
  const n = ev.user ? ev.user.display_name || ev.user.name : "Jemand";
  const a = ev.amount ?? 0;
  const t = { title: "", text: "", message: ev.message ?? "" };
  switch (ev.type) {
    case "follow":
      return { ...t, title: "Neuer Follower", text: `${n} folgt jetzt` };
    case "subscribe":
      if (ev.platform === "youtube") return { ...t, title: "Neuer Abonnent", text: `${n} hat abonniert` };
      return { ...t, title: "Neues Abo", text: `${n} hat abonniert${ev.tier ? ` (${ev.tier})` : ""}` };
    case "resub":
      return { ...t, title: "Treue", text: `${n} ist seit ${a} Monaten dabei` };
    case "gift_sub":
      return { ...t, title: "Geschenkte Abos", text: `${n} verschenkt ${a} ${ev.platform === "youtube" ? "Mitgliedschaften" : "Abos"}` };
    case "membership":
      return { ...t, title: "Neues Mitglied", text: `${n} ist jetzt Mitglied${ev.tier ? ` (${ev.tier})` : ""}` };
    case "cheer":
      return { ...t, title: `${a} Bits`, text: `${n} hat ${a} Bits gecheert` };
    case "superchat":
      return { ...t, title: `Super Chat ${ev.amount_display ?? ""}`, text: `${n} sendet ${ev.amount_display ?? a}` };
    case "supersticker":
      return { ...t, title: `Super Sticker ${ev.amount_display ?? ""}`, text: `${n} sendet einen Super Sticker` };
    case "gift":
      return {
        ...t,
        title: `${ev.coins ?? 0} Coins`,
        text: `${n} sendet ${a > 1 ? `${a}× ` : ""}${ev.gift_name ?? "ein Geschenk"}`,
      };
    case "like": {
      const platforms = (ev.data?.platforms ?? {}) as Record<string, number>;
      const parts = Object.entries(platforms).map(([k, v]) => `${v} ${PLATFORM_META[k as Platform]?.name ?? k}`);
      const top = (ev.data?.top as { user: SSUser; count: number }[] | undefined)?.[0];
      return {
        title: `${a} Likes`,
        text: parts.length ? parts.join(" · ") : `${a} neue Likes`,
        message: top ? `Top: ${top.user.display_name || top.user.name} (${top.count})` : "",
      };
    }
    case "share":
      return { ...t, title: "Geteilt", text: `${n} hat den Stream geteilt` };
    case "raid":
      return { ...t, title: "Raid", text: `${n} raidet mit ${a} Zuschauern` };
    default:
      return { ...t, title: ev.type, text: n };
  }
}
