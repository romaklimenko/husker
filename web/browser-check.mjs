// Drives the phone page in headless Chrome against a local `husker serve` and
// the test double of the sync storage. Nothing leaves this computer, and the
// only profile is a temporary directory under the output directory.
//
//   cargo build --bin husker --example fake_blob
//   node web/browser-check.mjs
//
// Optional arguments: the husker executable, the fake_blob executable, and the
// output directory, which receives one screenshot per step. Set CHROME to the
// path of a Chrome or Edge executable when it is not in a standard place.
// Keep the output directory path short on Windows: Chrome cannot store a
// service worker under a profile path longer than about 200 characters.
//
// This check is not part of `cargo test`. It needs Node 22 or newer and Chrome.

import { execFileSync, spawn } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import net from "node:net";
import { join, resolve } from "node:path";

const exe = process.platform === "win32" ? ".exe" : "";
const husker = resolve(process.argv[2] ?? `target/debug/husker${exe}`);
const fakeBlob = resolve(process.argv[3] ?? `target/debug/examples/fake_blob${exe}`);
const out = resolve(process.argv[4] ?? "target/browser-check");
const chromePath =
  process.env.CHROME ??
  [
    "C:/Program Files/Google/Chrome/Application/chrome.exe",
    "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/usr/bin/google-chrome",
    "/usr/bin/chromium",
  ].find((path) => existsSync(path));
const sleep = (ms) => new Promise((done) => setTimeout(done, ms));

for (const [what, path] of [["husker", husker], ["fake_blob", fakeBlob], ["Chrome", chromePath]]) {
  if (!path || !existsSync(path)) {
    console.log(`Cannot find ${what}${path ? ` at ${path}` : ""}. Read the comment at the top of this file.`);
    process.exit(2);
  }
}

// The script works in a new or empty directory, or in one that an earlier run marked as its own.
// It removes only what it made there.
const MARKER = ".husker-check";
mkdirSync(out, { recursive: true });
if (readdirSync(out).length > 0 && !existsSync(join(out, MARKER))) {
  console.log(`${out} holds files that this script did not make. Give it a new or an empty directory.`);
  process.exit(2);
}
writeFileSync(join(out, MARKER), "A Husker check script works here and removes what it made.\n");
// A browser of an earlier run can hold its files for a moment after it was closed.
for (const name of ["profile", "chrome"]) rmSync(join(out, name), { recursive: true, force: true, maxRetries: 20, retryDelay: 250 });
for (const name of readdirSync(out)) {
  if (/^\d\d-[a-z-]+\.png$/.test(name)) rmSync(join(out, name));
}
const profile = join(out, "profile");
mkdirSync(profile, { recursive: true });

const children = [];
const passed = [];
const notes = [];
const pageLog = [];
const questions = [];

// The server settings come from this script only, never from the caller's environment.
const baseEnv = { ...process.env };
for (const name of Object.keys(baseEnv)) {
  if (name.startsWith("HUSKER_") || name === "FUNCTIONS_CUSTOMHANDLER_PORT") delete baseEnv[name];
}

function cli(...args) {
  return execFileSync(husker, ["--profile", profile, ...args], { encoding: "utf8", env: baseEnv }).trim();
}

function check(what, condition, detail = "") {
  if (!condition) throw new Error(`FAILED: ${what}${detail ? ` - ${detail}` : ""}`);
  passed.push(what);
}

