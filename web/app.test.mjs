// Tests of the page script against a small stand-in for the document. They
// check what the page sends and what it shows, also when answers arrive late or
// not at all. They do not replace `web/browser-check.mjs` or a real phone.
// Run them with: node --test web/app.test.mjs

import assert from "node:assert/strict";
import { test } from "node:test";

// The page hides its message after six seconds. The timer must not keep this run alive.
const realSetTimeout = globalThis.setTimeout;
globalThis.setTimeout = (callback, ms, ...rest) => {
  const timer = realSetTimeout(callback, ms, ...rest);
  timer.unref?.();
  return timer;
};

class Element {
  constructor(tag, id = "") {
    this.tag = tag;
    this.id = id;
    this.children = [];
    this.listeners = {};
    this.attributes = {};
    this.dataset = {};
    this.styles = {};
    this.style = { setProperty: (name, value) => (this.styles[name] = value) };
    this.names = new Set();
    this.classList = {
      toggle: (name, on) => (on ? this.names.add(name) : this.names.delete(name)),
      add: (name) => this.names.add(name),
    };
    this.hidden = false;
    this.disabled = false;
    this.open = false;
    this.value = "";
    this.textContent = "";
    this.className = "";
  }
  addEventListener(type, listener) {
    (this.listeners[type] ??= []).push(listener);
  }
  setAttribute(name, value) {
    this.attributes[name] = value;
  }
  append(...nodes) {
    this.children.push(...nodes);
  }
  replaceChildren(...nodes) {
    this.children = [...nodes];
  }
  showModal() {
    this.open = true;
  }
  close() {
    this.open = false;
  }
  async fire(type, event = {}) {
    for (const listener of this.listeners[type] ?? []) {
      await listener({ preventDefault() {}, target: this, ...event });
    }
  }
  /** Every element below this one, in document order. */
  all() {
    return this.children.flatMap((child) => [child, ...child.all()]);
  }
  text() {
    return [this.textContent, ...this.children.map((child) => child.text())].filter(Boolean).join(" ");
  }
}

/** A field that changes line breaks when it takes text, as a browser does. */
class Field extends Element {
  constructor(id, takes) {
    super("input", id);
    this.takes = takes;
  }
  get value() {
    return this.shown ?? "";
  }
  set value(text) {
    // The base class sets an empty value before `takes` exists.
    this.shown = this.takes ? this.takes(String(text)) : String(text);
  }
}

// What Chrome does, measured: a text input drops line breaks, a textarea turns every line break into "\n".
const FIELDS = {
  "task-title": (text) => text.replace(/[\r\n]/g, ""),
  "task-description": (text) => text.replace(/\r\n?/g, "\n"),
};

// The elements that `web/index.html` hides until the script shows them.
const HIDDEN = ["app", "pairing", "notice", "more", "pair-error", "sheet-error", "session", "toast"];

const row = (id, title, listId, listName, extra = {}) => ({
  id,
  version: `v-${id}`,
  list_id: listId,
  list_name: listName,
  title,
  description: "",
  status: "open",
  due: null,
  due_label: null,
  show_due: false,
  deleted: false,
  ...extra,
});

const sampleView = () => ({
  view: "all",
  today: "2026-11-15",
  write: true,
  session: { id: "0123456789ab", name: "phone", expires: "2027-11-15", write: true, list: null },
  lists: [
    { id: "inbox", name: "Inbox", count: 1, default: true },
    { id: "work", name: "Work", count: 2, default: false },
  ],
  suggested_list: "work",
  trash_count: 0,
  total: 3,
  next_offset: null,
  groups: [
    {
      key: "overdue",
      label: "Earlier",
      count: 1,
      rows: [row("a", "Overdue report", "work", "Work", { due: "2026-11-10", due_label: "2026-11-10 Tue W46", show_due: true })],
    },
    {
      key: "today",
      label: "Today - 2026-11-15 Sun W46",
      count: 2,
      rows: [row("b", "Buy milk", "inbox", "Inbox"), row("c", "Call Ann", "work", "Work")],
    },
  ],
});

/** A view in which one row has other values. */
const viewWith = (id, values) => {
  const view = sampleView();
  for (const group of view.groups) {
    group.rows = group.rows.map((entry) => (entry.id === id ? { ...entry, ...values } : entry));
  }
  return view;
};

const unauthorized = { status: 401, body: { error: { code: "unauthorized", message: "no" } } };
const paired = { body: { id: "0123456789ab", name: "phone", expires: "2027-11-15", write: true, list: null } };
const stale = { status: 409, body: { error: { code: "stale_version", message: "stale" } } };
const offline = () => {
  throw new TypeError("no network");
};

let caseNumber = 0;

/** Loads a fresh copy of the page script into a stand-in browser. */
async function openPage({ hash = "", answers = {}, language = "en-GB", kept = null, storage = {}, confirmAnswer = true, workers = null, viewport = null } = {}) {
  const elements = new Map();
  const group = (attribute, values) =>
    values.map((value) => {
      const element = new Element("button");
      element.dataset[attribute] = value;
      return element;
    });
  const groups = {
    "[data-preset]": group("preset", ["today", "tomorrow", "next_monday", "clear"]),
    "[data-view]": group("view", ["all", "history", "trash"]),
    "[data-lang]": group("lang", ["en", "da"]),
  };
  const keptEntries = Object.entries(storage).map(([key, value]) => [key, JSON.stringify(value)]);
  const store = new Map(kept ? [["husker.view", JSON.stringify(kept)], ...keptEntries] : keptEntries);
  const requests = [];
  const page = {
    $: (id) => {
      if (!elements.has(id)) {
        const element = id in FIELDS ? new Field(id, FIELDS[id]) : new Element("div", id);
        element.hidden = HIDDEN.includes(id);
        elements.set(id, element);
      }
      return elements.get(id);
    },
    groups,
    requests,
    storage: store,
    visibility: new Element("document"),
    window: new Element("window"),
    confirms: [],
    reloads: 0,
  };

  globalThis.document = {
    documentElement: new Element("html"),
    visibilityState: "visible",
    getElementById: page.$,
    createElement: (tag) => new Element(tag),
    querySelectorAll: (selector) =>
      selector === "dialog[open]" ? [page.$("sheet"), page.$("menu")].filter((dialog) => dialog.open) : groups[selector],
    addEventListener: (type, listener) => page.visibility.addEventListener(type, listener),
  };
  globalThis.window = {
    addEventListener: (type, listener) => page.window.addEventListener(type, listener),
    visualViewport: viewport,
    innerHeight: 800,
  };
  globalThis.confirm = (message) => {
    page.confirms.push(message);
    return confirmAnswer;
  };
  globalThis.localStorage = {
    getItem: (key) => store.get(key) ?? null,
    setItem: (key, value) => store.set(key, value),
    removeItem: (key) => store.delete(key),
  };
  Object.defineProperty(globalThis, "navigator", {
    value: workers ? { language, serviceWorker: workers } : { language },
    configurable: true,
  });
  globalThis.location = { origin: "https://tasks.example.net", pathname: "/", hash, reload: () => (page.reloads += 1) };
  globalThis.history = { replaceState: () => (globalThis.location.hash = "") };
  globalThis.fetch = async (input, options = {}) => {
    const url = new URL(String(input), "https://tasks.example.net");
    const method = options.method ?? "GET";
    const request = {
      method,
      path: url.pathname,
      query: Object.fromEntries(url.searchParams),
      headers: options.headers ?? {},
      body: options.body ? JSON.parse(options.body) : undefined,
      limited: options.signal instanceof AbortSignal,
    };
    requests.push(request);
    if (url.pathname === "/strings.json") {
      const strings = { no_tasks: "No tasks", web_server_error: "Server error {status}", web_paired_as: "{name} until {date} ({id})" };
      return { ok: true, status: 200, json: async () => ({ en: strings, da: { no_tasks: "Ingen opgaver" } }) };
    }
    // Without an answer of the test, a phone has no session yet and every other request answers a view.
    const fallback = method === "GET" && url.pathname === "/api/session" ? unauthorized : undefined;
    const answer = answers[`${method} ${url.pathname}`] ?? answers[method] ?? fallback;
    // An answer can be a promise that the test releases later, as a slow server would answer.
    const { status = 200, body = sampleView() } = typeof answer === "function" ? await answer(request) : (answer ?? {});
    return {
      ok: status < 400,
      status,
      json: async () => {
        if (body === "not json") throw new SyntaxError("not json");
        return body;
      },
    };
  };

  caseNumber += 1;
  await import(`./app.js?case=${caseNumber}`);
  await settle();
  return page;
}

/** Lets pending promises of the page run. */
async function settle() {
  for (let turn = 0; turn < 20; turn += 1) await new Promise((resolve) => setImmediate(resolve));
}

const apiRequests = (page) => page.requests.filter((request) => request.path.startsWith("/api/"));
const sent = (page, method) => apiRequests(page).filter((request) => request.method === method);
const calls = (page) => apiRequests(page).map((request) => `${request.method} ${request.path}`);

/** An answer that the test releases later. */
function held() {
  let release;
  const promise = new Promise((resolve) => (release = resolve));
  return { promise, release };
}

