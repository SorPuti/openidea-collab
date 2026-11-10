"use strict";
const invitation = window.location.href;
const hasSecret = /^[A-Za-z0-9_-]{43}$/.test(window.location.hash.slice(1));
window.history.replaceState(null, "", window.location.pathname);
const copy = document.getElementById("copy");
const status = document.getElementById("status");
copy.disabled = !hasSecret;
if (!hasSecret) status.textContent = "Peça um convite completo ao anfitrião.";
copy.addEventListener("click", async () => {
  try {
    await navigator.clipboard.writeText(invitation);
    status.textContent = "Convite copiado. Cole em “Aceitar convite” no plugin.";
  } catch {
    status.textContent = "O navegador não permitiu copiar. Peça o convite novamente e cole diretamente no plugin.";
  }
});
