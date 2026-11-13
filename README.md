# StreamSuite

Desktop-App (Tauri 2 + React + shadcn/ui) für macOS, Windows und Linux, die **Twitch, YouTube und TikTok** an einem Ort zusammenführt. Läuft vollständig lokal, ohne eigenen Server und ohne Cloud.

## Funktionen

- **Verbindungen**: Einrichtung und Status (nicht eingerichtet / getrennt / verbinde / verbunden / Fehler / LIVE) je Plattform
- **Multichat**: alle Chats in einem Fenster, Senden an Twitch + YouTube gleichzeitig
- **Alerts** (kombiniert):
  - Follows (Twitch, TikTok), Abos (Twitch), Abonnenten (YouTube), Mitgliedschaften und Super Chats (YouTube)
  - TikTok-Geschenke mit Coins (Streaks werden zusammengefasst)
  - **Kombinierte Likes** aus TikTok und YouTube, gesammelt pro Intervall
  - Bits, Raids, Sub-Gifts, Shares
- **Stream-Info**: Titel, Kategorie (Twitch-Kategorie / YouTube-Kategorie) und Beschreibung auf allen Plattformen gleichzeitig ändern
- **TikTok-LIVE-Steuerung** (eigener Kanal): aktueller Titel, Thema und Zahlen, Kommentare an/aus, Kommentarfilter, gesperrte Wörter, Moderatoren; im Multichat Zuschauer stummschalten, aus dem LIVE entfernen oder zu Moderatoren machen
- **OBS-Overlays** über einen lokalen Server (`http://127.0.0.1:4848`):
  - `/dock`: Multichat-Dock für OBS (benutzerdefiniertes Browser-Dock)
  - `/overlay/chat` und `/overlay/alerts`: fertige transparente Overlays mit URL-Parametern
  - `/custom/<name>/`: **eigene Templates** (HTML/CSS/JS) aus dem Overlay-Ordner, mit JavaScript-SDK (`/sdk.js`) und WebSocket-API