const withClass = (page, name) =>
  page
    .$("rows")
    .all()
    .filter((element) => element.className === name);
const closedRows = (page) =>
  page
    .$("rows")
    .all()
    .filter((element) => element.names.has("closed"));
const pendingRows = (page) =>
  page
    .$("rows")
    .all()
    .filter((element) => element.className === "row" && element.names.has("pending"));
const headings = (page) => page.$("rows").children.filter((child) => child.tag === "h2").map((child) => child.textContent);
const waiting = (page) => withClass(page, "row adding");

/** Opens the sheet of the row at this position. */
async function openSheet(page, position) {
  await withClass(page, "body")[position].fire("click");
  assert.equal(page.$("sheet").open, true);
}

const addAnswer = (request) => ({ body: { task: row(request.body.id, request.body.title, "work", "Work"), view: sampleView() } });

test("the page loads All tasks and draws headings and rows", async () => {
  const page = await openPage();
  const [first] = apiRequests(page);
  assert.equal(first.path, "/api/view");
  assert.equal(first.query.view, "all");
  assert.equal(first.query.lang, "en");
  assert.match(first.query.today, /^\d{4}-\d{2}-\d{2}$/);
  assert.equal(first.headers["X-Husker"], "1");
  assert.equal(first.limited, true, "a request gives up after a time");

  assert.deepEqual(headings(page), ["Earlier (1)", "Today - 2026-11-15 Sun W46 (2)"]);
  const text = page.$("rows").text();
  assert.ok(text.includes("Overdue report") && text.includes("2026-11-10 Tue W46"), text);
  assert.ok(text.includes("Work"), "the list name shows in the all-lists view");
  assert.equal(page.$("app").hidden, false);
  assert.equal(page.$("pairing").hidden, true);
  assert.equal(page.$("add").hidden, false);
  assert.equal(page.$("add-list").value, "work", "a new task starts in the suggested list");
  assert.ok(page.storage.has("husker.view"), "the view is kept for the next start");
  assert.equal(page.$("session").hidden, false);
  assert.equal(page.$("session").textContent, "phone until 2027-11-15 (0123456789ab)", "the menu names the pairing and its token");
  assert.notEqual(page.$("filter").attributes["aria-label"], page.$("add-list").attributes["aria-label"], "the two list pickers have names of their own");
});

test("a tap on the checkbox changes the row first and asks for the row alone", async () => {
  const page = await openPage({
    answers: {
      "PATCH /api/tasks/b": (request) => ({
        body: { task: { ...row("b", "Buy milk", "inbox", "Inbox"), status: request.body.status, version: "v2" } },
      }),
    },
  });
  await withClass(page, "check")[1].fire("click");
  await settle();
  const [change] = sent(page, "PATCH");
  assert.equal(change.path, "/api/tasks/b");
  assert.deepEqual(change.body, { status: "done" });
  assert.equal(change.query.view, "none", "the answer needs no view");
  assert.equal(closedRows(page).length, 1, "the row shows as closed and keeps its place");
  assert.equal(headings(page).length, 2, "the groups stay as they were");
  const lists = page.$("filter").children.map((entry) => entry.textContent);
  assert.deepEqual(lists.slice(1), ["Inbox (0)", "Work (2)"], "the closed task no longer counts for its list");
});

test("a failed change puts the row back and shows the server's message", async () => {
  const page = await openPage({
    answers: { "PATCH /api/tasks/b": { status: 502, body: { error: { code: "storage_unavailable", message: "The storage is away." } } } },
  });
  await withClass(page, "check")[1].fire("click");
  await settle();
  assert.equal(closedRows(page).length, 0);
  assert.equal(page.$("toast").hidden, false);
  assert.equal(page.$("toast-text").textContent, "The storage is away.");
});

test("a change without an answer goes back, says so, and asks for the tasks again", async () => {
  const page = await openPage({ answers: { "PATCH /api/tasks/b": offline } });
  await withClass(page, "check")[1].fire("click");
  await settle();
  assert.equal(closedRows(page).length, 0);
  assert.equal(page.$("toast-text").textContent, "web_not_saved");
  assert.equal(calls(page).at(-1), "GET /api/view", "the change may have arrived, so the view is read again");
});

test("an error that is not the server's shows a text with the status", async () => {
  const page = await openPage({ answers: { "PATCH /api/tasks/b": { status: 503, body: "not json" } } });
  await withClass(page, "check")[1].fire("click");
  await settle();
  assert.equal(page.$("toast-text").textContent, "Server error 503");
});

test("the list filter hides the other lists", async () => {
  const page = await openPage();
  const filter = page.$("filter");
  filter.value = "inbox";
  await filter.fire("change");
  assert.deepEqual(headings(page), ["Today - 2026-11-15 Sun W46 (1)"]);
  assert.equal(page.$("add-list").value, "inbox", "a new task goes to the filtered list");
});

test("adding a task sends an id and shows the new view", async () => {
  const page = await openPage({ answers: { "POST /api/tasks": addAnswer } });
  page.$("add-title").value = "  New task  ";
  await page.$("add").fire("submit");
  await settle();
  const [add] = sent(page, "POST");
  assert.equal(add.path, "/api/tasks");
  assert.match(add.body.id, /^[0-9a-f]{32}$/);
  assert.equal(add.body.title, "New task");
  assert.equal(add.body.list_id, "work");
  assert.equal(page.$("add-title").value, "");
  assert.equal(waiting(page).length, 0, "the row of the new task comes from the answer");
  assert.equal(page.storage.has("husker.unsent"), false);
});

test("a new task shows at once and returns to the field when the server refuses it", async () => {
  const gate = held();
  const page = await openPage({ answers: { "POST /api/tasks": () => gate.promise } });
  page.$("add-title").value = "Write the long report";
  const sending = page.$("add").fire("submit");
  await settle();
  assert.equal(page.$("add-title").value, "", "the next task can be typed while this one is sent");
  assert.equal(waiting(page).length, 1);
  assert.ok(waiting(page)[0].names.has("pending") && waiting(page)[0].text().includes("Write the long report"));
  await page.$("add").fire("submit");
  assert.equal(sent(page, "POST").length, 1, "an empty field sends nothing");

  gate.release({ status: 400, body: { error: { code: "too_long", message: "Too long" } } });
  await sending;
  await settle();
  assert.equal(waiting(page).length, 0);
  assert.equal(page.$("add-title").value, "Write the long report");
  assert.equal(page.$("toast-text").textContent, "Too long");
  assert.equal(page.storage.has("husker.unsent"), false, "a refused task is not kept");
});

test("a refused task is named when the next title is in the field already", async () => {
  const gate = held();
  const page = await openPage({ answers: { "POST /api/tasks": () => gate.promise } });
  page.$("add-title").value = "Renew passport";
  const sending = page.$("add").fire("submit");
  await settle();
  page.$("add-title").value = "Call A";
  gate.release({ status: 404, body: { error: { code: "unknown_list", message: "That list is gone." } } });
  await sending;
  await settle();
  assert.equal(page.$("add-title").value, "Call A", "the typing is not disturbed");
  assert.equal(page.$("toast-text").textContent, "Renew passport: That list is gone.");
});

test("a new task without an answer stays on screen and is sent again", async () => {
  let reachable = false;
  const page = await openPage({
    answers: { "POST /api/tasks": (request) => (reachable ? addAnswer(request) : offline()) },
  });
  page.$("add-title").value = "Renew passport";
  await page.$("add").fire("submit");
  await settle();
  assert.equal(sent(page, "POST").length, 2, "one repeat covers a lost answer");
  const [failed] = waiting(page);
  assert.ok(failed.text().includes("Renew passport") && failed.text().includes("web_not_sent"), failed.text());
  assert.equal(failed.names.has("pending"), false);
  const id = sent(page, "POST")[0].body.id;

  // The next view that arrives shows that the server answers again. The task goes out with its old id.
  reachable = true;
  await page.$("refresh").fire("click");
  await settle();
  assert.equal(sent(page, "POST").length, 3);
  assert.equal(sent(page, "POST")[2].body.id, id, "the same id, so the server creates one task");
  assert.equal(waiting(page).length, 0);

  // A tap on the row sends it too.
  reachable = false;
  page.$("add-title").value = "Book train";
  await page.$("add").fire("submit");
  await settle();
  reachable = true;
  await waiting(page)[0].fire("click");
  await settle();
  assert.equal(waiting(page).length, 0);
  assert.equal(sent(page, "POST").at(-1).body.title, "Book train");
});