function isoDate(offset) {
  const date = new Date();
  date.setDate(date.getDate() + offset);
  const pad = (number) => String(number).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
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

function startFakeBlob() {
  return new Promise((done, fail) => {
    const child = spawn(fakeBlob, [], { stdio: ["ignore", "pipe", "inherit"] });
    children.push(child);
    let text = "";
    child.stdout.on("data", (chunk) => {
      text += chunk;
      const match = text.match(/Sync file URL: (\S+)/);
      if (match) done(match[1]);
    });
    child.on("exit", (code) => fail(new Error(`the test storage stopped with ${code}`)));
  });
}

function startServer(port, settings) {
  const child = spawn(husker, ["serve", "--port", String(port)], {
    stdio: "ignore",
    env: { ...baseEnv, ...settings },
  });
  children.push(child);
  return child;
}

async function waitHealth(origin, up = true) {
  for (let attempt = 0; attempt < 300; attempt++) {
    let ok = false;
    try {
      ok = (await fetch(`${origin}/api/health`)).ok;
    } catch {
      ok = false;
    }
    if (ok === up) return;
    await sleep(50);
  }
  throw new Error(up ? "the online server did not start" : "the online server did not stop");
}

// --- Chrome DevTools Protocol -------------------------------------------------

class Cdp {
  constructor(socket) {
    this.socket = socket;
    this.next = 0;
    this.waiting = new Map();
    this.onEvent = () => {};
    // A browser that went away fails every request that waits, so the check ends and cleans up.
    socket.addEventListener("close", () => {
      for (const entry of this.waiting.values()) entry.fail(new Error(`${entry.method}: the browser closed the connection`));
      this.waiting.clear();
      this.closed = true;
    });
    socket.addEventListener("message", (message) => {
      const data = JSON.parse(message.data);
      if (data.id) {
        const entry = this.waiting.get(data.id);
        this.waiting.delete(data.id);
        if (data.error) entry.fail(new Error(`${entry.method}: ${JSON.stringify(data.error)}`));
        else entry.done(data.result);
      } else this.onEvent(data);
    });
  }

  send(method, params = {}, sessionId) {
    if (this.closed) return Promise.reject(new Error(`${method}: the browser closed the connection`));
    const id = ++this.next;
    const promise = new Promise((done, fail) => this.waiting.set(id, { done, fail, method }));
    this.socket.send(JSON.stringify({ id, method, params, sessionId }));
    return promise;
  }
}

async function startChrome() {
  const data = join(out, "chrome");
  mkdirSync(data, { recursive: true });
  const child = spawn(
    chromePath,
    [
      "--headless=new",
      "--remote-debugging-port=0",
      `--user-data-dir=${data}`,
      "--no-first-run",
      "--no-default-browser-check",
      "--disable-extensions",
      "--disable-background-networking",
      "--window-size=390,844",
      "about:blank",
    ],
    { stdio: "ignore" },
  );
  children.push(child);
  const portFile = join(data, "DevToolsActivePort");
  for (let attempt = 0; attempt < 200 && !existsSync(portFile); attempt++) await sleep(50);
  const [port, path] = readFileSync(portFile, "utf8").trim().split("\n");
  const socket = new WebSocket(`ws://127.0.0.1:${port.trim()}${path.trim()}`);
  await new Promise((done, fail) => {
    socket.addEventListener("open", done);
    socket.addEventListener("error", fail);
  });
  return new Cdp(socket);
}

// Functions the check calls inside the page.
const HELPERS = `globalThis.__h ??= {
  snap() {
    const groups = [];
    let group = null;
    for (const element of document.querySelector("#rows").children) {
      if (element.matches("h2.group")) {
        group = { heading: element.textContent, rows: [] };
        groups.push(group);
      } else if (element.matches(".row:not(.adding)")) {
        if (!group) { group = { heading: null, rows: [] }; groups.push(group); }
        group.rows.push({
          title: element.querySelector(".title").textContent,
          meta: element.querySelector(".meta")?.textContent ?? "",
          closed: element.classList.contains("closed"),
        });
      }
    }
    return groups;
  },
  row(title) {
    return [...document.querySelectorAll("#rows .row:not(.adding)")].find((row) => row.querySelector(".title").textContent === title) ?? null;
  },
  headingOf(title) {
    let heading = null;
    for (const element of document.querySelector("#rows").children) {
      if (element.matches("h2.group")) heading = element.textContent;
      else if (!element.matches(".adding") && element.querySelector(".title")?.textContent === title) return heading ?? "(no heading)";
    }
    return "";
  },
  point(element) {
    if (!element) return null;
    element.scrollIntoView({ block: "center" });
    const box = element.getBoundingClientRect();
    const x = box.left + box.width / 2;
    const y = box.top + box.height / 2;
    const hit = document.elementFromPoint(x, y);
    return {
      x, y, w: Math.round(box.width), h: Math.round(box.height),
      ok: !!hit && (hit === element || element.contains(hit)),
      hit: hit ? hit.tagName + "#" + hit.id + "." + hit.className : "nothing",
    };
  },
};`;

let cdp;
let session;
let shots = 0;

async function evaluate(expression) {
  const answer = await cdp.send(
    "Runtime.evaluate",
    { expression: `${HELPERS}\n(${expression})`, awaitPromise: true, returnByValue: true },
    session,
  );
  if (answer.exceptionDetails) {
    throw new Error(`page script: ${answer.exceptionDetails.exception?.description ?? answer.exceptionDetails.text}`);
  }
  return answer.result.value;
}

async function waitFor(expression, what, ms = 10000) {
  const started = Date.now();
  for (;;) {
    if (await evaluate(expression)) return;
    if (Date.now() - started > ms) throw new Error(`FAILED: timed out waiting for ${what}`);
    await sleep(50);
  }
}

async function shot(name) {
  const { data } = await cdp.send("Page.captureScreenshot", { format: "png" }, session);
  shots += 1;
  writeFileSync(join(out, `${String(shots).padStart(2, "0")}-${name}.png`), Buffer.from(data, "base64"));
}

/** Clicks the middle of an element, as a finger would. Refuses a covered element. */
async function tap(elementExpression, what) {
  const point = await evaluate(`__h.point(${elementExpression})`);
  if (!point) throw new Error(`FAILED: no element to tap: ${what}`);
  if (!point.ok) throw new Error(`FAILED: ${what} is covered by ${point.hit}`);
  if (point.w < 40 || point.h < 40) notes.push(`small tap target: ${what} is ${point.w} x ${point.h} px`);
  const event = { x: point.x, y: point.y, button: "left", clickCount: 1 };
  await cdp.send("Input.dispatchMouseEvent", { type: "mousePressed", ...event }, session);
  await cdp.send("Input.dispatchMouseEvent", { type: "mouseReleased", ...event }, session);
}

/**
 * The controls on screen that have no name in the browser's accessibility tree,
 * which is what a screen reader reads.
 */
async function unnamedControls() {
  const { nodes } = await cdp.send("Accessibility.getFullAXTree", {}, session);
  const controls = new Set(["button", "textbox", "combobox", "checkbox", "Date", "listbox", "link", "menuitem"]);
  const unnamed = nodes.filter((node) => !node.ignored && controls.has(node.role?.value) && !(node.name?.value ?? "").trim());
  const described = [];
  for (const node of unnamed) {
    const { node: element } = await cdp.send("DOM.describeNode", { backendNodeId: node.backendDOMNodeId }, session);
    const attributes = element.attributes ?? [];
    const id = attributes[attributes.indexOf("id") + 1];
    described.push(`${node.role.value} ${element.nodeName}${attributes.includes("id") ? `#${id}` : ""}`);
  }
  return described;
}

const q = (selector) => `document.querySelector(${JSON.stringify(selector)})`;
const row = (title) => `__h.row(${JSON.stringify(title)})`;
const under = (title, start) => `__h.headingOf(${JSON.stringify(title)}).startsWith(${JSON.stringify(start)})`;
const snap = () => evaluate(`__h.snap()`);
const text = (selector) => evaluate(`${q(selector)}.textContent`);

async function type(value) {
  await cdp.send("Input.insertText", { text: value }, session);
}

async function pressEnter() {
  const key = { key: "Enter", code: "Enter", windowsVirtualKeyCode: 13, nativeVirtualKeyCode: 13 };
  await cdp.send("Input.dispatchKeyEvent", { type: "keyDown", text: "\r", ...key }, session);
  await cdp.send("Input.dispatchKeyEvent", { type: "keyUp", ...key }, session);
}

async function goto(url) {
  await cdp.send("Page.navigate", { url }, session);
  await waitFor(`document.readyState === "complete"`, `the page at ${url}`);
}

async function colors(value) {
  await cdp.send("Emulation.setEmulatedMedia", { features: [{ name: "prefers-color-scheme", value }] }, session);
}

/** Chooses an option as the phone's picker does: it sets the value and reports a change. */
async function choose(selector, value) {
  await evaluate(`(() => {
    const field = ${q(selector)};
    field.value = ${JSON.stringify(value)};
    field.dispatchEvent(new Event("change", { bubbles: true }));
  })()`);
}

async function openSheet(title) {
  await tap(`${row(title)}?.querySelector(".body")`, `the row ${title}`);
  await waitFor(`${q("#sheet")}.open`, `the sheet of ${title}`);
}

async function openMenu() {
  await tap(q("#menu-open"), "the menu button");
  await waitFor(`${q("#menu")}.open`, "the menu");
}

/** Page log lines other than the refusals this check causes on purpose. */
function unexpectedLog() {
  return pageLog.splice(0).filter((line) => !line.includes("/api/session") && !line.includes("/api/view"));
}

// --- The check ----------------------------------------------------------------

async function main() {
  const today = isoDate(0);
  const syncUrl = await startFakeBlob();
  writeFileSync(join(profile, "sync.toml"), `url = "${syncUrl}"\n`);

  const id = (output) => output.split("\t")[0];
  cli("list-add", "Work");
  cli("add", "Buy milk");
  cli("add", "Write report", "--list", "Work", "--due", today);
  cli("add", "Pay rent", "--due", isoDate(-3));
  cli("add", "Call Ann", "--due", isoDate(1));
  cli("add", "Plan trip", "--due", isoDate(10), "--description", "Flights first.\nThen the hotel.");
  cli("done", id(cli("add", "Old chore")));
  cli("trash", id(cli("add", "Trashed thing")));
  cli("sync");
  const key = cli("token", "--public-key");

  const port = await freePort();
  const origin = `http://127.0.0.1:${port}`;
  const settings = { HUSKER_SYNC_URL: syncUrl, HUSKER_ONLINE_KEYS: key, HUSKER_ONLINE_ORIGIN: origin };
  let server = startServer(port, settings);
  await waitHealth(origin);
  const pairing = JSON.parse(cli("--json", "pair", "--url", origin));
  check("husker pair returns a link with the code", pairing.link === `${origin}/#code=${pairing.code}`);

  cdp = await startChrome();
  const { targetId } = await cdp.send("Target.createTarget", { url: "about:blank" });
  session = (await cdp.send("Target.attachToTarget", { targetId, flatten: true })).sessionId;
  cdp.onEvent = (event) => {
    if (event.method === "Runtime.exceptionThrown") {
      const details = event.params.exceptionDetails;
      pageLog.push(`exception: ${details.exception?.description ?? details.text}`);
    } else if (event.method === "Runtime.consoleAPICalled" && ["error", "warning"].includes(event.params.type)) {
      pageLog.push(`console.${event.params.type}: ${event.params.args.map((arg) => arg.value ?? arg.description).join(" ")}`);
    } else if (event.method === "Log.entryAdded" && ["error", "warning"].includes(event.params.entry.level)) {
      pageLog.push(`log ${event.params.entry.source}: ${event.params.entry.text} ${event.params.entry.url ?? ""}`);
    } else if (event.method === "Page.javascriptDialogOpening") {
      // The page asks before a link replaces a pairing. The check answers yes.
      questions.push(event.params.message);
      cdp.send("Page.handleJavaScriptDialog", { accept: true }, session).catch((error) => pageLog.push(`dialog: ${error.message}`));
    }
  };
  for (const domain of ["Page", "Runtime", "Log", "Network", "DOM", "Accessibility"]) await cdp.send(`${domain}.enable`, {}, session);
  await cdp.send("Emulation.setDeviceMetricsOverride", { width: 390, height: 844, deviceScaleFactor: 2, mobile: true }, session);
  await cdp.send(
    "Emulation.setUserAgentOverride",
    {
      userAgent:
        "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Mobile Safari/537.36",
      acceptLanguage: "en-US",
    },
    session,
  );
  await colors("light");

  // 1. Without a session the page shows the pairing screen. A wrong code gets a message.
  await goto(`${origin}/`);
  await waitFor(`!${q("#pairing")}.hidden`, "the pairing screen");
  check("the pairing screen shows without a session", await evaluate(`${q("#app")}.hidden`));
  check("the pairing screen has its texts", (await text("#pair-submit")).length > 0 && (await text("#pair-hint")).length > 0);
  await shot("pairing");
  await tap(q("#pair-code"), "the code field");
  await type("AAAA-BBBB-CCCC");
  await pressEnter();
  await waitFor(`!${q("#pair-error")}.hidden`, "the message for a wrong code");
  notes.push(`wrong code message: ${await text("#pair-error")}`);
  let unexpected = unexpectedLog();
  check("no script errors on the pairing screen", unexpected.length === 0, unexpected.join(" | "));

  // 2. A link opened in the tab that already shows the page pairs it, and the code leaves the address.
  await evaluate(`location.assign(${JSON.stringify(pairing.link)})`);
  await waitFor(`!${q("#app")}.hidden && document.querySelectorAll("#rows .row").length > 0`, "rows after pairing");
  check("the address holds no code after pairing", (await evaluate("location.href")) === `${origin}/`);
  check("page script cannot read the cookie", (await evaluate("document.cookie")) === "");
  const { cookies } = await cdp.send("Network.getCookies", { urls: [`${origin}/api/view`] }, session);
  const cookie = cookies.find((entry) => entry.name === "husker_session");
  check(
    "the session cookie is HttpOnly, Strict, and under /api",
    cookie && cookie.httpOnly && cookie.sameSite === "Strict" && cookie.path === "/api",
    JSON.stringify(cookie && { ...cookie, value: "..." }),
  );
  const elsewhere = (await cdp.send("Network.getCookies", { urls: [`${origin}/mcp`] }, session)).cookies;
  check("the cookie is not sent to /mcp", !elsewhere.some((entry) => entry.name === "husker_session"));
  const policy = await evaluate(`fetch("/").then((response) => response.headers.get("content-security-policy"))`);
  check("the page carries a content security policy", policy?.includes("default-src 'self'"), String(policy));
  await shot("all-tasks");
  let unnamed = await unnamedControls();
  check("every control of the task screen has a name", unnamed.length === 0, unnamed.join(", "));

  let groups = await snap();
  const headings = groups.map((group) => group.heading);
  check(
    "All tasks has the terminal's groups in order",
    headings.length === 6 &&
      headings[0].startsWith("Earlier") &&
      headings[1].startsWith("Today") &&
      headings[2].startsWith("Tomorrow") &&
      headings[3].startsWith(isoDate(10)) &&
      headings[4].startsWith("Sometime") &&
      headings[5].startsWith("Done today"),
    headings.join(" | "),
  );
  check("every heading shows its count", headings.every((heading) => heading.endsWith("(1)")));
  const rent = groups[0].rows[0];
  check("an earlier task shows its planned date", rent.title === "Pay rent" && rent.meta.includes(isoDate(-3)), JSON.stringify(rent));
  check("a row names its list", groups[1].rows[0].meta.includes("Work"), JSON.stringify(groups[1].rows[0]));

  // 3. Add a task with the keyboard, then one with the button and another list.
  await tap(q("#add-title"), "the add field");
  await type("Buy stamps");
  await pressEnter();
  check("the add field is empty as soon as the task is sent", (await evaluate(`${q("#add-title")}.value`)) === "");
  await waitFor(under("Buy stamps", "Sometime"), "the added task under Sometime");
  passed.push("a new task without a date is under Sometime");
  const workId = await evaluate(`[...${q("#add-list")}.options].find((option) => option.textContent === "Work").value`);
  await choose("#add-list", workId);
  await tap(q("#refresh"), "the refresh button");
  await sleep(300);
  check("a refresh keeps the list chosen for a new task", (await evaluate(`${q("#add-list")}.value`)) === workId);
  await tap(q("#add-title"), "the add field");
  await type("Send invoice");
  await tap(q("#add-submit"), "the add button");
  await waitFor(`${row("Send invoice")} !== null`, "the task added with the button");
  check("the chosen list gets the task", (await evaluate(`${row("Send invoice")}.querySelector(".meta").textContent`)).includes("Work"));
  check("the add form stays on the list of the newest task", (await evaluate(`${q("#add-list")}.value`)) === workId);

  // 4. One tap closes a task. The row stays in place until the next refresh.
  await tap(`${row("Buy milk")}.querySelector(".check")`, "the checkbox of Buy milk");
  await waitFor(`${row("Buy milk")}.classList.contains("closed") && !${row("Buy milk")}.classList.contains("pending")`, "the closed row");
  check("a closed task keeps its row", await evaluate(under("Buy milk", "Sometime")));
  check(
    "the checkbox keeps the focus after its row is drawn again",
    await evaluate(`document.activeElement === ${row("Buy milk")}.querySelector(".check")`),
    await evaluate(`document.activeElement.tagName + "." + document.activeElement.className`),
  );
  await shot("closed-in-place");
  await tap(q("#refresh"), "the refresh button");
  await waitFor(under("Buy milk", "Done today"), "the closed task under Done today");
  await tap(`${row("Buy milk")}.querySelector(".check")`, "the checkbox of the closed task");
  await waitFor(`!${row("Buy milk")}.classList.contains("closed") && !${row("Buy milk")}.classList.contains("pending")`, "the reopened row");
  await tap(q("#refresh"), "the refresh button");
  await waitFor(under("Buy milk", "Sometime"), "the reopened task under Sometime");
  passed.push("a tap closes and reopens a task");

  // 5. The sheet: planned date, list, text, cancel, Trash with Undo.
  await openSheet("Plan trip");
  check(
    "the sheet shows the title and the description",
    (await evaluate(`${q("#task-title")}.value`)) === "Plan trip" &&
      (await evaluate(`${q("#task-description")}.value`)) === "Flights first.\nThen the hotel.",
  );
  check(
    "the sheet shows the planned date and the list",
    (await evaluate(`${q("#task-due")}.value`)) === isoDate(10) &&
      (await evaluate(`${q("#task-list")}.selectedOptions[0].textContent`)) === "Inbox",
  );
  check(
    "the sheet opens without a text field in focus, so no keyboard covers it",
    !(await evaluate(`document.activeElement.matches("input, textarea, select")`)),
    await evaluate(`document.activeElement.tagName + "#" + document.activeElement.id`),
  );
  await shot("sheet");
  unnamed = await unnamedControls();
  check("every control of the sheet has a name", unnamed.length === 0, unnamed.join(", "));
  check("the sheet is named after its task", (await evaluate(`${q("#sheet")}.getAttribute("aria-label")`)) === "Plan trip");
  // While a request of the sheet runs, its buttons do not react. The close button does.
  const waitingStyles = await evaluate(`(() => {
    const form = ${q("#sheet-form")};
    form.classList.add("saving");
    const styles = ["#task-save", "#task-trash", "#sheet-close"].map((selector) => getComputedStyle(document.querySelector(selector)).pointerEvents);
    form.classList.remove("saving");
    return styles.join(" ");
  })()`);
  check("a waiting sheet locks its buttons and keeps the close button", waitingStyles === "none none auto", waitingStyles);
  // The script reports the height of the phone's keyboard in --keyboard. The style sheet lifts the sheet.
  await evaluate(`document.documentElement.style.setProperty("--keyboard", "300px")`);
  const lifted = await evaluate(
    `(() => { const box = ${q("#sheet")}.getBoundingClientRect(); return { top: Math.round(box.top), bottom: Math.round(box.bottom), page: innerHeight }; })()`,
  );
  check("a sheet ends above the keyboard", lifted.top >= 0 && lifted.bottom <= lifted.page - 299, JSON.stringify(lifted));
  await shot("sheet-above-keyboard");
  await evaluate(`document.documentElement.style.setProperty("--keyboard", "0px")`);
  await tap(q(`#sheet [data-preset="tomorrow"]`), "Tomorrow in the sheet");
  await waitFor(`!${q("#sheet")}.open && ${under("Plan trip", "Tomorrow")}`, "Plan trip under Tomorrow");
  await openSheet("Plan trip");
  await tap(q(`#sheet [data-preset="clear"]`), "the no date button in the sheet");
  await waitFor(under("Plan trip", "Sometime"), "Plan trip under Sometime");
  await openSheet("Plan trip");
  check("the close button says Close while nothing is changed", (await text("#sheet-close")) === "Close");
  await choose("#task-due", isoDate(20));
  check("a date in the field waits for Save", await evaluate(`${q("#sheet")}.open`));
  check("the close button warns that it drops a change", (await text("#sheet-close")) === "Discard changes");
  await tap(q("#task-save"), "Save");
  await waitFor(`!${q("#sheet")}.open && ${under("Plan trip", isoDate(20))}`, "Plan trip under its date");
  passed.push("the sheet plans a task: preset, no date, and the date field with Save");

  // A button in the sheet sends typed text with its action.
  await openSheet("Plan trip");
  await tap(q("#task-description"), "the description field");
  await evaluate(`${q("#task-description")}.select()`);
  await type("Flights first.");
  await tap(q(`#sheet [data-preset="today"]`), "Today in the sheet");
  await waitFor(`!${q("#sheet")}.open && ${under("Plan trip", "Today")}`, "Plan trip under Today");
  await openSheet("Plan trip");
  check("a tapped date keeps the typed description", (await evaluate(`${q("#task-description")}.value`)) === "Flights first.");
  await tap(q("#sheet-close"), "Close in the sheet");
  await waitFor(`!${q("#sheet")}.open`, "the closed sheet");

  await openSheet("Plan trip");
  await choose("#task-list", workId);
  await waitFor(`!${q("#sheet")}.open && ${row("Plan trip")}.querySelector(".meta").textContent.includes("Work")`, "Plan trip in Work");
  passed.push("the sheet moves a task to another list");

  await openSheet("Plan trip");
  await tap(q("#task-title"), "the title field");
  await evaluate(`${q("#task-title")}.select()`);
  await type("Plan the summer trip");
  await pressEnter();
  await waitFor(`!${q("#sheet")}.open && ${row("Plan the summer trip")} !== null`, "the new title after Enter");
  await openSheet("Plan the summer trip");
  await tap(q("#task-description"), "the description field");
  await evaluate(`${q("#task-description")}.select()`);
  await type("Book in March.");
  await tap(q("#task-save"), "Save");
  await waitFor(`!${q("#sheet")}.open`, "the sheet closed after Save");
  await openSheet("Plan the summer trip");
  check("the sheet saves the description", (await evaluate(`${q("#task-description")}.value`)) === "Book in March.");
  await tap(q("#sheet-close"), "Close in the sheet");
  await waitFor(`!${q("#sheet")}.open`, "the closed sheet");
  passed.push("the sheet edits the title with Enter and the description with Save");

  // Closing a task from the sheet keeps its row in place, as a tap on the checkbox does.
  await openSheet("Call Ann");
  await tap(q("#task-cancel"), "Cancel task");
  await waitFor(`!${q("#sheet")}.open && ${row("Call Ann")}.classList.contains("closed") && !${row("Call Ann")}.classList.contains("pending")`, "the cancelled row");
  check("a task cancelled in the sheet keeps its row", await evaluate(under("Call Ann", "Tomorrow")));
  await openSheet("Send invoice");
  await tap(q("#task-toggle"), "Mark done in the sheet");
  await waitFor(`!${q("#sheet")}.open && ${row("Send invoice")}.classList.contains("closed") && !${row("Send invoice")}.classList.contains("pending")`, "the row marked done");
  check("a task marked done in the sheet keeps its row", await evaluate(under("Send invoice", "Sometime")));
  await tap(q("#refresh"), "the refresh button");
  await waitFor(`${under("Call Ann", "Cancelled today")} && ${under("Send invoice", "Done today")}`, "the closed tasks under their headings");
  await openSheet("Send invoice");
  await tap(q("#task-toggle"), "Reopen in the sheet");
  await waitFor(`!${q("#sheet")}.open && !${row("Send invoice")}.classList.contains("closed") && !${row("Send invoice")}.classList.contains("pending")`, "the reopened row");
  await tap(q("#refresh"), "the refresh button");
  await waitFor(under("Send invoice", "Sometime"), "the reopened task under Sometime");
  passed.push("the sheet cancels, closes, and reopens a task");

  await openSheet("Pay rent");
  await tap(q("#task-trash"), "Move to Trash");
  await waitFor(`${row("Pay rent")} === null && !${q("#toast")}.hidden`, "the row gone and the Undo message");
  await shot("undo-message");
  await tap(q("#toast-undo"), "Undo");
  await waitFor(`${row("Pay rent")} !== null`, "the row back after Undo");
  passed.push("Trash offers Undo and Undo restores the task");

  // 6. History and Trash.
  await openMenu();
  check("the menu names the pairing", (await text("#session")).startsWith("Paired as phone until "), await text("#session"));
  await shot("menu");
  await tap(q(`#menu [data-view="history"]`), "History in the menu");
  await waitFor(`${row("Old chore")} !== null`, "History rows");
  check(
    "History lists closed tasks and hides the add form",
    (await evaluate(`${q("#add")}.hidden`)) && (await evaluate(`${row("Call Ann")} !== null`)),
  );
  await shot("history");
  await openSheet("Old chore");
  await tap(q("#task-toggle"), "Reopen in the sheet");
  await waitFor(`${row("Old chore")} === null`, "the reopened task leaving History");
  passed.push("History reopens a task from its sheet");

  await openMenu();
  await tap(q(`#menu [data-view="trash"]`), "Trash in the menu");
  await waitFor(`${row("Trashed thing")} !== null`, "Trash rows");
  await shot("trash");
  await tap(`${row("Trashed thing")}.querySelector(".restore")`, "Restore");
  await waitFor(`${row("Trashed thing")} === null`, "the restored task leaving Trash");
  passed.push("Trash restores a task");

  // 7. Back to All tasks: Danish, one list, and the dark colors.
  await openMenu();
  await tap(q(`#menu [data-view="all"]`), "All tasks in the menu");
  await waitFor(`${row("Trashed thing")} !== null && ${row("Old chore")} !== null`, "the restored and reopened tasks in All tasks");
  await openMenu();
  await tap(q(`#menu [data-lang="da"]`), "Dansk in the menu");
  await waitFor(`[...document.querySelectorAll("h2.group")].some((heading) => heading.textContent.startsWith("I dag"))`, "Danish headings");
  check("the page language follows the choice", (await evaluate("document.documentElement.lang")) === "da");
  await shot("danish");
  await choose("#filter", workId);
  groups = await snap();
  const filtered = groups.flatMap((group) => group.rows.map((entry) => entry.title)).sort();
  check(
    "the list filter shows that list only",
    JSON.stringify(filtered) === JSON.stringify(["Plan the summer trip", "Send invoice", "Write report"]),
    filtered.join(", "),
  );
  check(
    "a filtered heading counts its own rows",
    groups.every((group) => group.heading.endsWith(`(${group.rows.length})`)),
    groups.map((group) => group.heading).join(" | "),
  );
  check("the add form follows the filter", (await evaluate(`${q("#add-list")}.value`)) === workId);
  await colors("dark");
  await shot("one-list-dark");
  await choose("#filter", "");
  await openSheet("Write report");
  await shot("sheet-dark");
  await tap(q("#sheet-close"), "Close in the sheet");
  await waitFor(`!${q("#sheet")}.open`, "the closed sheet");
  await colors("light");
  await openMenu();
  await tap(q(`#menu [data-lang="en"]`), "English in the menu");
  await waitFor(`[...document.querySelectorAll("h2.group")].some((heading) => heading.textContent.startsWith("Today"))`, "English headings");
  unexpected = unexpectedLog();
  check("no script errors while changing tasks", unexpected.length === 0, unexpected.join(" | "));

  // A pairing link opened on a paired phone asks before it replaces the pairing.
  const again = JSON.parse(cli("--json", "pair", "--name", "phone-again"));
  await evaluate(`location.assign(${JSON.stringify(again.link)})`);
  await waitFor(`${q("#session")}.textContent.includes("phone-again")`, "the new pairing");
  check(
    "a link asks before it replaces a pairing",
    questions.length === 1 && questions[0].startsWith("This phone is paired already"),
    questions.join(" | "),
  );
  check("the address holds no code after the second pairing", (await evaluate("location.href")) === `${origin}/`);

  // 8. The kept view opens while the server is away, and a change is rolled back.
  await goto(`${origin}/`);
  await waitFor(`navigator.serviceWorker.controller !== null && document.querySelectorAll("#rows .row").length > 0`, "the service worker in control");
  const kept = await evaluate(
    `caches.keys().then(async (names) => (await Promise.all(names.map((name) => caches.open(name).then((cache) => cache.keys())))).flat().map((request) => new URL(request.url).pathname))`,
  );
  check(
    "the service worker keeps page files only",
    kept.length > 0 && kept.every((path) => !path.startsWith("/api") && !path.startsWith("/mcp")),
    kept.join(", "),
  );
  const cacheNames = await evaluate(`caches.keys()`);
  check(
    "the kept files belong to one version of the page",
    cacheNames.length === 1 && /^husker-page-[0-9a-f]{16}$/.test(cacheNames[0]),
    cacheNames.join(", "),
  );
  server.kill();
  await waitHealth(origin, false);
  await goto(`${origin}/`);
  await waitFor(`document.querySelectorAll("#rows .row").length > 0 && !${q("#notice")}.hidden`, "the kept view and the message");
  notes.push(`message while the server is away: ${await text("#notice")}`);
  await shot("server-away");
  await tap(`${row("Write report")}.querySelector(".check")`, "a checkbox while the server is away");
  await waitFor(`!${q("#toast")}.hidden && !${row("Write report")}.classList.contains("closed")`, "the rolled back row and its message");
  notes.push(`message for a change that failed: ${await text("#toast-text")}`);
  passed.push("the kept view opens without the server, and a failed change is rolled back");

  // A task added while the server is away stays on screen and goes out when the server returns.
  await tap(q("#add-title"), "the add field");
  await type("Thought of on the train");
  await pressEnter();
  await waitFor(`document.querySelector("#rows .row.adding:not(.pending)") !== null`, "the task that was not sent");
  check("a task without an answer stays on screen", (await text("#rows .row.adding")).includes("Thought of on the train"));
  notes.push(`a task that was not sent says: ${await text("#rows .row.adding .meta")}`);
  await shot("not-sent");
  pageLog.length = 0;
  server = startServer(port, settings);
  await waitHealth(origin);
  await tap(q("#refresh"), "the refresh button");
  await waitFor(`${q("#notice")}.hidden`, "the message cleared after the server returned");
  await waitFor(`${row("Thought of on the train")} !== null && document.querySelector("#rows .row.adding") === null`, "the task sent after the server returned");
  passed.push("the page recovers when the server returns and sends the waiting task");

  // 9. Sign out, then pair a read-only session with a typed code.
  await openMenu();
  await tap(q("#sign-out"), "Sign out");
  await waitFor(`!${q("#pairing")}.hidden`, "the pairing screen after signing out");
  check("signing out removes the kept view", (await evaluate(`localStorage.getItem("husker.view")`)) === null);
  await waitFor(`localStorage.getItem("husker.signout") === null`, "the sign-out confirmed by the server");
  passed.push("the server confirms the sign-out");
  check("the API refuses the browser after signing out", (await evaluate(`fetch("/api/view").then((response) => response.status)`)) === 401);
  const readOnly = JSON.parse(cli("--json", "pair", "--name", "tablet", "--read-only"));
  const typed = `${readOnly.code.slice(0, 4)}-${readOnly.code.slice(4, 8)} ${readOnly.code.slice(8)}`.toLowerCase();
  await tap(q("#pair-code"), "the code field");
  await type(typed);
  await tap(q("#pair-submit"), "the pair button");
  await waitFor(`!${q("#app")}.hidden && document.querySelectorAll("#rows .row").length > 0`, "rows after typing a code");
  check("a read-only session hides the add form", await evaluate(`${q("#add")}.hidden`));
  check("a read-only session disables the checkboxes", await evaluate(`[...document.querySelectorAll(".check")].every((button) => button.disabled)`));
  notes.push(`read-only message: ${await text("#notice")}`);
  await openSheet("Write report");
  check(
    "a read-only sheet offers no change",
    await evaluate(`["#task-save", "#task-toggle", "#task-cancel", "#task-trash", "#plan", "#move"].every((selector) => document.querySelector(selector).hidden)`),
  );
  await shot("read-only-sheet");
  await tap(q("#sheet-close"), "Close in the sheet");

  // 10. The computer receives the phone's changes.
  notes.push(`husker sync on the computer: ${cli("sync")}`);
  const tasks = cli("tasks");
  check(
    "the computer receives the phone's changes",
    ["Buy stamps", "Send invoice", "Plan the summer trip", "Trashed thing", "Old chore", "Thought of on the train"].every((title) => tasks.includes(title)),
    tasks,
  );
  unexpected = unexpectedLog();
  check("no script errors at the end", unexpected.length === 0, unexpected.join(" | "));
}

/** Stops the browser, the server, and the test storage. */
function stopChildren() {
  for (const child of children) child.kill();
}

// Ctrl+C stops them too, and so does a check that hangs.
process.on("SIGINT", () => {
  stopChildren();
  process.exit(130);
});
const watchdog = setTimeout(() => {
  console.log("The check did not finish within five minutes.");
  stopChildren();
  process.exit(1);
}, 300000);
watchdog.unref();

let failure = null;
try {
  await main();
} catch (error) {
  failure = error;
  try {
    if (session) {
      await shot("failure");
      console.log("rows at the failure:", JSON.stringify(await snap()));
      console.log("open dialogs:", await evaluate(`[...document.querySelectorAll("dialog[open]")].map((dialog) => dialog.id).join(",")`));
    }
  } catch (second) {
    console.log("no page state:", second.message);
  }
} finally {
  try {
    if (cdp) await Promise.race([cdp.send("Browser.close"), sleep(2000)]);
  } catch {
    // Chrome is gone already.
  }
  stopChildren();
}

console.log(`passed ${passed.length} checks:`);
for (const line of passed) console.log(`  ok  ${line}`);
for (const line of notes) console.log(`  note  ${line}`);
console.log(`screenshots: ${out}`);
if (failure) {
  console.log(String(failure.stack ?? failure));
  process.exit(1);
}
console.log("ALL CHECKS PASSED");
process.exit(0);
