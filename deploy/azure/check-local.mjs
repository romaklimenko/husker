// Runs the Azure Functions host on this computer with `husker serve` as its
// custom handler, and checks what the host forwards. It uses `host.json` and
// `serve/function.json` from this directory, so it tests the files that
// `deploy.ps1` uploads. The sync storage is the local test double, the profile
// is a temporary directory, and nothing reaches Azure.
//
//   cargo build --bin husker --example fake_blob
//   node deploy/azure/check-local.mjs
//
// Optional arguments: the husker executable, the fake_blob executable, and a
// work directory. The check needs Node 22 or newer and Azure Functions Core
// Tools 4 (`npm install -g azure-functions-core-tools@4`). Set FUNC to the path
// of the `func` executable when it is not in the standard place.
//
// What this check cannot see: the Azure front end before the host, the Linux
// build of husker, and the time a start takes in Azure.

import { execFileSync, spawn } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import net from "node:net";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, "../..");
const windows = process.platform === "win32";
const exe = windows ? ".exe" : "";
const husker = resolve(process.argv[2] ?? join(repo, `target/debug/husker${exe}`));
const fakeBlob = resolve(process.argv[3] ?? join(repo, `target/debug/examples/fake_blob${exe}`));
const work = resolve(process.argv[4] ?? join(repo, "target/functions-check"));
const func =
  process.env.FUNC ??
  (windows ? join(process.env.APPDATA ?? "", "npm/node_modules/azure-functions-core-tools/bin/func.exe") : "func");

const sleep = (ms) => new Promise((done) => setTimeout(done, ms));
const results = [];
const hosts = [];
const children = [];

// The server settings come from this script only, never from the caller's environment.
// Core Tools lets a variable of the shell win over local.settings.json, so a HUSKER_SYNC_URL
// of the shell would point the handler at a real sync file.
const env = { ...process.env };
for (const name of Object.keys(env)) {
  if (name.startsWith("HUSKER_") || name === "FUNCTIONS_CUSTOMHANDLER_PORT") delete env[name];
}

// The work directory holds a marker, so the script removes only what it made.
const MARKER = ".husker-check";

for (const [what, path] of [["husker", husker], ["fake_blob", fakeBlob], ...(windows ? [["func", func]] : [])]) {
  if (!existsSync(path)) {
    console.log(`Cannot find ${what} at ${path}. Read the comment at the top of this file.`);
    process.exit(2);
  }
}

function check(what, ok, detail = "") {
  results.push({ what, ok: Boolean(ok) });
  console.log(`${ok ? "  ok  " : "  FAIL"} ${what}${detail ? ` - ${detail}` : ""}`);
}

function freePort() {
  return new Promise((done) => {
    const server = net.createServer();
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address();
      server.close(() => done(port));
    });
  });
}