test("a server that cannot save now keeps every waiting task", async () => {
  // Three tasks wait from an earlier visit. The server answers the view but fails each new task.
  const tasks = ["First task", "Second task", "Third task"].map((title, index) => ({ id: `${index}`.repeat(32), title, list_id: "work" }));
  let mode = "storage";
  const page = await openPage({
    storage: { "husker.unsent": tasks },
    answers: {
      "POST /api/tasks": (request) => {
        if (mode === "storage") return { status: 502, body: { error: { code: "storage_unavailable", message: "The storage is away." } } };
        if (mode === "gateway") return { status: 503, body: "not json" };
        if (mode === "other") return { body: { hello: "world" } };
        return addAnswer(request);
      },
    },
  });
  assert.equal(sent(page, "POST").length, 3, "the first view sends the tasks that wait");
  assert.equal(waiting(page).length, 3, "none of them leaves the screen");
  assert.ok(waiting(page).every((entry) => entry.text().includes("web_not_sent")));
  assert.equal(JSON.parse(page.storage.get("husker.unsent")).length, 3, "and none leaves the phone");
  assert.equal(page.$("add-title").value, "", "no title is put into the field");
  assert.equal(page.$("toast-text").textContent, "The storage is away.");

  // An error of a gateway and a strange answer keep them too.
  for (mode of ["gateway", "other"]) {
    await page.$("refresh").fire("click");
    await settle();
    assert.equal(waiting(page).length, 3, mode);
    assert.equal(JSON.parse(page.storage.get("husker.unsent")).length, 3, mode);
  }

  mode = "fine";
  await page.$("refresh").fire("click");
  await settle();
  assert.equal(waiting(page).length, 0);
  assert.equal(page.storage.has("husker.unsent"), false);
});

test("a waiting task that the server has already leaves the screen", async () => {
  const task = { id: "9".repeat(32), title: "Renew passport", list_id: "work" };
  const page = await openPage({
    storage: { "husker.unsent": [task] },
    answers: { "POST /api/tasks": { status: 409, body: { error: { code: "task_exists", message: "exists" } } } },
  });
  assert.equal(waiting(page).length, 0);
  assert.equal(page.storage.has("husker.unsent"), false);
  assert.equal(page.$("add-title").value, "", "the title does not come back, because the server has the task");
  assert.equal(calls(page).at(-1), "GET /api/view", "the view shows the task as it is now");
});

test("a task that was not sent survives a reload and goes out with any answer", async () => {
  // The first visit has no connection for the new task.
  const first = await openPage({ answers: { "POST /api/tasks": offline } });
  first.$("add-title").value = "Renew passport";
  await first.$("add").fire("submit");
  await settle();
  const kept = JSON.parse(first.storage.get("husker.unsent"));
  assert.equal(kept.length, 1);
  assert.equal(kept[0].title, "Renew passport");

  // The browser unloaded the page. The next visit shows the task as not sent and sends it with its old id.
  const posts = [];
  const second = await openPage({
    storage: { "husker.unsent": kept },
    answers: {
      "POST /api/tasks": (request) => {
        posts.push(request.body);
        return addAnswer(request);
      },
    },
  });
  assert.deepEqual(posts, [kept[0]], "the same id, the same title, the same list");
  assert.equal(waiting(second).length, 0);
  assert.equal(second.storage.has("husker.unsent"), false, "the server has the task now");

  // A confirmed change is an answer too: a task that waits goes out without a refresh.
  let reachable = false;
  const third = await openPage({
    answers: {
      "POST /api/tasks": (request) => (reachable ? addAnswer(request) : offline()),
      "PATCH /api/tasks/b": { body: { task: { ...row("b", "Buy milk", "inbox", "Inbox"), status: "done" } } },
    },
  });
  third.$("add-title").value = "Book train";
  await third.$("add").fire("submit");
  await settle();
  assert.equal(waiting(third).length, 1);
  reachable = true;
  await withClass(third, "check")[1].fire("click");
  await settle();
  assert.equal(waiting(third).length, 0);
  assert.equal(sent(third, "POST").at(-1).body.title, "Book train");

  // A second open page of the phone keeps the task of the first when it adds its own.
  const other = { id: "7".repeat(32), title: "From the other page" };
  const fourth = await openPage({ answers: { "POST /api/tasks": offline } });
  fourth.storage.set("husker.unsent", JSON.stringify([other]));
  fourth.$("add-title").value = "From this page";
  await fourth.$("add").fire("submit");
  await settle();
  assert.deepEqual(JSON.parse(fourth.storage.get("husker.unsent")).map((body) => body.title), ["From the other page", "From this page"]);

  // Signing out removes the tasks that wait from the phone.
  fourth.$("sign-out").fire("click");
  await settle();
  assert.equal(fourth.storage.has("husker.unsent"), false);
});

test("the sheet sends presets, the list, and Trash at once", async () => {
  const page = await openPage({ answers: { PATCH: { body: { task: row("c", "Call Ann", "work", "Work"), view: sampleView() } } } });
  await openSheet(page, 2);
  assert.equal(page.$("task-title").value, "Call Ann");
  assert.equal(page.$("sheet").attributes["aria-label"], "Call Ann");
  assert.equal(page.$("sheet-close").textContent, "web_close");

  await page.groups["[data-preset]"][1].fire("click");
  await settle();
  assert.deepEqual(sent(page, "PATCH").at(-1).body, { preset: "tomorrow" });
  assert.equal(page.$("sheet").open, false);

  await openSheet(page, 2);
  page.$("task-list").value = "inbox";
  await page.$("task-list").fire("change");
  await settle();
  assert.deepEqual(sent(page, "PATCH").at(-1).body, { list_id: "inbox" });

  await openSheet(page, 2);
  await page.$("task-trash").fire("click");
  await settle();
  assert.deepEqual(sent(page, "PATCH").at(-1).body, { deleted: true });
  assert.equal(page.$("toast").hidden, false);
  await page.$("toast-undo").onclick();
  await settle();
  assert.deepEqual(sent(page, "PATCH").at(-1).body, { deleted: false });
});

test("Save sends only the changed fields, and the version with text only", async () => {
  const page = await openPage({ answers: { PATCH: { body: { task: row("c", "Call Ann", "work", "Work"), view: sampleView() } } } });
  await openSheet(page, 2);
  page.$("task-title").value = "Call Ann back";
  await page.$("task-title").fire("input");
  assert.equal(page.$("sheet-close").textContent, "web_discard", "closing would drop the new title");
  await page.$("sheet-form").fire("submit");
  await settle();
  assert.deepEqual(sent(page, "PATCH").at(-1).body, { title: "Call Ann back", version: "v-c" });
  assert.equal(page.$("sheet").open, false);

  // The date field is saved with Save. It needs no version, so newer text of another device stays.
  await openSheet(page, 2);
  page.$("task-due").value = "2026-12-24";
  await page.$("task-due").fire("change");
  assert.equal(page.$("sheet-close").textContent, "web_discard");
  assert.equal(sent(page, "PATCH").length, 1, "a date in the field sends nothing by itself");
  await page.$("sheet-form").fire("submit");
  await settle();
  assert.deepEqual(sent(page, "PATCH").at(-1).body, { due: "2026-12-24" });

  // An emptied date field removes the planned date.
  await openSheet(page, 0);
  assert.equal(page.$("task-due").value, "2026-11-10");
  page.$("task-due").value = "";
  await page.$("sheet-form").fire("submit");
  await settle();
  assert.deepEqual(sent(page, "PATCH").at(-1).body, { clear_due: true });

  // Save without a change closes the sheet and sends nothing.
  const before = sent(page, "PATCH").length;
  await openSheet(page, 2);
  await page.$("sheet-form").fire("submit");
  assert.equal(page.$("sheet").open, false);
  assert.equal(sent(page, "PATCH").length, before);
});

test("text that a field shows with other line breaks does not count as changed", async () => {
  // An agent wrote a description with Windows line breaks and a title with a line break.
  const view = viewWith("c", { title: "Call Ann\nand Bob", description: "Line one\r\nLine two" });
  const page = await openPage({
    answers: {
      "GET /api/view": { body: view },
      PATCH: { body: { task: row("c", "Call Ann", "work", "Work"), view } },
    },
  });
  await openSheet(page, 2);
  assert.equal(page.$("task-description").value, "Line one\nLine two", "the field shows other line breaks than the task has");
  assert.equal(page.$("task-title").value, "Call Annand Bob");
  assert.equal(page.$("sheet-close").textContent, "web_close", "nothing was typed, so nothing counts as changed");

  // A button on the untouched sheet sends its action alone, without text and without a version.
  await page.groups["[data-preset]"][0].fire("click");
  await settle();
  assert.deepEqual(sent(page, "PATCH").at(-1).body, { preset: "today" });
  assert.equal(page.$("sheet").open, false);

  // Typing still counts, and only the typed field is sent.
  await openSheet(page, 2);
  page.$("task-description").value = "Line one\nLine two\nLine three";
  await page.$("sheet-form").fire("submit");
  await settle();
  assert.deepEqual(sent(page, "PATCH").at(-1).body, { description: "Line one\nLine two\nLine three", version: "v-c" });
});

