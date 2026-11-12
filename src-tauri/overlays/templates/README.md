# Eigene Overlays für StreamSuite

Jeder Unterordner mit einer `index.html` in diesem Verzeichnis ist automatisch ein Overlay und unter

```
http://127.0.0.1:4848/custom/<ordnername>/
```

erreichbar (der Port steht in der App unter **Overlays**). Du kannst die URL direkt als Browserquelle in OBS einfügen.
Alle Dateien im Ordner (CSS, JS, Bilder, Sounds, Schriften) werden mitgeliefert. Nach einer Änderung lädst du die Quelle in OBS neu (Rechtsklick → Aktualisieren).

Die Ordner `beispiel-chat` und `beispiel-alerts` sind Vorlagen. Kopiere sie, um eigene Overlays zu bauen.

## SDK

```html
<script src="/sdk.js"></script>
<script>
  const ss = StreamSuite.connect();

  ss.on("chat",  (ev) => { /* Chatnachricht (type "chat") oder Löschung (type "delete") */ });
  ss.on("alert", (ev) => { /* alle Alerts */ });
  ss.on("gift",  (ev) => { /* nur TikTok-Geschenke – funktioniert mit jedem Event-Typ */ });
  ss.on("stats", (stats) => { /* { viewers, likes, platforms: { twitch: {...}, ... } } */ });
  ss.on("history", ({ chat, alerts }) => { /* Verlauf beim Verbinden */ });
  ss.on("connected", () => {});
  ss.on("disconnected", () => {});   // Das SDK verbindet sich automatisch neu

  ss.sendChat("Hallo Chat!", "all"); // "all" | "twitch" | "youtube" | "tiktok"
</script>
```

### Helfer

| Funktion | Beschreibung |
|---|---|
| `StreamSuite.helpers.renderMessage(ev)` | Nachricht als HTML, Twitch-Emotes als `<img class="ss-emote">`, Text ist escaped |
| `StreamSuite.helpers.formatAlert(ev)` | Deutsche Standardtexte `{ title, text, message }` |
| `StreamSuite.helpers.escapeHtml(text)` | Text sicher für `innerHTML` machen |
| `StreamSuite.helpers.platformIcon(p)` | Plattform-Logo als SVG-String |
| `StreamSuite.helpers.platformColor(p)` | Markenfarbe (`#9146ff`, `#ff0033`, `#25f4ee`) |
| `StreamSuite.helpers.platformName(p)` | Anzeigename der Plattform |
| `StreamSuite.param("key", fallback)` | URL-Parameter lesen, z. B. `?duration=8` |

## Event-Format

```jsonc
{
  "id": "uuid",
  "platform": "twitch" | "youtube" | "tiktok" | "system",
  "type": "chat",            // siehe Tabelle
  "category": "chat" | "alert" | "system",
  "timestamp": 1731420000000,
  "user": {
    "id": "123", "name": "login", "display_name": "Anzeigename",
    "avatar": "https://…",     // optional (YouTube, TikTok)
    "color": "#ff0000",        // optional (Twitch)
    "badges": ["moderator", "subscriber"],
    "is_mod": false, "is_subscriber": false, "is_owner": false
  },
  "message": "Text",            // optional
  "parts": [{ "type": "text" | "emote", "text": "Kappa", "url": "…" }],
  "amount": 5,                  // optional, Bedeutung je Typ
  "currency": "EUR",            // Super Chat
  "amount_display": "5,00 €",   // Super Chat
  "coins": 50,                  // TikTok-Coins gesamt
  "gift_name": "Rose",          // TikTok
  "gift_image": "https://…",    // TikTok
  "tier": "Tier 1",             // Twitch-Abo-Stufe / YouTube-Mitgliedsstufe
  "data": {},                   // Zusatzdaten (z. B. message_id)
  "test": true                  // nur bei Test-Alerts aus der App
}
```

| type | Plattformen | `amount` |
|---|---|---|
| `chat` | alle | – |
| `delete` | Twitch, YouTube | – (`data.message_id`) |
| `follow` | Twitch, TikTok | – |
| `subscribe` | Twitch (Abo), YouTube (Abonnent), TikTok | – |
| `resub` | Twitch, YouTube (Mitglied-Meilenstein) | Monate |
| `gift_sub` | Twitch, YouTube | Anzahl |
| `membership` | YouTube | – |
| `cheer` | Twitch | Bits |
| `superchat` / `supersticker` | YouTube | Betrag |
| `gift` | TikTok | Anzahl (Streak), `coins` = Gesamtwert |
| `like` | `system` (kombiniert aus TikTok + YouTube) | Likes im Intervall, `data.platforms`, `data.top` |
| `share` | TikTok | – |
| `raid` | Twitch | Zuschauer |
| `join` | TikTok | – (Kategorie `system`) |

## REST

- `GET /api/history` – letzter Chat- und Alert-Verlauf
- `GET /api/stats` – Zuschauer & Likes
- `GET /api/status` – Verbindungsstatus der Plattformen
- `POST /api/chat` – `{ "message": "…", "platform": "all" }`
- WebSocket `ws://127.0.0.1:<port>/ws` – `{ op: "hello", data }` und `{ op: "event", data: Event }`
