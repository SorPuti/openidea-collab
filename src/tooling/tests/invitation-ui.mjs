// Optional browser evidence. Requires Playwright and Chromium on the developer machine.
import { createRequire } from "node:module";
import { spawn } from "node:child_process";
import { createServer } from "node:net";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve, join } from "node:path";
const { chromium } = createRequire(import.meta.url)("playwright");
const root = resolve(import.meta.dirname, "../../..");
const temporary = await mkdtemp(join(tmpdir(), "collab-ui-"));
const reservation = createServer();
await new Promise((ok) => reservation.listen(0, "127.0.0.1", ok));
const port = reservation.address().port;
await new Promise((ok) => reservation.close(ok));
const origin = `http://127.0.0.1:${port}`;
const child = spawn(join(root, "target/debug/collab-server"), [], {
  cwd: root, stdio: "ignore", env: { ...process.env, COLLAB_BIND: `127.0.0.1:${port}`, COLLAB_PUBLIC_ORIGIN: origin, COLLAB_STATE_FILE: join(temporary, "private/metadata.json") },
});
let browser;
try {
  let ready = false;
  for (let i = 0; i < 100; i++) {
    try { ready = (await fetch(`${origin}/healthz`)).status === 204; } catch {}
    if (ready) break;
    await new Promise((ok) => setTimeout(ok, 20));
  }
  if (!ready) throw new Error("Test relay failed to start");
  const session = await (await fetch(`${origin}/v1/sessions`, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ protocol: 1 }) })).json();
  const invite = await (await fetch(`${origin}/v1/sessions/${session.session_id}/invitations`, { method: "POST", headers: { "content-type": "application/json", authorization: `Bearer ${session.credential}` }, body: JSON.stringify({ role: "editor", ttl_seconds: 300 }) })).json();
  browser = await chromium.launch({ headless: true, executablePath: process.env.CHROMIUM_EXECUTABLE_PATH });
  const context = await browser.newContext({ permissions: ["clipboard-read", "clipboard-write"], viewport: { width: 900, height: 500 } });
  const page = await context.newPage();
  const requests = [];
  page.on("request", (request) => requests.push(new URL(request.url()).origin));
  await page.goto(invite.url);
  await page.getByRole("button", { name: "Copiar convite" }).click();
  await page.getByRole("status").filter({ hasText: "Convite copiado" }).waitFor();
  if (new URL(page.url()).hash) throw new Error("Fragment not removed from current history entry");
  const copied = await page.evaluate(() => navigator.clipboard.readText());
  if (copied !== invite.url) throw new Error("Copied invitation mismatch");
  if (requests.some((requestOrigin) => requestOrigin !== origin)) throw new Error("Unexpected third-party request");
  await page.screenshot({ path: join(root, "docs/assets/invitation-page.png"), fullPage: true });
  await page.goto(`${origin}/join/${crypto.randomUUID()}`);
  if (!(await page.getByRole("button", { name: "Copiar convite" }).isDisabled())) throw new Error("Missing fragment was accepted");
  console.log("Invitation UI: copy, fragment removal, missing-secret refusal and no third-party requests passed");
} finally {
  if (browser) await browser.close();
  child.kill("SIGTERM");
  await new Promise((ok) => child.once("close", ok));
  await rm(temporary, { recursive: true, force: true });
}