test("a button in the sheet sends the typed text with its action", async () => {
  const gate = held();
  const page = await openPage({ answers: { "PATCH /api/tasks/c": () => gate.promise } });
  await openSheet(page, 2);
  page.$("task-title").value = "Call Ann at 15:00";
  page.$("task-due").value = "2026-12-24";
  const sending = page.groups["[data-preset]"][0].fire("click");
  await settle();
  const [change] = sent(page, "PATCH");
  assert.deepEqual(change.body, { title: "Call Ann at 15:00", preset: "today", version: "v-c" }, "the tapped date wins over the date field");
  assert.equal(page.$("sheet").open, true, "the sheet stays open until the server accepted the text");

  gate.release({ body: { task: row("c", "Call Ann at 15:00", "work", "Work"), view: viewWith("c", { title: "Call Ann at 15:00" }) } });
  await sending;
  await settle();
  assert.equal(page.$("sheet").open, false);
  assert.ok(page.$("rows").text().includes("Call Ann at 15:00"));
});

test("while a request of the sheet runs, its fields and its row wait", async () => {
  const gate = held();
  const page = await openPage({ answers: { "PATCH /api/tasks/c": () => gate.promise } });
  await openSheet(page, 2);
  page.$("task-title").value = "Call Ann back";
  const sending = page.groups["[data-preset]"][0].fire("click");
  await settle();
  assert.ok(page.$("sheet-form").names.has("saving"), "the buttons show that they wait");
  assert.equal(page.$("sheet-close").textContent, "web_close", "the changes are on their way, so closing discards nothing");
  for (const id of ["task-title", "task-description", "task-due", "task-list"]) {
    assert.equal(page.$(id).disabled, true, `${id} takes no text that the request would not hold`);
  }
  assert.equal(page.$("task-save").disabled, true);
  assert.equal(pendingRows(page).length, 1, "the row of the task waits too");

  // The checkbox of that row and a second sheet change nothing meanwhile.
  await withClass(page, "check")[2].fire("click");
  assert.equal(sent(page, "PATCH").length, 1);

  gate.release({ status: 502, body: { error: { code: "storage_unavailable", message: "The storage is away." } } });
  await sending;
  await settle();
  assert.equal(page.$("sheet-form").names.has("saving"), false);
  assert.equal(page.$("task-title").disabled, false, "after a failure the fields take text again");
  assert.equal(page.$("task-title").value, "Call Ann back", "and they still hold what was typed");
  assert.equal(pendingRows(page).length, 0);
});

test("Mark done in the sheet keeps the row in place, as the checkbox does", async () => {
  const page = await openPage({
    answers: { "PATCH /api/tasks/c": (request) => ({ body: { task: { ...row("c", "Call Ann", "work", "Work"), status: request.body.status } } }) },
  });
  await openSheet(page, 2);
  await page.$("task-toggle").fire("click");
  await settle();
  const [change] = sent(page, "PATCH");
  assert.deepEqual(change.body, { status: "done" });
  assert.equal(change.query.view, "none");
  assert.equal(page.$("sheet").open, false);
  assert.equal(closedRows(page).length, 1);
  assert.deepEqual(headings(page), ["Earlier (1)", "Today - 2026-11-15 Sun W46 (2)"], "the row stays under its heading");

  // Cancel task keeps the place too.
  await openSheet(page, 1);
  await page.$("task-cancel").fire("click");
  await settle();
  assert.deepEqual(sent(page, "PATCH").at(-1).body, { status: "cancelled" });
  assert.equal(sent(page, "PATCH").at(-1).query.view, "none");
});

test("text refused as stale is rebased, and the next Save keeps the other change", async () => {
  // A computer renamed the task and marked it done after the phone read it.
  const renamed = viewWith("c", { title: "Call Ann at 15:00", status: "done", version: "v-new" });
  let patches = 0;
  const page = await openPage({
    answers: {
      "GET /api/view": () => ({ body: patches === 0 ? sampleView() : renamed }),
      "PATCH /api/tasks/c": () => {
        patches += 1;
        return patches === 1 ? stale : { body: { task: row("c", "Call Ann at 15:00", "work", "Work"), view: renamed } };
      },
    },
  });
  await openSheet(page, 2);
  assert.equal(page.$("task-toggle").textContent, "web_mark_done");
  page.$("task-description").value = "Ask about the invoice.";
  await page.$("sheet-form").fire("submit");
  await settle();
  assert.deepEqual(sent(page, "PATCH")[0].body, { description: "Ask about the invoice.", version: "v-c" });
  assert.equal(page.$("sheet").open, true);
  assert.equal(page.$("sheet-error").textContent, "web_stale");
  assert.equal(page.$("task-title").value, "Call Ann at 15:00", "the field the user did not change shows the new title");
  assert.equal(page.$("task-description").value, "Ask about the invoice.", "the typed text stays");
  assert.equal(page.$("task-toggle").textContent, "action_reopen", "the status button follows the task as it is now");

  await page.$("sheet-form").fire("submit");
  await settle();
  assert.deepEqual(sent(page, "PATCH")[1].body, { description: "Ask about the invoice.", version: "v-new" }, "the title is not sent back");
  assert.equal(page.$("sheet").open, false);
});

test("a button whose request failed goes out again with the next Save", async () => {
  const renamed = viewWith("c", { description: "From the computer.", version: "v-new" });
  let patches = 0;
  let answer = () => stale;
  const page = await openPage({
    answers: {
      "GET /api/view": () => ({ body: patches === 0 ? sampleView() : renamed }),
      "PATCH /api/tasks/c": () => {
        patches += 1;
        return patches === 1 ? answer() : { body: { task: row("c", "Call Ann at 15:00", "work", "Work"), view: renamed } };
      },
      "PATCH /api/tasks/b": { body: { task: row("b", "Buy oat milk", "inbox", "Inbox"), view: renamed } },
    },
  });
  await openSheet(page, 2);
  page.$("task-title").value = "Call Ann at 15:00";
  await page.groups["[data-preset]"][0].fire("click");
  await settle();
  assert.equal(page.$("sheet").open, true);
  assert.equal(page.$("sheet-error").textContent, "web_stale");
  assert.equal(page.$("task-description").value, "From the computer.", "the untouched field shows the other change");

  // The user tapped Today. Save sends that with the title, so the tap is not lost.
  await page.$("sheet-form").fire("submit");
  await settle();
  assert.deepEqual(sent(page, "PATCH")[1].body, { title: "Call Ann at 15:00", preset: "today", version: "v-new" });
  assert.equal(page.$("sheet").open, false);

  // The same holds after a request without an answer, and the picker keeps the list that waits.
  patches = 0;
  answer = offline;
  await openSheet(page, 2);
  page.$("task-title").value = "Call Ann once more";
  page.$("task-list").value = "inbox";
  await page.$("task-list").fire("change");
  await settle();
  assert.equal(page.$("sheet-error").textContent, "web_not_saved");
  assert.equal(page.$("task-list").value, "inbox", "the picker shows the list the next Save moves the task to");
  await page.$("sheet-form").fire("submit");
  await settle();
  const again = sent(page, "PATCH").at(-1).body;
  assert.equal(again.list_id, "inbox");
  assert.equal(again.title, "Call Ann once more");

  // After a stale answer to a move, the picker shows the list that the next Save moves the task to.
  patches = 0;
  answer = () => stale;
  await openSheet(page, 2);
  page.$("task-title").value = "Call Ann from the office";
  page.$("task-list").value = "inbox";
  await page.$("task-list").fire("change");
  await settle();
  assert.equal(page.$("sheet-error").textContent, "web_stale");
  assert.equal(page.$("task-list").value, "inbox", "not the list the task is in now");
  await page.$("sheet-close").fire("click");

  // After a refusal the user can also discard. The button then belongs to no other task:
  // the sheet of another task sends its own fields only.
  patches = 0;
  answer = () => stale;
  await openSheet(page, 2);
  page.$("task-title").value = "Call Ann a third time";
  await page.groups["[data-preset]"][0].fire("click");
  await settle();
  assert.equal(page.$("sheet-error").textContent, "web_stale");
  await page.$("sheet-close").fire("click");
  await openSheet(page, 1);
  page.$("task-title").value = "Buy oat milk";
  await page.$("sheet-form").fire("submit");
  await settle();
  assert.deepEqual(sent(page, "PATCH").at(-1).body, { title: "Buy oat milk", version: "v-b" });
});

test("a newer choice replaces a button that waits for Save", async () => {
  const fresh = viewWith("c", { version: "v-new" });
  let patches = 0;
  const page = await openPage({
    answers: {
      "GET /api/view": () => ({ body: patches === 0 ? sampleView() : fresh }),
      "PATCH /api/tasks/c": () => {
        patches += 1;
        return patches % 2 === 1 ? stale : { body: { task: row("c", "Call Ann", "work", "Work"), view: fresh } };
      },
    },
  });

  // A move was refused. The user picks the list of the task again, so the move is off.
  await openSheet(page, 2);
  page.$("task-title").value = "Call Ann from home";
  page.$("task-list").value = "inbox";
  await page.$("task-list").fire("change");
  await settle();
  assert.equal(page.$("sheet-error").textContent, "web_stale");
  page.$("task-list").value = "work";
  await page.$("task-list").fire("change");
  await page.$("sheet-form").fire("submit");
  await settle();
  assert.deepEqual(sent(page, "PATCH").at(-1).body, { title: "Call Ann from home", version: "v-new" }, "no list in the request");

  // Today was refused. The user then picks a day in the date field, and that day counts.
  await openSheet(page, 2);
  page.$("task-title").value = "Call Ann from the car";
  await page.groups["[data-preset]"][0].fire("click");
  await settle();
  assert.equal(page.$("sheet-error").textContent, "web_stale");
  page.$("task-due").value = "2026-12-24";
  await page.$("task-due").fire("change");
  await page.$("sheet-form").fire("submit");
  await settle();
  assert.deepEqual(sent(page, "PATCH").at(-1).body, { title: "Call Ann from the car", due: "2026-12-24", version: "v-new" });
});

