/*!
 * StreamSuite Overlay SDK
 * Einbinden:  <script src="/sdk.js"></script>
 *
 *   const ss = StreamSuite.connect();
 *   ss.on("chat",   (ev) => { ... });   // alle Chatnachrichten
 *   ss.on("alert",  (ev) => { ... });   // alle Alerts (follow, subscribe, gift, like, …)
 *   ss.on("gift",   (ev) => { ... });   // ein bestimmter Event-Typ
 *   ss.on("stats",  (stats) => { ... });// Zuschauer / Likes
 *   ss.on("delete", (ev) => { ... });   // gelöschte Nachricht (ev.data.message_id)
 *   ss.on("history",(h) => { ... });    // { chat: [...], alerts: [...] } beim Verbinden
 *   ss.on("connected" | "disconnected", () => { ... });
 *   ss.sendChat("Hallo!", "all");       // "all" | "twitch" | "youtube" | "tiktok"
 *
 * Event-Format: siehe README.md im Overlay-Ordner.
 */
(function () {
  "use strict";

  var script = document.currentScript;
  var BASE = script && script.src ? new URL(script.src).origin : location.origin;
  var WS_URL = BASE.replace(/^http/, "ws") + "/ws";

  function Client() {
    this._handlers = {};
    this.history = { chat: [], alerts: [] };
    this.stats = null;
    this.status = [];
    this.connected = false;
    this._retry = 1000;
    this._connect();
  }

  Client.prototype.on = function (type, fn) {
    (this._handlers[type] = this._handlers[type] || []).push(fn);
    return this;
  };

  Client.prototype.off = function (type, fn) {
    var list = this._handlers[type] || [];
    this._handlers[type] = list.filter(function (f) { return f !== fn; });
    return this;
  };

  Client.prototype._emit = function (type, payload) {
    var list = this._handlers[type] || [];
    for (var i = 0; i < list.length; i++) {
      try { list[i](payload); } catch (e) { console.error("[StreamSuite]", e); }
    }
  };

  Client.prototype._connect = function () {
    var self = this;
    var ws;
    try { ws = new WebSocket(WS_URL); } catch (e) { return self._reconnect(); }
    self._ws = ws;
    ws.onopen = function () {
      self.connected = true;
      self._retry = 1000;
      self._emit("connected");
    };
    ws.onmessage = function (msg) {
      var data;
      try { data = JSON.parse(msg.data); } catch (e) { return; }
      if (data.op === "hello") {
        self.history = data.data.history;
        self.stats = data.data.stats;
        self.status = data.data.status;
        self._emit("history", self.history);
        self._emit("stats", self.stats);
      } else if (data.op === "event") {
        var ev = data.data;
        if (ev.type === "stats") {
          self.stats = ev.data;
          self._emit("stats", ev.data);
          return;
        }
        self._emit("event", ev);
        self._emit(ev.category, ev);
        self._emit(ev.type, ev);
      }
    };
    ws.onclose = function () {
      if (self.connected) self._emit("disconnected");
      self.connected = false;
      self._reconnect();
    };
    ws.onerror = function () { try { ws.close(); } catch (e) {} };
  };

  Client.prototype._reconnect = function () {
    var self = this;
    setTimeout(function () { self._connect(); }, self._retry);
    self._retry = Math.min(self._retry * 2, 10000);
  };

  Client.prototype.sendChat = function (message, platform) {
    return fetch(BASE + "/api/chat", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ message: message, platform: platform || "all" }),
    }).then(function (r) { return r.json(); });
  };

  // ---------------------------------------------------------------------------
  // Helfer
  // ---------------------------------------------------------------------------

  function escapeHtml(s) {
    return String(s == null ? "" : s)
      .replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;").replace(/'/g, "&#39;");
  }

  /** Nachricht als HTML (Twitch-Emotes werden als <img> dargestellt). */
  function renderMessage(ev) {
    if (ev.parts && ev.parts.length) {
      return ev.parts.map(function (p) {
        if (p.type === "emote" && p.url) {
          return '<img class="ss-emote" src="' + escapeHtml(p.url) + '" alt="' + escapeHtml(p.text) + '" title="' + escapeHtml(p.text) + '">';
        }
        return escapeHtml(p.text);
      }).join("");
    }
    return escapeHtml(ev.message || "");
  }

  var COLORS = { twitch: "#9146ff", youtube: "#ff0033", tiktok: "#25f4ee", system: "#a3a3a3" };
  var NAMES = { twitch: "Twitch", youtube: "YouTube", tiktok: "TikTok", system: "StreamSuite" };

  var ICONS = {
    twitch: '<svg viewBox="0 0 24 24" fill="currentColor"><path d="M4.3 3 3 6.4v13.3h4.6V22h2.6l2.3-2.3H16l5-5V3H4.3Zm15 10.8-2.9 2.9h-4.6l-2.4 2.3v-2.3H5.6V4.7h13.7v9.1ZM16.4 7.8v4.7h-1.7V7.8h1.7Zm-4.6 0v4.7h-1.7V7.8h1.7Z"/></svg>',
    youtube: '<svg viewBox="0 0 24 24" fill="currentColor"><path d="M23 7.2a3 3 0 0 0-2.1-2.1C19 4.6 12 4.6 12 4.6s-7 0-8.9.5A3 3 0 0 0 1 7.2 31 31 0 0 0 .5 12 31 31 0 0 0 1 16.8a3 3 0 0 0 2.1 2.1c1.9.5 8.9.5 8.9.5s7 0 8.9-.5a3 3 0 0 0 2.1-2.1c.4-1.6.5-3.2.5-4.8s-.1-3.2-.5-4.8ZM9.7 15.1V8.9l5.8 3.1-5.8 3.1Z"/></svg>',
    tiktok: '<svg viewBox="0 0 24 24" fill="currentColor"><path d="M16.6 5.8A4.3 4.3 0 0 1 15.5 3h-3.2v12.7a2.6 2.6 0 1 1-2.6-2.6c.3 0 .5 0 .8.1V9.9a5.9 5.9 0 1 0 5 5.8V9.3a7.5 7.5 0 0 0 4.4 1.4V7.5a4.3 4.3 0 0 1-3.3-1.7Z"/></svg>',
    system: '<svg viewBox="0 0 24 24" fill="currentColor"><path d="M12 21.4 10.6 20C5.4 15.4 2 12.3 2 8.5 2 5.4 4.4 3 7.5 3c1.7 0 3.4.8 4.5 2.1A6 6 0 0 1 16.5 3C19.6 3 22 5.4 22 8.5c0 3.8-3.4 6.9-8.6 11.5L12 21.4Z"/></svg>',
  };

  function name(ev) {
    return ev.user ? ev.user.display_name || ev.user.name : "Jemand";
  }

  /** Deutsche Standardtexte für Alerts: { title, text, message }. */
  function formatAlert(ev) {
    var n = name(ev);
    var a = ev.amount;
    var t = { title: "", text: "", message: ev.message || "" };
    switch (ev.type) {
      case "follow": t.title = "Neuer Follower"; t.text = n + " folgt jetzt!"; break;
      case "subscribe":
        if (ev.platform === "youtube") { t.title = "Neuer Abonnent"; t.text = n + " hat abonniert!"; }
        else if (ev.platform === "tiktok") { t.title = "Neues Abo"; t.text = n + " hat abonniert!"; }
        else { t.title = "Neues Abo"; t.text = n + " hat abonniert" + (ev.tier ? " (" + ev.tier + ")" : "") + "!"; }
        break;
      case "resub": t.title = "Treue!"; t.text = n + " ist seit " + a + " Monaten dabei!"; break;
      case "gift_sub": t.title = "Geschenkte Abos"; t.text = n + " verschenkt " + a + (ev.platform === "youtube" ? " Mitgliedschaften!" : " Abos!"); break;
      case "membership": t.title = "Neues Mitglied"; t.text = n + " ist jetzt Mitglied" + (ev.tier ? " (" + ev.tier + ")" : "") + "!"; break;
      case "cheer": t.title = a + " Bits"; t.text = n + " hat " + a + " Bits gecheert!"; break;
      case "superchat": t.title = "Super Chat " + (ev.amount_display || ""); t.text = n + " sendet " + (ev.amount_display || a); break;
      case "supersticker": t.title = "Super Sticker " + (ev.amount_display || ""); t.text = n + " sendet einen Super Sticker"; break;
      case "gift": t.title = (ev.coins || 0) + " Coins"; t.text = n + " sendet " + (a > 1 ? a + "× " : "") + (ev.gift_name || "ein Geschenk") + "!"; break;
      case "like":
        t.title = a + " Likes";
        var parts = [];
        var p = (ev.data && ev.data.platforms) || {};
        for (var k in p) parts.push(p[k] + " " + (NAMES[k] || k));
        t.text = parts.length ? parts.join(" · ") : a + " neue Likes";
        if (ev.data && ev.data.top && ev.data.top[0]) t.message = "Top: " + (ev.data.top[0].user.display_name || ev.data.top[0].user.name) + " (" + ev.data.top[0].count + ")";
        break;
      case "share": t.title = "Geteilt"; t.text = n + " hat den Stream geteilt!"; break;
      case "raid": t.title = "Raid!"; t.text = n + " raidet mit " + a + " Zuschauern!"; break;
      default: t.title = ev.type; t.text = n;
    }
    return t;
  }

  var params = new URLSearchParams(location.search);

  window.StreamSuite = {
    base: BASE,
    connect: function () { return new Client(); },
    params: params,
    param: function (key, fallback) { var v = params.get(key); return v == null || v === "" ? fallback : v; },
    helpers: {
      escapeHtml: escapeHtml,
      renderMessage: renderMessage,
      formatAlert: formatAlert,
      platformIcon: function (p) { return ICONS[p] || ICONS.system; },
      platformColor: function (p) { return COLORS[p] || COLORS.system; },
      platformName: function (p) { return NAMES[p] || p; },
    },
  };
})();
