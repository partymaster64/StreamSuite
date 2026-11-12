// Beispiel: eigenes Alert-Overlay mit Warteschlange
const { helpers } = StreamSuite;
const box = document.getElementById("alert");
const DURATION = 5000;
const queue = [];
let showing = false;

function showNext() {
  if (showing || queue.length === 0) return;
  showing = true;
  const ev = queue.shift();

  // formatAlert liefert fertige deutsche Texte – oder baue deine eigenen aus ev.*
  const { title, text, message } = helpers.formatAlert(ev);

  // Eigene Logik je Typ, z. B. große TikTok-Geschenke hervorheben:
  if (ev.type === "gift" && ev.coins >= 100) {
    box.style.setProperty("--accent", "gold");
  } else {
    box.style.setProperty("--accent", helpers.platformColor(ev.platform));
  }

  document.getElementById("title").textContent = title;
  document.getElementById("text").textContent = text;
  document.getElementById("message").textContent = message;
  box.classList.remove("hidden");

  setTimeout(() => {
    box.classList.add("hidden");
    setTimeout(() => { showing = false; showNext(); }, 500);
  }, DURATION);
}

StreamSuite.connect().on("alert", (ev) => {
  queue.push(ev);
  showNext();
});