function localDate() {
  const date = new Date();
  const pad = (number) => String(number).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

async function waitFor(url, ms = 120000) {
  const started = Date.now();
  for (;;) {
    try {
      const response = await fetch(url);
      if (response.status < 500) return;
    } catch {
      // The host is still starting.
    }
    if (Date.now() - started > ms) throw new Error(`no answer from ${url}`);
    await sleep(250);
  }
}

/** Starts the Functions host in a directory. Its output goes to a file in the work directory. */
function startHost(directory, port, name) {
  const child = spawn(func, ["start", "--port", String(port)], {
    cwd: directory,
    env,
    stdio: ["ignore", "pipe", "pipe"],
    detached: !windows,
  });
  let output = "";
  child.stdout.on("data", (chunk) => (output += chunk));
  child.stderr.on("data", (chunk) => (output += chunk));
  child.on("exit", () => writeFileSync(join(work, `${name}.log`), output));
  const host = { child, output: () => output, stopped: false };
  hosts.push(host);
  return host;
}

/** Stops a host and the handler it started, and nothing else. */
function stopHost(host) {
  if (host.stopped) return;
  host.stopped = true;
  // A host that ended by itself has no process tree left, and its number may belong to another process by now.
  if (host.child.exitCode !== null || host.child.signalCode !== null) return;
  try {
    if (windows) execFileSync("taskkill", ["/PID", String(host.child.pid), "/T", "/F"], { stdio: "ignore" });
    else process.kill(-host.child.pid);
  } catch {
    // The host is gone already.
  }
}

function appFolder(name, hostJson, settings) {
  const directory = join(work, name);
  mkdirSync(join(directory, "serve"), { recursive: true });
  writeFileSync(join(directory, "host.json"), JSON.stringify(hostJson, null, 2));
  copyFileSync(join(here, "serve/function.json"), join(directory, "serve/function.json"));
  const values = {
    FUNCTIONS_WORKER_RUNTIME: "custom",
    AzureWebJobsStorage: "",
    // main.bicep sets this too. Without it the host answers the root path itself.
    AzureWebJobsDisableHomepage: "true",
    ...settings,
  };
  writeFileSync(join(directory, "local.settings.json"), JSON.stringify({ IsEncrypted: false, Values: values }, null, 2));
  return directory;
}

/** Stops what the script started. */
function cleanUp() {
  for (const host of hosts) stopHost(host);
  for (const child of children) child.kill();
}

async function main() {
  // The script works in a new or empty directory, or in one that an earlier run marked as its own.
  mkdirSync(work, { recursive: true });
  if (readdirSync(work).length > 0 && !existsSync(join(work, MARKER))) {
    console.log(`${work} holds files that this script did not make. Give it a new or an empty directory.`);
    process.exit(2);
  }
  writeFileSync(join(work, MARKER), "A Husker check script works here and removes what it made.\n");
  for (const name of ["profile", "echo", "app"]) {
    rmSync(join(work, name), { recursive: true, force: true, maxRetries: 20, retryDelay: 250 });
  }
  const profile = join(work, "profile");
  mkdirSync(profile, { recursive: true });
  const cli = (...args) => execFileSync(husker, ["--profile", profile, ...args], { encoding: "utf8", env }).trim();

  const blob = spawn(fakeBlob, [], { stdio: ["ignore", "pipe", "inherit"] });
  children.push(blob);
  const syncUrl = await new Promise((done) => {
    let text = "";
    blob.stdout.on("data", (chunk) => {
      text += chunk;
      const match = text.match(/Sync file URL: (\S+)/);
      if (match) done(match[1]);
    });
  });
  writeFileSync(join(profile, "sync.toml"), `url = "${syncUrl}"\n`);
  cli("list-add", "Work");
  cli("add", "Buy milk");
  cli("add", "Write report", "--list", "Work");
  cli("sync");
  const key = cli("token", "--public-key");
  const hostJson = JSON.parse(readFileSync(join(here, "host.json"), "utf8"));

  // 1. A handler that answers with the request it received shows what the host forwards.
  const echoPort = await freePort();
  const echo = appFolder(
    "echo",
    { ...hostJson, customHandler: { ...hostJson.customHandler, description: { defaultExecutablePath: process.execPath, arguments: ["echo.mjs"] } } },
    {},
  );
  writeFileSync(
    join(echo, "echo.mjs"),
    `import http from "node:http";
http.createServer((request, response) => {
  let body = "";
  request.on("data", (chunk) => (body += chunk));
  request.on("end", () => {
    response.writeHead(200, { "content-type": "application/json", "set-cookie": "probe=1; Path=/api; HttpOnly; SameSite=Strict", "x-probe": "kept" });
    response.end(JSON.stringify({ method: request.method, url: request.url, headers: request.headers, body }));
  });
}).listen(Number(process.env.FUNCTIONS_CUSTOMHANDLER_PORT), "127.0.0.1");
`,
  );
  const echoHost = startHost(echo, echoPort, "echo");
  await waitFor(`http://localhost:${echoPort}/probe`);
  const seen = await (
    await fetch(`http://localhost:${echoPort}/api/tasks/abc?view=all&x=a%20b`, {
      method: "PATCH",
      headers: {
        authorization: "Bearer token-value",
        cookie: "husker_session=cookie-value",
        origin: `http://localhost:${echoPort}`,
        "x-husker": "1",
        "content-type": "application/json",
        "mcp-protocol-version": "2025-06-18",
        accept: "application/json, text/event-stream",
      },
      body: JSON.stringify({ status: "done" }),
    })
  ).json();
  console.log("A handler behind the local Functions host received:");
  console.log(`  ${seen.method} ${seen.url}`);
  for (const [name, value] of Object.entries(seen.headers)) console.log(`  ${name}: ${value}`);
  check("the path and the query reach the handler unchanged", seen.url === "/api/tasks/abc?view=all&x=a%20b", seen.url);
  check("PATCH and its body reach the handler", seen.method === "PATCH" && seen.body === '{"status":"done"}');
  check("Authorization reaches the handler", seen.headers.authorization === "Bearer token-value");
  check("Cookie reaches the handler", seen.headers.cookie === "husker_session=cookie-value");
  check("Origin reaches the handler", seen.headers.origin === `http://localhost:${echoPort}`);
  check("X-Husker and the MCP header reach the handler", seen.headers["x-husker"] === "1" && seen.headers["mcp-protocol-version"] === "2025-06-18");
  check("X-Forwarded-Host names the address the client used", seen.headers["x-forwarded-host"] === `localhost:${echoPort}`, String(seen.headers["x-forwarded-host"]));
  const root = await fetch(`http://localhost:${echoPort}/`);
  const rootSeen = await root.json().catch(() => null);
  check("the root path reaches the handler", rootSeen?.url === "/", rootSeen ? rootSeen.url : `status ${root.status}`);
  check(
    "Set-Cookie and other answer headers pass the host",
    root.headers.get("set-cookie") === "probe=1; Path=/api; HttpOnly; SameSite=Strict" && root.headers.get("x-probe") === "kept",
    root.headers.get("set-cookie") ?? "no Set-Cookie",
  );
  stopHost(echoHost);

  // 2. husker serve behind the host, with the host.json that is deployed.
  const port = await freePort();
  const origin = `http://localhost:${port}`;
  const local = structuredClone(hostJson);
  local.customHandler.description.defaultExecutablePath = `husker${exe}`;
  const app = appFolder("app", local, {
    HUSKER_SYNC_URL: syncUrl,
    HUSKER_ONLINE_KEYS: key,
    HUSKER_ONLINE_ORIGIN: origin,
    HUSKER_ONLINE_TZ: "Europe/Copenhagen",
  });
  copyFileSync(husker, join(app, `husker${exe}`));
  const host = startHost(app, port, "app");
  const started = Date.now();
  await waitFor(`${origin}/api/health`);
  console.log(`husker serve answered behind the host after ${Date.now() - started} ms`);

  const health = await (await fetch(`${origin}/api/health`)).json();
  check("the health answer comes through the host", typeof health.version === "string", JSON.stringify(health));
  const page = await fetch(`${origin}/`);
  const html = await page.text();
  check("the page is served at the root path", page.status === 200 && html.includes('<script type="module" src="/app.js"></script>'), `status ${page.status}`);
  check(
    "the page keeps its content type and its content security policy",
    page.headers.get("content-type")?.startsWith("text/html") && page.headers.get("content-security-policy")?.includes("default-src 'self'"),
    `${page.headers.get("content-type")} | ${page.headers.get("content-security-policy")}`,
  );
  check(
    "the host keeps the cache and sniffing headers",
    page.headers.get("cache-control") === "no-cache" && page.headers.get("x-content-type-options") === "nosniff",
    `${page.headers.get("cache-control")} | ${page.headers.get("x-content-type-options")}`,
  );
  for (const [path, type] of [
    ["/app.js", "text/javascript"],
    ["/logic.js", "text/javascript"],
    ["/app.css", "text/css"],
    ["/sw.js", "text/javascript"],
    ["/strings.json", "application/json"],
    ["/manifest.webmanifest", "application/manifest+json"],
  ]) {
    const file = await fetch(`${origin}${path}`);
    check(`${path} is served with its type`, file.status === 200 && file.headers.get("content-type")?.startsWith(type), `${file.status} ${file.headers.get("content-type")}`);
  }
  const icon = await fetch(`${origin}/icon-192.png`);
  const iconBytes = (await icon.arrayBuffer()).byteLength;
  check("an image arrives whole", iconBytes === statSync(join(repo, "web/icon-192.png")).size, `${iconBytes} bytes`);
  const missing = await fetch(`${origin}/no/such/path`);
  check("an unknown path is answered by husker, not by the host", missing.status === 404 && (await missing.text()).includes('"error"'), `status ${missing.status}`);
  const head = await fetch(`${origin}/`, { method: "HEAD" });
  check("HEAD works for the page", head.status === 200, `status ${head.status}`);
  const noToken = await fetch(`${origin}/api/view`);
  check("a request without a token gets 401 and WWW-Authenticate", noToken.status === 401 && /bearer/i.test(noToken.headers.get("www-authenticate") ?? ""), `${noToken.status} ${noToken.headers.get("www-authenticate")}`);
  const foreign = await fetch(`${origin}/api/health`, { headers: { origin: "http://evil.example" } });
  check("another origin is refused", foreign.status === 403, `status ${foreign.status}`);

  // The phone page's requests: pairing, the cookie, and each method.
  const pairing = JSON.parse(cli("--json", "pair", "--url", origin));
  check("husker pair gets a code through the host", pairing.code?.length === 12);
  const json = { "content-type": "application/json", "x-husker": "1" };
  const session = await fetch(`${origin}/api/session`, { method: "POST", headers: json, body: JSON.stringify({ code: pairing.code }) });
  const setCookie = session.headers.get("set-cookie") ?? "";
  const attributes = setCookie.split(";").slice(1).map((part) => part.trim()).join("; ");
  check(
    "the session cookie arrives with its attributes",
    session.status === 200 && /HttpOnly/.test(setCookie) && /SameSite=Strict/.test(setCookie) && /Path=\/api/.test(setCookie),
    `status ${session.status}, attributes: ${attributes}`,
  );
  const cookie = setCookie.split(";")[0];
  const query = `view=all&today=${localDate()}&lang=en`;
  const view = await fetch(`${origin}/api/view?${query}`, { headers: { cookie } });
  check("the cookie reads the view", view.status === 200 && (await view.json()).lists?.length === 2, `status ${view.status}`);
  const id = "0123456789abcdef0123456789abcdef";
  const task = JSON.stringify({ id, title: "From the phone", preset: "today" });
  const added = await fetch(`${origin}/api/tasks?${query}`, { method: "POST", headers: { ...json, cookie }, body: task });
  check("POST adds a task", added.status === 200, `status ${added.status}`);
  const bare = await fetch(`${origin}/api/tasks?${query}`, { method: "POST", headers: { "content-type": "application/json", cookie }, body: task });
  check("a change without X-Husker is refused", bare.status === 403, `status ${bare.status}`);
  const changed = await fetch(`${origin}/api/tasks/${id}?${query}`, { method: "PATCH", headers: { ...json, cookie }, body: JSON.stringify({ description: "Changed", title: "From the phone", version: (await added.json()).task.version }) });
  check("PATCH changes a task", changed.status === 200 && (await changed.json()).task?.description === "Changed", `status ${changed.status}`);
  const out = await fetch(`${origin}/api/session`, { method: "DELETE", headers: { ...json, cookie } });
  check("DELETE signs out with 204 and removes the cookie", out.status === 204 && /Max-Age=0/.test(out.headers.get("set-cookie") ?? ""), `status ${out.status}`);

  // The agents' requests.
  const token = execFileSync(husker, ["--profile", profile, "token", "--name", "check", "--url", origin], {
    encoding: "utf8",
    env,
    stdio: ["ignore", "pipe", "ignore"],
  }).trim();
  const mcp = { authorization: `Bearer ${token}`, "content-type": "application/json", accept: "application/json, text/event-stream" };
  const initialize = JSON.stringify({
    jsonrpc: "2.0",
    id: 1,
    method: "initialize",
    params: { protocolVersion: "2025-06-18", capabilities: {}, clientInfo: { name: "check-local", version: "0" } },
  });
  const first = await fetch(`${origin}/mcp`, { method: "POST", headers: mcp, body: initialize });
  const firstBody = await first.text();
  check(
    "MCP initialize answers with JSON through the host",
    first.status === 200 && first.headers.get("content-type")?.startsWith("application/json") && firstBody.includes("serverInfo"),
    `status ${first.status} ${first.headers.get("content-type")}`,
  );
  const scoped = await fetch(`${origin}/mcp/list/Work`, { method: "POST", headers: mcp, body: initialize });
  check("the list path reaches husker", scoped.status === 200, `status ${scoped.status}`);
  const unknown = await fetch(`${origin}/mcp/list/Nothing`, { method: "POST", headers: mcp, body: initialize });
  check("an unknown list answers 404", unknown.status === 404, `status ${unknown.status}`);
  const get = await fetch(`${origin}/mcp`, { headers: mcp });
  check("GET /mcp answers 405", get.status === 405, `status ${get.status}`);
  const anonymous = await fetch(`${origin}/mcp`, { method: "POST", headers: { "content-type": "application/json", accept: mcp.accept }, body: initialize });
  check("MCP without a token answers 401", anonymous.status === 401, `status ${anonymous.status}`);

  cli("sync");
  check("the computer receives the change at its next sync", cli("tasks").includes("From the phone"));
  await sleep(500);
  const log = host.output();
  check("the host's output holds no access token and no sync address", !log.includes("hsk1_") && !log.includes(syncUrl));
  stopHost(host);
}

// Ctrl+C stops the hosts and the test storage too.
process.on("SIGINT", () => {
  cleanUp();
  process.exit(130);
});

let failure = null;
try {
  await main();
} catch (error) {
  failure = error;
} finally {
  cleanUp();
}
const failed = results.filter((result) => !result.ok);
console.log(`${results.length - failed.length} of ${results.length} checks passed. Host output: ${work}`);
if (failure) console.log(String(failure.stack ?? failure));
process.exit(failure || failed.length ? 1 : 0);