test("a task that left the view ends its sheet, with the text still in sight", async () => {
  // A computer moved the task to Trash while the phone had its sheet open.
  const without = sampleView();
  without.groups[1].rows = without.groups[1].rows.filter((entry) => entry.id !== "c");
  let patches = 0;
  const page = await openPage({
    answers: {
      "GET /api/view": () => ({ body: patches === 0 ? sampleView() : without }),
      "PATCH /api/tasks/c": () => {
        patches += 1;
        return stale;
      },
    },
  });
  await openSheet(page, 2);
  page.$("task-description").value = "Ask about the invoice.";
  await page.$("sheet-form").fire("submit");
  await settle();
  assert.equal(page.$("sheet").open, true);
  assert.equal(page.$("sheet-error").textContent, "web_task_left");
  assert.equal(page.$("task-save").hidden, true, "a Save would be refused again and again");
  assert.equal(page.$("task-trash").hidden, true);
  assert.equal(page.$("task-description").value, "Ask about the invoice.", "the text stays in sight");
  assert.equal(page.$("sheet-close").textContent, "web_discard");

  // A task that the server no longer knows ends its sheet in the same way, with the page's own text.
  const gone = await openPage({
    answers: { "PATCH /api/tasks/c": { status: 404, body: { error: { code: "task_not_found", message: "Task c does not exist." } } } },
  });
  await openSheet(gone, 2);
  gone.$("task-title").value = "Call Ann back";
  await gone.$("sheet-form").fire("submit");
  await settle();
  assert.equal(gone.$("sheet-error").textContent, "web_task_missing");
  assert.equal(gone.$("task-save").hidden, true);
});

test("a save whose answer was lost is not reported as a change from elsewhere", async () => {
  // The server saved the title, but the answer did not arrive. The next view shows the saved title.
  const saved = viewWith("c", { title: "Call Ann back", version: "v-new" });
  let patches = 0;
  const page = await openPage({
    answers: {
      "GET /api/view": () => ({ body: patches === 0 ? sampleView() : saved }),
      "PATCH /api/tasks/c": () => {
        patches += 1;
        return offline();
      },
    },
  });
  await openSheet(page, 2);
  page.$("task-title").value = "Call Ann back";
  const sending = page.$("sheet-form").fire("submit");
  await settle();
  await sending;
  assert.equal(page.$("sheet").open, true);
  assert.equal(page.$("sheet-error").textContent, "web_not_saved");
  assert.equal(page.$("sheet-close").textContent, "web_close", "the sheet took the saved task, so nothing is left to discard");

  // Save finds nothing to send and closes the sheet. No message about a change from elsewhere.
  await page.$("sheet-form").fire("submit");
  await settle();
  assert.equal(patches, 1);
  assert.equal(page.$("sheet").open, false);
});

test("a task reopened in History leaves the list, by checkbox and by button", async () => {
  const closed = (ids) => ({
    ...sampleView(),
    view: "history",
    groups: [{ key: "2026-11-14", label: "2026-11-14 Sat W46", count: ids.length, rows: ids.map((id) => row(id, `Closed ${id}`, "work", "Work", { status: "done" })) }],
  });
  let left = ["x", "y", "z"];
  const page = await openPage({
    answers: {
      "GET /api/view": (request) => ({ body: request.query.view === "history" ? closed(left) : sampleView() }),
      PATCH: (request) => {
        left = left.filter((id) => `/api/tasks/${id}` !== request.path);
        return { body: { task: row("x", "Closed", "work", "Work"), view: closed(left) } };
      },
    },
  });
  await page.groups["[data-view]"][1].fire("click");
  await settle();
  assert.equal(withClass(page, "check").length, 3);

  await withClass(page, "check")[0].fire("click");
  await settle();
  const byCheckbox = sent(page, "PATCH").at(-1);
  assert.deepEqual(byCheckbox.body, { status: "open" });
  assert.notEqual(byCheckbox.query.view, "none", "History needs the view after the change");
  assert.equal(page.$("rows").text().includes("Closed x"), false, "the reopened task left History");

  await openSheet(page, 0);
  await page.$("task-toggle").fire("click");
  await settle();
  assert.equal(page.$("rows").text().includes("Closed y"), false);
  assert.ok(page.$("rows").text().includes("Closed z"));
});

test("a save that fails after the sheet was closed names the task in a message", async () => {
  const gate = held();
  const page = await openPage({ answers: { "PATCH /api/tasks/c": () => gate.promise } });
  await openSheet(page, 2);
  page.$("task-title").value = "Call Ann back";
  const sending = page.$("sheet-form").fire("submit");
  await settle();
  // The close button works while the request runs, so a slow connection cannot lock the sheet.
  await page.$("sheet-close").fire("click");
  assert.equal(page.$("sheet").open, false);
  await openSheet(page, 1);

  gate.release({ status: 502, body: { error: { code: "storage_unavailable", message: "The storage is away." } } });
  await sending;
  await settle();
  assert.equal(page.$("toast-text").textContent, "Call Ann: The storage is away.");
  assert.equal(page.$("sheet-error").hidden, true, "the sheet of another task shows no error");
  assert.equal(page.$("sheet").open, true, "the other sheet stays open");
});

test("Save sends the text once, however often it is tapped", async () => {
  const gate = held();
  const page = await openPage({ answers: { "PATCH /api/tasks/c": () => gate.promise } });
  await openSheet(page, 2);
  page.$("task-title").value = "Call Ann back";
  const first = page.$("sheet-form").fire("submit");
  const second = page.$("sheet-form").fire("submit");
  await page.groups["[data-preset]"][0].fire("click");
  await settle();
  assert.equal(sent(page, "PATCH").length, 1);
  assert.equal(page.$("task-save").disabled, true);
  gate.release({ body: { task: row("c", "Call Ann back", "work", "Work"), view: sampleView() } });
  await Promise.all([first, second]);
  await settle();
  assert.equal(page.$("sheet").open, false);
  assert.equal(page.$("task-save").disabled, false);
});

test("a code in the address pairs the phone and leaves the address", async () => {
  const page = await openPage({ hash: "#code=k7qf-2m9x-tb4h", answers: { "POST /api/session": paired } });
  assert.deepEqual(calls(page), ["GET /api/session", "POST /api/session", "GET /api/view"]);
  assert.deepEqual(sent(page, "POST")[0].body, { code: "K7QF2M9XTB4H" });
  assert.equal(globalThis.location.hash, "");
  assert.equal(page.confirms.length, 0, "a phone without a pairing is not asked");
  assert.equal(page.$("app").hidden, false);
  assert.equal(page.$("pairing").hidden, true);
});

test("a pairing link asks before it replaces a working pairing", async () => {
  const declined = await openPage({
    hash: "#code=k7qf-2m9x-tb4h",
    kept: sampleView(),
    confirmAnswer: false,
    answers: { "GET /api/session": paired },
  });
  assert.deepEqual(declined.confirms, ["web_pair_replace"]);
  assert.equal(sent(declined, "POST").length, 0, "the code is not used");
  assert.equal(globalThis.location.hash, "");
  assert.ok(declined.$("rows").text().includes("Buy milk"));

  // The server says whether the phone is paired. A paired phone without kept tasks is asked too.
  const bare = await openPage({ hash: "#code=k7qf-2m9x-tb4h", confirmAnswer: false, answers: { "GET /api/session": paired } });
  assert.deepEqual(bare.confirms, ["web_pair_replace"]);
  assert.equal(sent(bare, "POST").length, 0);

  const accepted = await openPage({
    hash: "#code=k7qf-2m9x-tb4h",
    kept: sampleView(),
    answers: { "GET /api/session": paired, "POST /api/session": paired },
  });
  assert.deepEqual(sent(accepted, "POST")[0].body, { code: "K7QF2M9XTB4H" });
});

test("a link without an answer from the server keeps its code for later", async () => {
  const page = await openPage({ hash: "#code=k7qf-2m9x-tb4h", answers: { "GET /api/session": offline } });
  assert.equal(page.$("pairing").hidden, false);
  assert.equal(page.$("pair-error").textContent, "web_offline");
  assert.equal(page.$("pair-code").value, "K7QF2M9XTB4H", "the code waits in the field");
  assert.equal(sent(page, "POST").length, 0);

  // With kept tasks the phone keeps showing them.
  const kept = await openPage({ hash: "#code=k7qf-2m9x-tb4h", kept: sampleView(), answers: { "GET /api/session": offline } });
  assert.equal(kept.$("app").hidden, false);
  assert.ok(kept.$("rows").text().includes("Buy milk"));
});