- **Multistreaming** läuft über das OBS-Plugin [Aitum Multistream](https://aitum.tv/products/multi). StreamSuite sendet selbst kein Video.

## App-Zugangsdaten (einmalig, nur für den Entwickler)

Endnutzer klicken nur auf „Anmelden“. Dafür braucht die App einmalig eigene Zugangsdaten, die beim Build eingebettet werden.
Kopiere `src-tauri/credentials.example.json` nach `src-tauri/credentials.json` (steht in .gitignore) und trage ein:

| Schlüssel | Woher |
|---|---|
| `twitch_client_id` | [dev.twitch.tv/console/apps/create](https://dev.twitch.tv/console/apps/create): Name beliebig, Redirect-URL `http://localhost`, Kategorie *Application Integration*, Client-Typ **Öffentlich**. Kein Secret nötig. |
| `google_client_id` / `google_client_secret` | [Google Cloud Console](https://console.cloud.google.com): Projekt anlegen → *YouTube Data API v3* aktivieren → *OAuth-Zustimmungsbildschirm* (Extern) → *Anmeldedaten* → OAuth-Client-ID vom Typ **Desktop-App**. Den Zustimmungsbildschirm auf **„In Produktion“** stellen, sonst laufen Anmeldungen nach 7 Tagen ab. Ohne Google-Verifizierung erscheint beim Login ein Hinweis „App nicht überprüft“ (→ *Erweitert → Weiter*), das Limit liegt dann bei 100 Nutzern. |

Alternativ als Umgebungsvariablen `SS_TWITCH_CLIENT_ID`, `SS_GOOGLE_CLIENT_ID`, `SS_GOOGLE_CLIENT_SECRET` (z. B. als GitHub-Secrets für den Build-Workflow).
TikTok braucht keine App-Zugangsdaten.
Nutzer können eigene Werte unter *Einstellungen → Erweitert* eintragen.

## Anmeldung

- **Twitch**: Der Browser öffnet sich, du meldest dich an und klickst auf *Autorisieren*.
- **YouTube**: Der Browser öffnet sich mit dem Google-Login.
- **TikTok**: In der App öffnet sich ein TikTok-Fenster. Melde dich dort per QR-Code oder E-Mail an; „Weiter mit Google“ funktioniert in eingebetteten Fenstern nicht. Der Kanal wird automatisch erkannt.
  - Events (Chat, Geschenke, Likes, Follows, Shares, Abos, Zuschauer) kommen direkt über TikToks Webcast-WebSocket – ohne Zusatzdienst, ohne API-Key. Die Protokoll-Implementierung stammt aus [PirateTok/live-rs](https://github.com/PirateTok/live-rs) (0BSD). Für das reine Lesen ist keine Anmeldung nötig.
  - **Chat senden** läuft kostenlos und lokal: Ein unsichtbares Fenster öffnet deine eigene LIVE-Seite mit deinem Login und schreibt die Nachricht ins Chatfeld. Das hängt am Aufbau der TikTok-Webseite und kann nach TikTok-Updates angepasst werden müssen.
  - **Steuerung** (Kommentare, Wortfilter, Moderation) läuft ebenfalls lokal über dieses Fenster: StreamSuite ruft die TikTok-Webcast-API im Kontext der eingeloggten Seite auf, die TikTok-Seite signiert die Anfragen selbst. Es ist kein externer Signier-Dienst beteiligt.

## Hintergrundbetrieb

Beim Schließen läuft StreamSuite im Hintergrund weiter (abschaltbar in den Einstellungen):
- **Windows/Linux**: Symbol in der Taskleiste/im Systembereich (Linux benötigt `libayatana-appindicator`, z. B. `sudo pacman -S libayatana-appindicator` bzw. `sudo apt install libayatana-appindicator3-1`)
- **macOS**: Symbol in der Menüleiste, Klick auf das Dock-Icon öffnet das Fenster
- Ein erneuter Start der App holt das Fenster immer nach vorne.

## Verwendete APIs

| Plattform | Schnittstelle | Anmeldung |
|---|---|---|
| Twitch | offiziell: [EventSub WebSocket](https://dev.twitch.tv/docs/eventsub/handling-websocket-events/) + [Helix API](https://dev.twitch.tv/docs/api/) | OAuth Device Code Flow (nur Client-ID, kein Secret) |
| YouTube | offiziell: [YouTube Data API v3](https://developers.google.com/youtube/v3) | OAuth 2.0 Desktop-App (Loopback 127.0.0.1 + PKCE) |
| TikTok | inoffiziell (TikTok hat keine offizielle LIVE-API): Webcast-WebSocket über [PirateTok/live-rs](https://github.com/PirateTok/live-rs) zum Lesen, Webcast-API im eingebetteten TikTok-Fenster für Chat-Senden, Moderation und Einstellungen | Login im TikTok-Fenster |

Was die Plattformen nicht anbieten:
- TikTok: Titel und Thema eines laufenden LIVE lassen sich nicht ändern. TikTok legt sie beim Erstellen des LIVE-Raums fest, und das geht nur mit dem signierten LIVE-Studio-Client (bzw. kostenpflichtigen Signier-Diensten). StreamSuite zeigt den aktuellen Titel an.
- TikTok: Es gibt keine offizielle Schnittstelle. Das Webcast-Protokoll kann sich jederzeit ändern; dann muss `piratetok-live-rs` aktualisiert werden (`cargo update -p piratetok-live-rs`).
- Twitch hat kein Beschreibungsfeld, die Beschreibung wird nur auf YouTube gesetzt.
- YouTube-Likes kommen ohne Nutzernamen (die API liefert nur die Gesamtzahl). Neue Abonnenten erscheinen nur, wenn deren Abos öffentlich sind.
- Die YouTube-API hat ein Tageskontingent (Standard 10.000 Einheiten). Das Chat-Abfrageintervall ist in den Einstellungen anpassbar.

## Entwicklung

Voraussetzungen: [Node.js](https://nodejs.org) ≥ 20, [Rust](https://rustup.rs) (stable) und die [Tauri-Systemabhängigkeiten](https://tauri.app/start/prerequisites/).

Linux (Debian/Ubuntu):
```bash
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```
Arch / CachyOS:
```bash
sudo pacman -S --needed webkit2gtk-4.1 base-devel curl wget file openssl appmenu-gtk-module libappindicator-gtk3 librsvg
```

```bash
npm install
npm run tauri dev      # Entwicklung
npm run tauri build    # Installer für das aktuelle Betriebssystem
```

Build-Ergebnisse liegen in `src-tauri/target/release/bundle/`:
- **Linux**: `.deb` (Debian/Ubuntu), `.rpm` (Fedora/openSUSE), `.AppImage` (alle Distributionen)
- **Windows**: `.msi` und `.exe` (NSIS)
- **macOS**: `.app` und `.dmg`

Installer für macOS und Windows lassen sich nur auf dem jeweiligen Betriebssystem bauen. Das übernimmt der GitHub-Workflow `.github/workflows/build.yml`:

- **Release veröffentlichen** (GitHub → *Releases* → *Draft a new release* → Tag wie `v0.2.0` → *Publish release*): Der Workflow baut macOS (Apple Silicon + Intel), Windows (x64) und Linux (x64 + ARM64) und hängt die Installer automatisch an dieses Release an (ca. 15–25 Minuten). Die App-Version wird dabei aus dem Tag übernommen (`v0.2.0` → `0.2.0`).
- **Manuell starten** (*Actions* → *Build* → *Run workflow*): baut alle Plattformen zum Testen; die Installer liegen als Artefakte am Workflow-Lauf.

Als Entwurf gespeicherte Releases starten noch keinen Build, erst das Veröffentlichen. Die Builds sind nicht signiert: Windows SmartScreen und macOS Gatekeeper zeigen beim ersten Start eine Warnung.

## Projektstruktur

```
src/                      React-Frontend (shadcn/ui)
  pages/                  Übersicht, Verbindungen, Multichat, Alerts, Stream-Info, Overlays, Einstellungen
  lib/api.ts              Typen + Tauri-Commands
  lib/store.tsx           Globaler Zustand (Events, Status, Statistiken)
src-tauri/src/
  lib.rs                  Tauri-Commands & Setup
  hub.rs                  Event-Hub: einheitliches Event-Format, Verlauf, Statistiken, Like-Aggregation
  twitch.rs / youtube.rs / tiktok.rs   Plattform-Integrationen (tiktok.rs: LIVE-Events über piratetok-live-rs)
  tiktok_web.rs           Eingebettetes TikTok-Fenster: Login, Chat-Senden, Webcast-Aufrufe im Seitenkontext
  tiktok_control.rs       TikTok-LIVE-Steuerung: Raum-Infos, Kommentare, Wortfilter, Moderation
  server.rs               Lokaler HTTP/WebSocket-Server für OBS
  config.rs               Einstellungen (JSON im App-Konfigurationsordner)
src-tauri/overlays/       Eingebaute Overlays, SDK und Beispiel-Templates
```

Daten liegen lokal im Konfigurationsordner des Betriebssystems (z. B. `~/.config/dev.streamsuite.app/settings.json`, unter Linux mit Dateirechten 600). Eigene Overlays liegen im Datenordner unter `overlays/`; die App zeigt den genauen Pfad unter **Overlays** an.

## Eigene Overlays

Siehe [`src-tauri/overlays/templates/README.md`](src-tauri/overlays/templates/README.md). Diese Datei liegt auch im Overlay-Ordner.
