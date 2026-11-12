// Beispiel: eigenes Chat-Overlay mit dem StreamSuite SDK
const { helpers } = StreamSuite;
const chat = document.getElementById("chat");
const MAX_MESSAGES = 15;

const ss = StreamSuite.connect();

ss.on("chat", (ev) => {
  if (ev.type !== "chat") return; // "delete" kommt ebenfalls in dieser Kategorie

  const li = document.createElement("li");
  li.className = "message";
  li.dataset.id = ev.data?.message_id ?? ev.id;
  li.style.setProperty("--platform", helpers.platformColor(ev.platform));
  li.innerHTML = `
    <span class="user">${helpers.escapeHtml(ev.user.display_name)}</span>
    <span class="text">${helpers.renderMessage(ev)}</span>
  `;
  chat.appendChild(li);

  while (chat.children.length > MAX_MESSAGES) chat.firstElementChild.remove();
});

// Gelöschte Nachrichten (Moderation) entfernen
ss.on("delete", (ev) => {
  chat.querySelector(`[data-id="${CSS.escape(ev.data.message_id)}"]`)?.remove();
});