test("a refused code leaves a paired phone paired", async () => {
  const refused = { status: 404, body: { error: { code: "unknown_code", message: "unknown" } } };
  const page = await openPage({
    hash: "#code=k7qf-2m9x-tb4h",
    kept: sampleView(),
    answers: { "GET /api/session": paired, "POST /api/session": refused },
  });
  assert.equal(page.$("app").hidden, false, "the cookie of the phone still works");
  assert.equal(page.$("pairing").hidden, true);
  assert.equal(page.$("toast-text").textContent, "web_pair_failed");
  assert.ok(page.storage.has("husker.view"), "the kept tasks stay");

  // Without a pairing, the same refusal shows the pairing screen with the message.
  const fresh = await openPage({
    hash: "#code=k7qf-2m9x-tb4h",
    answers: { "POST /api/session": refused, "GET /api/view": unauthorized },
  });
  assert.equal(fresh.$("pairing").hidden, false);
  assert.equal(fresh.$("app").hidden, true);
  assert.equal(fresh.$("pair-error").textContent, "web_pair_failed");
});

test("without a session the page shows the pairing screen and forgets the kept view", async () => {
  const page = await openPage({ kept: sampleView(), answers: { "GET /api/view": unauthorized } });
  assert.equal(page.$("pairing").hidden, false);
  assert.equal(page.$("app").hidden, true);
  assert.equal(page.storage.has("husker.view"), false);
  assert.equal(page.$("pair-error").textContent, "web_unauthorized", "a phone that was paired is told");
  assert.equal(page.$("rows").children.length, 0, "the hidden page holds no task");

  page.$("pair-code").value = "not a code";
  await page.$("pair-form").fire("submit");
  assert.equal(page.$("pair-error").hidden, false);
  assert.equal(sent(page, "POST").length, 0);
});

test("the kept view shows at once, and Danish follows the browser", async () => {
  const kept = sampleView();
  kept.groups[1].rows[0].title = "Kept title";
  const gate = held();
  const page = await openPage({ kept, language: "da-DK", answers: { "GET /api/view": () => gate.promise } });
  assert.equal(page.$("app").hidden, false);
  assert.ok(page.$("rows").text().includes("Kept title"), "the kept view is on screen before the server answers");
  assert.equal(apiRequests(page)[0].query.lang, "da");
  assert.equal(globalThis.document.documentElement.lang, "da");
  gate.release({ body: sampleView() });
  await settle();
  assert.ok(page.$("rows").text().includes("Buy milk"), "the current view replaced the kept one");
  assert.equal(page.$("rows").text().includes("Kept title"), false);
});

test("a kept view that the page cannot draw is dropped", async () => {
  const page = await openPage({ kept: { view: "all" } });
  assert.equal(page.$("app").hidden, false);
  assert.ok(page.$("rows").text().includes("Buy milk"), "the page asked the server and drew its answer");
});

test("an answer without the shape of a view leaves the screen as it is", async () => {
  const answers = [() => ({ body: sampleView() }), () => ({ body: { hello: "world" } })];
  const page = await openPage({
    answers: {
      "GET /api/view": () => answers.shift()(),
      // The answer to a change with something else in place of the view.
      "PATCH /api/tasks/c": { body: { task: row("c", "Call Ann", "work", "Work"), view: "all" } },
    },
  });
  await page.$("refresh").fire("click");
  await settle();
  assert.ok(page.$("rows").text().includes("Buy milk"));
  assert.equal(JSON.parse(page.storage.get("husker.view")).groups.length, 2, "the kept view is not replaced");

  await openSheet(page, 2);
  await page.groups["[data-preset]"][0].fire("click");
  await settle();
  assert.ok(page.$("rows").text().includes("Buy milk"), "the rows stay on screen");
});

test("a read-only access hides the controls, and a lost connection is still named", async () => {
  const view = { ...sampleView(), write: false };
  let reachable = true;
  const page = await openPage({ answers: { "GET /api/view": () => (reachable ? { body: view } : offline()) } });
  assert.equal(page.$("add").hidden, true);
  assert.ok(withClass(page, "check").every((check) => check.disabled));
  assert.equal(page.$("notice").hidden, false);
  assert.equal(page.$("notice").textContent, "web_read_only");
  reachable = false;
  await page.$("refresh").fire("click");
  await settle();
  assert.equal(page.$("notice").textContent, "web_offline web_saved_view");
});

// The server can take seconds to start, so answers arrive late and out of order.

test("the first opening shows a loading line until the server answers", async () => {
  const gate = held();
  const page = await openPage({ answers: { "GET /api/view": () => gate.promise } });
  assert.equal(page.$("app").hidden, false, "the task screen shows while the server starts");
  assert.equal(page.$("add").hidden, true);
  assert.ok(page.$("rows").text().includes("web_loading"));
  assert.ok(page.$("refresh").names.has("busy"));
  gate.release({ body: sampleView() });
  await settle();
  assert.ok(page.$("rows").text().includes("Buy milk"));
  assert.equal(page.$("refresh").names.has("busy"), false);
});

test("a server that cannot be reached gets a message, with and without kept tasks", async () => {
  const empty = await openPage({ answers: { "GET /api/view": offline } });
  assert.equal(empty.$("app").hidden, false);
  assert.equal(empty.$("notice").hidden, false);
  assert.equal(empty.$("notice").textContent, "web_offline");
  assert.equal(empty.$("rows").children.length, 0);

  const kept = await openPage({ kept: sampleView(), answers: { "GET /api/view": offline } });
  assert.equal(kept.$("notice").textContent, "web_offline web_saved_view");
  assert.ok(kept.$("rows").text().includes("Buy milk"), "the kept tasks stay on screen");

  // When the phone has a connection again, the page asks at once.
  const before = apiRequests(kept).length;
  await kept.window.fire("online");
  await settle();
  assert.equal(apiRequests(kept).length, before + 1);
});

test("a refresh that answers during a change keeps the row as the user set it", async () => {
  const gate = held();
  const renamed = viewWith("c", { title: "Call Ann tomorrow" });
  const views = [sampleView(), renamed];
  const page = await openPage({
    answers: {
      "GET /api/view": () => ({ body: views.shift() ?? renamed }),
      "PATCH /api/tasks/b": () => gate.promise,
    },
  });
  const sending = withClass(page, "check")[1].fire("click");
  await settle();
  await page.$("refresh").fire("click");
  await settle();
  assert.ok(page.$("rows").text().includes("Call Ann tomorrow"), "the refresh arrived");
  assert.equal(closedRows(page).length, 1, "the tapped row still shows as closed");

  // The change fails. Only its own row goes back, and the refreshed row stays.
  gate.release({ status: 502, body: { error: { code: "storage_unavailable", message: "down" } } });
  await sending;
  await settle();
  assert.equal(closedRows(page).length, 0);
  assert.ok(page.$("rows").text().includes("Call Ann tomorrow"));
  assert.equal(page.$("toast-text").textContent, "down");
});

test("an older answer does not replace a newer view", async () => {
  const gate = held();
  const older = viewWith("b", { title: "Older answer" });
  const newer = viewWith("b", { title: "Newer answer" });
  const answers = [() => ({ body: sampleView() }), () => gate.promise, () => ({ body: newer })];
  const page = await openPage({ answers: { "GET /api/view": () => answers.shift()() } });
  const slow = page.$("refresh").fire("click");
  await settle();
  await page.$("refresh").fire("click");
  await settle();
  assert.ok(page.$("rows").text().includes("Newer answer"));
  gate.release({ body: older });
  await slow;
  await settle();
  assert.ok(page.$("rows").text().includes("Newer answer"), "the late answer is dropped");
  assert.equal(page.$("refresh").names.has("busy"), false);
});

test("a view that was read before a confirmed change is not shown", async () => {
  // The refresh is sent first. The server reads the file for it before the tap is saved.
  const gate = held();
  const done = viewWith("b", { status: "done" });
  const answers = [() => ({ body: sampleView() }), () => gate.promise, () => ({ body: done })];
  const page = await openPage({
    answers: {
      "GET /api/view": () => answers.shift()(),
      "PATCH /api/tasks/b": { body: { task: { ...row("b", "Buy milk", "inbox", "Inbox"), status: "done" } } },
    },
  });
  const slow = page.$("refresh").fire("click");
  await settle();
  await withClass(page, "check")[1].fire("click");
  await settle();
  assert.equal(closedRows(page).length, 1);

  gate.release({ body: sampleView() });
  await slow;
  await settle();
  assert.equal(closedRows(page).length, 1, "the old view does not open the task again");
  assert.equal(sent(page, "GET").length, 3, "the page asked once more for a view that is newer than the change");
  assert.equal(JSON.parse(page.storage.get("husker.view")).groups[1].rows[0].status, "done", "the kept view has the change");
});

test("the second of two overlapping changes is settled by a fresh view", async () => {
  const first = held();
  const second = held();
  const both = viewWith("b", { title: "Both changes" });
  let views = 0;
  const page = await openPage({
    answers: {
      "GET /api/view": () => ({ body: (views += 1) > 1 ? both : sampleView() }),
      "PATCH /api/tasks/b": () => first.promise,
      "PATCH /api/tasks/c": () => second.promise,
    },
  });
  await openSheet(page, 1);
  const one = page.groups["[data-preset]"][1].fire("click");
  await settle();
  await openSheet(page, 2);
  const two = page.groups["[data-preset]"][1].fire("click");
  await settle();

  // The answers cannot tell which change the server saved last, so the second view is not trusted.
  first.release({ body: { task: row("b", "Buy milk", "inbox", "Inbox"), view: viewWith("b", { title: "Only the first" }) } });
  await one;
  await settle();
  assert.ok(page.$("rows").text().includes("Only the first"));
  second.release({ body: { task: row("c", "Call Ann", "work", "Work"), view: viewWith("c", { title: "Only the second" }) } });
  await two;
  await settle();
  assert.equal(page.$("rows").text().includes("Only the second"), false);
  assert.ok(page.$("rows").text().includes("Both changes"), "a fresh view shows both");
});

test("two overlapping taps on checkboxes are settled by a fresh view too", async () => {
  const first = held();
  const second = held();
  const page = await openPage({
    answers: {
      "PATCH /api/tasks/b": () => first.promise,
      "PATCH /api/tasks/c": () => second.promise,
    },
  });
  const one = withClass(page, "check")[1].fire("click");
  const two = withClass(page, "check")[2].fire("click");
  await settle();
  const before = sent(page, "GET").length;
  first.release({ body: { task: { ...row("b", "Buy milk", "inbox", "Inbox"), status: "done" } } });
  await one;
  await settle();
  assert.equal(sent(page, "GET").length, before, "one answer alone is current");
  second.release({ body: { task: { ...row("c", "Call Ann", "work", "Work"), status: "done" } } });
  await two;
  await settle();
  assert.equal(sent(page, "GET").length, before + 1, "the second answer cannot say which change was saved last");
});

test("the answer to a change is not shown in a view the user has moved to", async () => {
  const change = held();
  const firstHistory = held();
  const history = { ...sampleView(), view: "history", groups: [{ key: "2026-11-14", label: "2026-11-14 Sat W46", count: 1, rows: [row("d", "Done thing", "work", "Work", { status: "done" })] }] };
  let historyRequests = 0;
  const page = await openPage({
    answers: {
      "GET /api/view": (request) => {
        if (request.query.view !== "history") return { body: sampleView() };
        historyRequests += 1;
        return historyRequests === 1 ? firstHistory.promise : { body: history };
      },
      "PATCH /api/tasks/c": () => change.promise,
    },
  });
  await openSheet(page, 2);
  const sending = page.$("task-trash").fire("click");
  await settle();
  const moving = page.groups["[data-view]"][1].fire("click");
  await settle();

  // The change answers first, with the All tasks view it was sent from. History is still loading.
  change.release({ body: { task: row("c", "Call Ann", "work", "Work", { deleted: true }), view: sampleView() } });
  await sending;
  await settle();
  assert.equal(page.$("rows").text().includes("Overdue report"), false, "the All tasks answer is not drawn under History");
  assert.equal(historyRequests, 2, "History is requested again after the change");
  assert.ok(page.$("rows").text().includes("Done thing"));

  firstHistory.release({ body: { ...history, groups: [] } });
  await moving;
  await settle();
  assert.ok(page.$("rows").text().includes("Done thing"), "the older History answer does not replace the newer one");
});

test("History asks for as many rows as the screen holds", async () => {
  const rows = (from, count) => Array.from({ length: count }, (_, index) => row(`h${from + index}`, `Closed ${from + index}`, "work", "Work", { status: "done" }));
  const page = await openPage({
    answers: {
      "GET /api/view": (request) => {
        if (request.query.view !== "history") return { body: sampleView() };
        const offset = Number(request.query.offset ?? 0);
        const limit = Number(request.query.limit ?? 100);
        const count = Math.min(limit, 130 - offset);
        return {
          body: {
            ...sampleView(),
            view: "history",
            total: 130,
            next_offset: offset + count < 130 ? offset + count : null,
            groups: [{ key: "2026-11-14", label: "2026-11-14 Sat W46", count: 130, rows: rows(offset, count) }],
          },
        };
      },
    },
  });
  await page.groups["[data-view]"][1].fire("click");
  await settle();
  assert.equal(withClass(page, "body").length, 100);
  assert.deepEqual(headings(page), ["2026-11-14 Sat W46 (130)"], "the heading counts the whole day");
  assert.equal(page.$("more").hidden, false);

  await page.$("more").fire("click");
  await settle();
  assert.equal(withClass(page, "body").length, 130);
  assert.deepEqual(headings(page), ["2026-11-14 Sat W46 (130)"]);
  assert.equal(page.$("more").hidden, true);

  // A refresh keeps the rows that Show more loaded.
  await page.$("refresh").fire("click");
  await settle();
  assert.equal(sent(page, "GET").at(-1).query.limit, "130");
  assert.equal(withClass(page, "body").length, 130);
});

test("a list chosen for a new task survives a refresh, and the filter replaces it", async () => {
  const page = await openPage();
  const picker = page.$("add-list");
  picker.value = "inbox";
  await picker.fire("change");
  await page.$("refresh").fire("click");
  await settle();
  assert.equal(picker.value, "inbox", "the refresh keeps the chosen list");
  page.$("filter").value = "work";
  await page.$("filter").fire("change");
  assert.equal(picker.value, "work", "a new filter sets the list again");
});

test("a row with a change under way cannot be tapped again or opened", async () => {
  const gate = held();
  const page = await openPage({ answers: { "PATCH /api/tasks/b": () => gate.promise } });
  const sending = withClass(page, "check")[1].fire("click");
  await settle();
  await withClass(page, "check")[1].fire("click");
  await withClass(page, "body")[1].fire("click");
  assert.equal(sent(page, "PATCH").length, 1);
  assert.equal(page.$("sheet").open, false);
  gate.release({ body: { task: { ...row("b", "Buy milk", "inbox", "Inbox"), status: "done" } } });
  await sending;
});

test("a pairing link opened in the open page pairs it", async () => {
  let isPaired = false;
  const page = await openPage({
    answers: {
      "GET /api/view": () => (isPaired ? { body: sampleView() } : unauthorized),
      "POST /api/session": () => {
        isPaired = true;
        return paired;
      },
    },
  });
  assert.equal(page.$("pairing").hidden, false);
  globalThis.location.hash = "#code=k7qf-2m9x-tb4h";
  await page.window.fire("hashchange");
  await settle();
  assert.deepEqual(sent(page, "POST")[0].body, { code: "K7QF2M9XTB4H" });
  assert.equal(globalThis.location.hash, "");
  assert.equal(page.$("pairing").hidden, true);
  assert.equal(page.$("app").hidden, false);
  assert.ok(page.$("rows").text().includes("Buy milk"));

  // A typed code that got no answer stays in the field for the next try.
  const away = await openPage({ answers: { "GET /api/view": unauthorized, "POST /api/session": offline } });
  away.$("pair-code").value = "k7qf-2m9x-tb4h";
  await away.$("pair-form").fire("submit");
  await settle();
  assert.equal(away.$("pairing").hidden, false);
  assert.equal(away.$("pair-error").textContent, "web_offline");
  assert.equal(away.$("pair-code").value, "K7QF2M9XTB4H");

  // A second tap on Pair while a code is checked sends no second request.
  const gate = held();
  const slow = await openPage({ answers: { "GET /api/view": unauthorized, "POST /api/session": () => gate.promise } });
  slow.$("pair-code").value = "k7qf-2m9x-tb4h";
  slow.$("pair-form").fire("submit");
  slow.$("pair-form").fire("submit");
  await settle();
  assert.equal(sent(slow, "POST").length, 1);
});

test("signing out leaves nothing of the tasks, and old answers belong to nobody", async () => {
  const gate = held();
  const change = held();
  const answers = [() => ({ body: sampleView() }), () => gate.promise];
  const page = await openPage({
    answers: {
      "GET /api/view": () => answers.shift()?.() ?? { body: viewWith("b", { title: "The new session" }) },
      "PATCH /api/tasks/b": () => change.promise,
      "PATCH /api/tasks/a": { status: 502, body: { error: { code: "storage_unavailable", message: "Overdue report was not saved." } } },
      "DELETE /api/session": { status: 204 },
      "POST /api/session": paired,
    },
  });
  // A message that names a task is on screen.
  await withClass(page, "check")[0].fire("click");
  await settle();
  assert.equal(page.$("toast-text").textContent, "Overdue report was not saved.");
  // A refresh and a change are under way when the user signs out.
  page.$("refresh").fire("click");
  await settle();
  await openSheet(page, 1);
  page.$("task-title").value = "Half a thought";
  page.$("sheet-form").fire("submit");
  page.$("add-title").value = "Not sent yet";
  await settle();
  page.$("sign-out").fire("click");
  await settle();
  assert.equal(page.$("pairing").hidden, false);
  assert.equal(page.$("app").hidden, true);
  assert.equal(page.$("pair-error").hidden, true);
  assert.equal(page.storage.has("husker.view"), false);
  assert.equal(page.storage.has("husker.signout"), false, "the server removed the cookie");
  assert.equal(page.$("rows").children.length, 0, "the hidden page holds no rows");
  for (const id of ["task-title", "task-description", "add-title"]) assert.equal(page.$(id).value, "", id);
  for (const id of ["filter", "add-list", "task-list"]) assert.equal(page.$(id).children.length, 0, `${id} names no list`);
  assert.equal(page.$("session").textContent, "");
  assert.equal(page.$("toast-text").textContent, "", "the last message could name a task");
  assert.equal(page.$("sheet").attributes["aria-label"], "", "the sheet is named after no task");
  assert.equal(page.$("sheet").open, false);
  assert.equal(page.$("task-save").disabled, false, "nothing waits for the old request");

  // Code that asks for the tasks again sends nothing while the pairing screen is up.
  const before = apiRequests(page).length;
  page.$("refresh").fire("click");
  await settle();
  assert.equal(apiRequests(page).length, before);

  // A new pairing begins. The answers of the session before arrive now and are dropped.
  page.$("pair-code").value = "k7qf-2m9x-tb4h";
  page.$("pair-form").fire("submit");
  await settle();
  assert.ok(page.$("rows").text().includes("The new session"));
  gate.release({ body: viewWith("b", { title: "The old session" }) });
  change.release({ status: 401, body: { error: { code: "unauthorized", message: "no" } } });
  await settle();
  assert.ok(page.$("rows").text().includes("The new session"), "the old view is not drawn");
  assert.equal(page.$("rows").text().includes("The old session"), false);
  assert.equal(page.$("app").hidden, false, "the old refusal does not end the new pairing");
  assert.equal(JSON.parse(page.storage.get("husker.view")).groups[1].rows[0].title, "The new session");
  assert.equal(page.$("toast").hidden, true, "no message about a change of the old session");
});

test("a refusal for the pairing before does not end a new pairing", async () => {
  // A paired phone has a refresh under way. Its access token was revoked, so that refresh will answer 401.
  const gate = held();
  const answers = [() => ({ body: sampleView() }), () => gate.promise];
  const page = await openPage({
    answers: {
      "GET /api/view": () => answers.shift()?.() ?? { body: viewWith("b", { title: "The new pairing" }) },
      "GET /api/session": paired,
      "POST /api/session": paired,
    },
  });
  page.$("refresh").fire("click");
  await settle();

  // The user opens a new pairing link and agrees. The new pairing shows its tasks.
  globalThis.location.hash = "#code=k7qf-2m9x-tb4h";
  page.window.fire("hashchange");
  await settle();
  assert.deepEqual(page.confirms, ["web_pair_replace"]);
  assert.ok(page.$("rows").text().includes("The new pairing"));

  // The refusal for the old pairing arrives now. It belongs to nobody.
  gate.release(unauthorized);
  await settle();
  assert.equal(page.$("app").hidden, false);
  assert.equal(page.$("pairing").hidden, true);
  assert.ok(page.storage.has("husker.view"));
  assert.equal(page.$("refresh").names.has("busy"), false, "nothing waits for the old request");
});

test("a new pairing opens on All tasks, whatever view was open before", async () => {
  const page = await openPage({ answers: { "DELETE /api/session": { status: 204 }, "POST /api/session": paired } });
  await page.groups["[data-view]"][1].fire("click");
  await settle();
  assert.equal(sent(page, "GET").at(-1).query.view, "history");
  page.$("sign-out").fire("click");
  await settle();
  page.$("pair-code").value = "k7qf-2m9x-tb4h";
  page.$("pair-form").fire("submit");
  await settle();
  assert.equal(sent(page, "GET").at(-1).query.view, "all");
  assert.equal(page.groups["[data-view]"][0].attributes["aria-current"], "true", "the menu marks All tasks");
});

test("a sign-out without a connection finishes at the next start, before any task shows", async () => {
  const page = await openPage({ answers: { "DELETE /api/session": offline } });
  page.$("sign-out").fire("click");
  await settle();
  assert.equal(page.$("pairing").hidden, false, "the tasks leave the screen at once");
  assert.equal(page.storage.has("husker.view"), false);
  assert.equal(page.$("pair-error").textContent, "web_sign_out_later");
  assert.equal(page.storage.get("husker.signout"), "true", "the page remembers the unfinished sign-out");

  // Still no connection at the next start: no task is requested, although the cookie still works.
  const still = await openPage({ storage: { "husker.signout": true }, answers: { "DELETE /api/session": offline } });
  assert.deepEqual(calls(still), ["DELETE /api/session"]);
  assert.equal(still.$("pairing").hidden, false);
  assert.equal(still.$("pair-error").textContent, "web_sign_out_later");

  // A typed code waits for the sign-out too, so the old cookie cannot come back.
  still.$("pair-code").value = "k7qf-2m9x-tb4h";
  await still.$("pair-form").fire("submit");
  await settle();
  assert.equal(sent(still, "POST").length, 0);

  // With a connection the sign-out finishes first. A code in the address is used after it, without a question.
  const online = await openPage({
    hash: "#code=k7qf-2m9x-tb4h",
    storage: { "husker.signout": true },
    answers: { "DELETE /api/session": { status: 204 }, "GET /api/session": paired, "POST /api/session": paired },
  });
  assert.deepEqual(calls(online).slice(0, 2), ["DELETE /api/session", "POST /api/session"]);
  assert.equal(online.confirms.length, 0);
  assert.equal(online.storage.has("husker.signout"), false);
  assert.equal(online.$("app").hidden, false);

  // One request removes the cookie. A code typed while it runs waits for it and sends no second one.
  const gate = held();
  const slow = await openPage({ answers: { "DELETE /api/session": () => gate.promise, "POST /api/session": paired } });
  slow.$("sign-out").fire("click");
  await settle();
  slow.$("pair-code").value = "k7qf-2m9x-tb4h";
  slow.$("pair-form").fire("submit");
  await settle();
  assert.equal(sent(slow, "DELETE").length, 1);
  assert.equal(sent(slow, "POST").length, 0, "the new cookie is not asked for before the old one is gone");
  gate.release({ status: 204 });
  await settle();
  assert.equal(sent(slow, "POST").length, 1);
  assert.equal(slow.$("app").hidden, false);
});

test("a new release starts the page again when nothing is typed or under way", async () => {
  const workers = new Element("workers");
  workers.controller = {};
  workers.register = async () => ({});
  workers.getRegistration = async () => ({ update: () => (workers.updates = (workers.updates ?? 0) + 1) });
  const page = await openPage({ workers, answers: { PATCH: { body: { task: row("c", "Call Ann", "work", "Work"), view: sampleView() } } } });
  page.$("add-title").value = "Half a title";
  await workers.fire("controllerchange");
  assert.equal(page.reloads, 0, "typed text is not dropped");
  page.$("add-title").value = "";

  // The Undo message after Trash is something under way too.
  await openSheet(page, 2);
  await page.$("task-trash").fire("click");
  await settle();
  assert.equal(page.$("toast-undo").hidden, false);
  assert.equal(page.reloads, 0, "Undo stays in reach");
  page.$("toast").hidden = true;

  await openSheet(page, 1);
  page.$("sheet").close();
  await page.$("sheet").fire("close");
  assert.equal(page.reloads, 1, "the page starts again at the next quiet moment");

  // A page that comes back to the screen asks for a new release.
  await page.visibility.fire("visibilitychange");
  await settle();
  assert.equal(workers.updates, 1);

  // The first worker of a phone takes over without a new release.
  const first = new Element("workers");
  first.controller = null;
  first.register = async () => ({});
  const fresh = await openPage({ workers: first });
  await first.fire("controllerchange");
  assert.equal(fresh.reloads, 0);
  await first.fire("controllerchange");
  assert.equal(fresh.reloads, 1, "a later change of the worker is a release");

  // A pairing that is being checked is not interrupted: the code would be used up without a cookie.
  const gate = held();
  const pairing = new Element("workers");
  pairing.controller = {};
  pairing.register = async () => ({});
  const busy = await openPage({ workers: pairing, answers: { "GET /api/view": unauthorized, "POST /api/session": () => gate.promise } });
  busy.$("pair-code").value = "k7qf-2m9x-tb4h";
  busy.$("pair-form").fire("submit");
  await settle();
  busy.$("pair-code").value = "";
  await pairing.fire("controllerchange");
  assert.equal(busy.reloads, 0);
});

test("the height of the keyboard lifts the sheets", async () => {
  const viewport = new Element("viewport");
  Object.assign(viewport, { height: 800, offsetTop: 0, scale: 1 });
  await openPage({ viewport });
  const styles = globalThis.document.documentElement.styles;
  viewport.height = 480;
  await viewport.fire("resize");
  assert.equal(styles["--keyboard"], "320px");
  viewport.height = 800;
  await viewport.fire("resize");
  assert.equal(styles["--keyboard"], "0px");
  // A zoomed page shows less too, without a keyboard.
  Object.assign(viewport, { height: 400, scale: 2 });
  await viewport.fire("resize");
  assert.equal(styles["--keyboard"], "0px");
});
