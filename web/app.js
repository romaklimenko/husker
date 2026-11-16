// The phone page of Husker. The server sends ready groups and labels, so this
// file only draws them and sends changes. Task text reaches the document
// through textContent only.

import {
  appendPage,
  drawable,
  filterGroups,
  findRow,
  localDate,
  newId,
  patchRow,
  pickLanguage,
  readCode,
  recount,
  rowCount,
} from "./logic.js";

const VIEW_KEY = "husker.view";
const LANGUAGE_KEY = "husker.language";
const STRINGS_KEY = "husker.strings";
const SIGN_OUT_KEY = "husker.signout";
const UNSENT_KEY = "husker.unsent";

// A request without an answer after this time counts as unanswered. A server that
// has to start answers within a few seconds.
const TIMEOUT = 30000;

const state = {
  strings: null,
  language: "en",
  kind: "all",
  view: null,
  filter: "",
  // The list the user chose for a new task. Empty follows the filter or the server's suggestion.
  addList: "",
  // Changes under way: task id to the values the row shows before the answer arrives.
  pending: new Map(),
  // New tasks on their way to the server, and those that got no answer. They are kept on the phone too.
  adding: [],
  // The task of the sheet, as it was when the sheet opened or when the server last refused its text.
  sheetTask: null,
  // What the fields of the sheet showed for that task. A browser changes line breaks
  // when a field takes text, so a field is compared with what it showed, not with the task.
  sheetShown: null,
  sheetEditable: false,
  // The action of a button whose request failed while the sheet stayed open. The next Save sends it again.
  sheetRetry: null,
  toastTimer: 0,
  // Requests that return a view are numbered in sending order. `shown` is the one on screen.
  sent: 0,
  shown: 0,
  // The highest request number when the server last confirmed a change.
  floor: 0,
  // Requests for the view that are under way.
  loading: 0,
  saving: false,
  // A pairing code is being checked.
  pairing: false,
  // Counts the sessions of this page: it grows at a sign-out, a lost session, and a new pairing.
  // An answer to a request of an earlier count belongs to nobody and is dropped.
  epoch: 0,
  // What the list pickers hold, so they are rebuilt only when it changes.
  pickers: "",
  // What is wrong with the connection, as text for the line above the rows.
  problem: null,
  // A new release of the page waits for a quiet moment to start.
  release: false,
};

const $ = (id) => document.getElementById(id);

function t(key) {
  return state.strings?.[state.language]?.[key] ?? key;
}

function store(key, value) {
  try {
    if (value === null) localStorage.removeItem(key);
    else localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // Storage can be full or switched off. The page works without it.
  }
}

function stored(key) {
  try {
    return JSON.parse(localStorage.getItem(key));
  } catch {
    return null;
  }
}

/** A promise that never settles: whoever waits for the answer of an ended session stops there. */
const abandoned = () => new Promise(() => {});

/**
 * Sends one API request. Throws an error with `status` and `code` on a failure.
 *
 * An answer that arrives after the session ended, or after a new one began, is
 * dropped: the caller never continues. `across` exempts the requests that open
 * and close a session.
 */
async function api(method, path, body, parameters = {}, { across = false } = {}) {
  const epoch = state.epoch;
  const stale = () => !across && epoch !== state.epoch;
  const url = new URL(path, location.origin);
  const all = { view: state.kind, today: localDate(), lang: state.language, ...parameters };
  for (const [name, value] of Object.entries(all)) url.searchParams.set(name, value);
  let response;
  try {
    response = await fetch(url, {
      method,
      credentials: "same-origin",
      headers: { "Content-Type": "application/json", "X-Husker": "1" },
      body: body === undefined ? undefined : JSON.stringify(body),
      // A stalled connection must not hold a request, and a sheet that waits for it, for minutes.
      signal: AbortSignal.timeout?.(TIMEOUT),
    });
  } catch (failure) {
    if (stale()) return abandoned();
    throw failure;
  }
  const data = response.status === 204 ? null : await response.json().catch(() => undefined);
  if (stale()) return abandoned();
  if (response.status === 204) return null;
  if (!response.ok) {
    // An error without the server's own message comes from something in front of the server.
    const failure = new Error(data?.error?.message ?? t("web_server_error").replace("{status}", response.status));
    failure.status = response.status;
    failure.code = data?.error?.code;
    throw failure;
  }
  // An answer that is not the server's counts as no answer.
  if (data === undefined || data === null) throw new Error("The answer is not JSON.");
  return data;
}

function notice(text) {
  $("notice").textContent = text ?? "";
  $("notice").hidden = !text;
}

function toast(text, undo) {
  clearTimeout(state.toastTimer);
  $("toast-text").textContent = text;
  $("toast-undo").hidden = !undo;
  $("toast-undo").onclick = undo
    ? () => {
        $("toast").hidden = true;
        undo();
      }
    : null;
  $("toast").hidden = false;
  state.toastTimer = setTimeout(() => {
    $("toast").hidden = true;
    restartWhenIdle();
  }, 6000);
}

/** Shows what went wrong after a failed change. A lost session goes to the pairing screen. */
function report(failure) {
  if (failure.status === 401) return showPairing(t("web_unauthorized"));
  if (failure.code === "task_not_found") return toast(t("web_task_missing"));
  toast(failure.status ? failure.message : t("web_not_saved"));
}

function applyTexts() {
  document.documentElement.lang = state.language;
  $("add-title").placeholder = t("new_task");
  $("add-title").setAttribute("aria-label", t("new_task"));
  $("add-submit").setAttribute("aria-label", t("web_add"));
  $("add-list").setAttribute("aria-label", t("web_add_list"));
  $("filter").setAttribute("aria-label", t("web_filter"));
  $("refresh").setAttribute("aria-label", t("action_refresh"));
  $("menu-open").setAttribute("aria-label", t("web_menu"));
  $("menu").setAttribute("aria-label", t("web_menu"));
  $("pair-title").textContent = t("web_pair_title");
  $("pair-hint").textContent = t("web_pair_hint");
  $("pair-code").placeholder = "XXXX-XXXX-XXXX";
  $("pair-code").setAttribute("aria-label", t("web_pair_code"));
  $("pair-submit").textContent = t("web_pair_button");
  $("label-title").textContent = t("title");
  $("label-description").textContent = t("description");
  $("label-due").textContent = t("due");
  $("label-list").textContent = t("action_move_task");
  $("task-save").textContent = t("web_save");
  $("task-cancel").textContent = t("web_cancel_task");
  $("task-trash").textContent = t("action_trash");
  $("menu-close").textContent = t("web_close");
  $("sign-out").textContent = t("web_sign_out");
  $("toast-undo").textContent = t("undo_short");
  $("more").textContent = t("web_show_more");
  const presets = { today: "today", tomorrow: "tomorrow", next_monday: "next_monday", clear: "clear_due" };
  for (const button of document.querySelectorAll("[data-preset]")) {
    button.textContent = t(presets[button.dataset.preset]);
  }
  const views = { all: "all_tasks", history: "history", trash: "trash" };
  for (const button of document.querySelectorAll("[data-view]")) {
    button.textContent = t(views[button.dataset.view]);
    button.setAttribute("aria-current", String(button.dataset.view === state.kind));
  }
  for (const button of document.querySelectorAll("[data-lang]")) {
    button.textContent = t(`web_lang_${button.dataset.lang}`);
    button.setAttribute("aria-current", String(button.dataset.lang === state.language));
  }
  markDraft();
  // The first entry of the list filter is a text of the page, so the pickers are drawn again.
  state.pickers = "";
}

function option(value, text) {
  const element = document.createElement("option");
  element.value = value;
  element.textContent = text;
  return element;
}

function line(text) {
  const element = document.createElement("p");
  element.className = "notice";
  element.textContent = text;
  return element;
}

function fillLists() {
  const lists = state.view?.lists ?? [];
  if (!lists.some((list) => list.id === state.filter)) state.filter = "";
  if (!lists.some((list) => list.id === state.addList)) state.addList = "";
  // A picker that is open on the phone keeps its entries until a list or a count changes.
  const pickers = JSON.stringify([state.kind, lists.map((list) => [list.id, list.name, list.count])]);
  if (pickers !== state.pickers) {
    state.pickers = pickers;
    const filter = $("filter");
    filter.replaceChildren(option("", t(state.kind === "all" ? "all_tasks" : state.kind)));
    for (const list of lists) filter.append(option(list.id, `${list.name} (${list.count})`));
    for (const select of [$("add-list"), $("task-list")]) {
      select.replaceChildren(...lists.map((list) => option(list.id, list.name)));
    }
    // New entries lose the choice of the sheet's picker. It shows the list of its task again.
    if (state.sheetTask && $("sheet").open) $("task-list").value = sheetList(state.sheetTask);
  }
  $("filter").value = state.filter;
  $("add-list").value = state.addList || state.filter || state.view?.suggested_list || lists[0]?.id || "";
}

function rowElement(row) {
  const element = document.createElement("div");
  const closed = row.status !== "open";
  element.className = "row";
  element.dataset.id = row.id;
  element.classList.toggle("closed", closed);
  element.classList.toggle("pending", state.pending.has(row.id));

  const body = document.createElement("button");
  body.type = "button";
  body.className = "body";
  const title = document.createElement("span");
  title.className = "title";
  title.textContent = row.title;
  const meta = document.createElement("span");
  meta.className = "meta";
  const details = [];
  if (!state.filter && row.list_name) details.push(row.list_name);
  if (row.show_due && row.due_label) details.push(row.due_label);
  meta.textContent = details.join(" · ");
  body.append(title);
  if (details.length) body.append(meta);
  body.addEventListener("click", () => openSheet(row.id));

  if (state.kind === "trash") {
    element.append(body);
    if (state.view.write) {
      const restore = document.createElement("button");
      restore.type = "button";
      restore.className = "restore";
      restore.textContent = t("action_restore");
      restore.addEventListener("click", () => {
        if (!state.pending.has(row.id)) change(row.id, { deleted: false }, { adopt: true });
      });
      element.append(restore);
    }
    return element;
  }

  const check = document.createElement("button");
  check.type = "button";
  check.className = "check";
  check.textContent = row.status === "done" ? "☑" : row.status === "cancelled" ? "☒" : "☐";
  check.setAttribute("aria-label", t(closed ? "action_reopen" : "web_mark_done"));
  check.setAttribute("aria-pressed", String(closed));
  check.disabled = !state.view.write;
  check.addEventListener("click", () => toggle(row.id));
  element.append(check, body);
  return element;
}

/** The row of a new task that the server has not confirmed yet. */
function addingRow(task) {
  const element = document.createElement(task.failed ? "button" : "div");
  if (task.failed) element.type = "button";
  element.className = "row adding";
  element.classList.toggle("pending", !task.failed);
  const title = document.createElement("span");
  title.className = "title";
  title.textContent = task.body.title;
  element.append(title);
  if (task.failed) {
    const meta = document.createElement("span");
    meta.className = "meta";
    meta.textContent = t("web_not_sent");
    element.append(meta);
    element.addEventListener("click", () => sendTask(task));
  }
  return element;
}

/** Draws the task screen. */
function render() {
  const view = state.view;
  const rows = $("rows");
  const waiting = state.loading > 0 || state.pairing;
  $("refresh").classList.toggle("busy", waiting);
  // A connection problem matters more than the reminder that the access is read-only.
  notice(state.problem ?? (view && !view.write ? t("web_read_only") : null));
  const session = view?.session;
  $("session").textContent = session
    ? t("web_paired_as").replace("{name}", session.name).replace("{date}", session.expires).replace("{id}", session.id)
    : "";
  $("session").hidden = !session;
  fillLists();
  if (!view) {
    // Nothing is kept on this phone yet. The first answer can take seconds while the server starts.
    $("add").hidden = true;
    $("more").hidden = true;
    rows.replaceChildren(...(waiting ? [line(t("web_loading"))] : []));
    return;
  }
  $("add").hidden = !view.write || state.kind !== "all";
  // The rows are built anew. The control that had the focus gets it back.
  const active = document.activeElement;
  const focused = active?.closest?.(".row")?.dataset?.id;
  const built = new Map();
  rows.replaceChildren();
  // A new task shows above the groups until the answer brings its place.
  const adding = state.kind === "all" ? state.adding : [];
  for (const task of adding) rows.append(addingRow(task));
  const groups = filterGroups(view.groups, state.filter);
  for (const group of groups) {
    if (group.label) {
      const heading = document.createElement("h2");
      heading.className = "group";
      heading.textContent = `${group.label} (${group.count})`;
      rows.append(heading);
    }
    for (const row of group.rows) {
      const element = rowElement(row);
      built.set(row.id, element);
      rows.append(element);
    }
  }
  if (groups.length === 0 && adding.length === 0) {
    rows.append(line(t(state.kind === "trash" ? "trash_empty" : "no_tasks")));
  }
  $("more").hidden = view.next_offset === null || view.next_offset === undefined;
  if (focused) {
    const control = [...(built.get(focused)?.children ?? [])].find((child) => child.className === active.className);
    control?.focus?.({ preventScroll: true });
  }
}

function showApp() {
  $("pairing").hidden = true;
  $("app").hidden = false;
  render();
}

/**
 * Starts a new session count. Requests of the count before are abandoned, and
 * with them everything on the page that waited for their answers.
 */
function newEpoch() {
  state.epoch += 1;
  state.loading = 0;
  state.saving = false;
  state.pending.clear();
  // A new task that was on its way gets no answer any more. It counts as not sent.
  for (const task of state.adding) task.failed = true;
  setSheetBusy(false);
}

/** Shows the pairing screen. Nothing of the tasks stays on the phone or in the page. */
function showPairing(message) {
  newEpoch();
  store(VIEW_KEY, null);
  store(UNSENT_KEY, null);
  state.view = null;
  state.adding = [];
  state.problem = null;
  state.kind = "all";
  state.filter = "";
  state.addList = "";
  state.sheetTask = null;
  state.sheetShown = null;
  state.sheetRetry = null;
  for (const dialog of document.querySelectorAll("dialog[open]")) dialog.close();
  $("rows").replaceChildren();
  for (const field of [$("add-title"), $("task-title"), $("task-description"), $("task-due")]) field.value = "";
  for (const picker of [$("filter"), $("add-list"), $("task-list")]) picker.replaceChildren();
  $("session").textContent = "";
  $("toast-text").textContent = "";
  $("sheet").setAttribute("aria-label", "");
  applyTexts();
  $("toast").hidden = true;
  $("app").hidden = true;
  $("pairing").hidden = false;
  $("pair-error").textContent = message ?? "";
  $("pair-error").hidden = !message;
}

/**
 * Puts a view from the server on screen. A row with a change under way keeps
 * the values the user chose, so a refresh does not flip it back for a moment.
 */
function adopt(view) {
  // An answer without the shape of a view is not the server's. The screen stays as it is.
  if (!drawable(view)) return;
  if (state.kind === "all") store(VIEW_KEY, view);
  for (const [id, values] of state.pending) {
    if (values) view = patchRow(view, id, values);
  }
  state.view = view;
  render();
}

/**
 * Notes that the server confirmed a change. Returns whether the view in that
 * answer is current: no other change was confirmed since its request was sent.
 *
 * The server answers a read without waiting for a write that is under way. A
 * view whose request was sent before a confirmed change can therefore be older
 * than that change, whenever it arrives.
 */
function confirmed(number) {
  const current = number > state.floor;
  state.floor = state.sent;
  // The server answers, so the new tasks that wait for it go out again.
  resend();
  return current;
}

/**
 * Shows the view from the answer to a change. Views appear in the order their
 * requests were sent. An answer for a view the user has left, one that arrives
 * after a newer answer, and one that may be older than a confirmed change are
 * replaced by a fresh request.
 */
function showAnswer(kind, number, view, current) {
  if (!current || kind !== state.kind || number < state.shown) return load();
  state.shown = number;
  adopt(view);
}

/** History and Trash ask for a page of rows, and for as many as the screen holds. */
function pageSize() {
  if (state.kind === "all" || !state.view) return {};
  return { limit: Math.min(200, Math.max(100, rowCount(state.view))) };
}

/** Requests the current view. `unpaired` is the message for a phone whose pairing code was refused. */
async function load(unpaired) {
  // The pairing screen is up, and it asks for no tasks.
  if ($("app").hidden) return;
  const kind = state.kind;
  const number = ++state.sent;
  state.loading += 1;
  render();
  try {
    const view = await api("GET", "/api/view", undefined, pageSize());
    state.loading -= 1;
    // A newer answer is on screen already, or the user moved to another view.
    if (kind !== state.kind || number < state.shown) return render();
    // A change was confirmed after this request was sent, so the view may be older than it.
    if (number <= state.floor) {
      render();
      return load(unpaired);
    }
    state.shown = number;
    state.problem = null;
    adopt(view);
    // The phone was paired before, and the code it just tried was refused.
    if (unpaired) toast(unpaired);
    resend();
  } catch (failure) {
    state.loading -= 1;
    if (failure.status === 401) return showPairing(unpaired ?? (state.view ? t("web_unauthorized") : undefined));
    if (kind !== state.kind || number < state.shown) return render();
    const away = state.view ? `${t("web_offline")} ${t("web_saved_view")}` : t("web_offline");
    state.problem = failure.status ? failure.message : away;
    render();
  }
}

async function loadMore() {
  const view = state.view;
  try {
    const page = await api("GET", "/api/view", undefined, { offset: view.next_offset });
    // A second tap, a refresh, or another view replaced the rows this page continues.
    if (view !== state.view) return;
    state.view = appendPage(view, page);
    render();
  } catch (failure) {
    report(failure);
  }
}

/**
 * Sends a change to one task. With `adopt`, the answer's view replaces the screen.
 * Without it, only the row changes, so a closed task keeps its place.
 */
async function change(id, body, { adopt: adoptView = false, undo, optimistic = null } = {}) {
  const kind = state.kind;
  const number = ++state.sent;
  const before = optimistic ? findRow(state.view, id) : null;
  // The screen changes first. The request follows, and a failure puts the row back.
  if (optimistic) state.view = patchRow(state.view, id, optimistic);
  state.pending.set(id, optimistic);
  render();
  try {
    // A row that keeps its place needs the row from the server, not the whole view.
    const answer = await api("PATCH", `/api/tasks/${id}`, body, adoptView ? pageSize() : { view: "none" });
    state.pending.delete(id);
    const current = confirmed(number);
    if (adoptView) showAnswer(kind, number, answer.view, current);
    else showRow(kind, id, answer.task, current);
    if (undo && state.view) toast(undo.text, undo.action);
    restartWhenIdle();
    return true;
  } catch (failure) {
    state.pending.delete(id);
    // Only the values this change set go back. Other rows may have changed meanwhile.
    if (before && state.view) {
      const old = Object.fromEntries(Object.keys(optimistic).map((key) => [key, before[key]]));
      state.view = patchRow(state.view, id, old);
    }
    render();
    report(failure);
    // Without an answer the change may still have reached the server. The next view tells.
    if (!failure.status) load();
    return false;
  }
}

/**
 * Puts the server's row of one task into the view on screen. The row keeps its place.
 * When another change was confirmed since the request was sent, the two answers
 * do not say which one the server saved last, so the view is read again.
 */
function showRow(kind, id, row, current) {
  if (kind !== state.kind) return load();
  if (!state.view) return;
  state.view = patchRow(state.view, id, row);
  if (state.kind === "all") {
    state.view = recount(state.view);
    store(VIEW_KEY, state.view);
  }
  render();
  if (!current) load();
}

function toggle(id) {
  const row = findRow(state.view, id);
  if (!row || state.pending.has(id)) return;
  const status = row.status === "open" ? "done" : "open";
  // In All tasks the row keeps its place. History holds closed tasks only, so a reopened task leaves it.
  change(id, { status }, state.kind === "all" ? { optimistic: { status } } : { adopt: true });
}

function addTask(event) {
  event.preventDefault();
  const input = $("add-title");
  const title = input.value.trim();
  if (!title) return;
  const task = { body: { id: newId(), title }, failed: false };
  if ($("add-list").value) task.body.list_id = $("add-list").value;
  // The field empties at once, so the next task can be typed while this one is sent.
  input.value = "";
  state.adding.push(task);
  keepUnsent();
  return sendTask(task);
}

/** The new tasks that are kept on the phone because the server does not have them yet. */
function unsentBodies() {
  const kept = stored(UNSENT_KEY);
  if (!Array.isArray(kept)) return [];
  return kept.filter((body) => typeof body?.id === "string" && typeof body?.title === "string");
}

/**
 * Keeps the new tasks that the server does not have yet on the phone. A browser can
 * unload the page at any time, and a task that waits must survive that. `done` is
 * the id of a task that no longer waits.
 */
function keepUnsent(done) {
  // Another open page of this phone can hold tasks of its own. They stay in the kept list.
  const mine = state.adding.map((task) => task.body);
  const others = unsentBodies().filter((body) => body.id !== done && !mine.some((own) => own.id === body.id));
  const all = [...others, ...mine];
  store(UNSENT_KEY, all.length > 0 ? all : null);
}

/** The server has the task, or refused it: it no longer waits. */
function forget(task) {
  state.adding = state.adding.filter((other) => other !== task);
  keepUnsent(task.body.id);
}

/**
 * Sends a new task. The page made its id, so a repeat creates no second task:
 * one retry covers a lost answer, and a task without an answer can be sent again later.
 */
async function sendTask(task) {
  const kind = state.kind;
  const number = ++state.sent;
  task.failed = false;
  render();
  try {
    let answer;
    try {
      answer = await api("POST", "/api/tasks", task.body, pageSize());
    } catch (failure) {
      if (failure.status) throw failure;
      answer = await api("POST", "/api/tasks", task.body, pageSize());
    }
    // Only an answer that holds the task says that the server has it.
    if (answer?.task?.id !== task.body.id) throw new Error("The answer does not hold the task.");
    forget(task);
    showAnswer(kind, number, answer.view, confirmed(number));
    restartWhenIdle();
  } catch (failure) {
    if (failure.status === 401) return report(failure);
    if (failure.code === "task_exists") {
      // An earlier try reached the server, and the task changed there since. The server has it.
      forget(task);
      render();
      return load();
    }
    // The server refuses a task with a status from 400 through 499 and its own error object.
    // Everything else is no answer, or a server that cannot save now.
    const refused =
      failure.code && failure.status >= 400 && failure.status < 500 && failure.status !== 408 && failure.status !== 429;
    if (!refused) {
      // The task stays on screen and on the phone. It is sent again on a tap or when the server answers.
      task.failed = true;
      render();
      if (failure.status) toast(failure.message);
      return;
    }
    // The server said why it refused. The title returns to the field,
    // or the message names it when the next title is typed there already.
    forget(task);
    render();
    const input = $("add-title");
    if (input.value) toast(`${task.body.title}: ${failure.message}`);
    else {
      input.value = task.body.title;
      toast(failure.message);
    }
  }
}

/** Sends the new tasks that got no answer, now that the server answers again. */
function resend() {
  for (const task of state.adding) {
    if (task.failed) sendTask(task);
  }
}

function openSheet(id) {
  const row = findRow(state.view, id);
  if (!row || state.pending.has(id)) return;
  state.sheetTask = row;
  state.sheetRetry = null;
  state.sheetEditable = state.view.write && !row.deleted;
  fillSheet(row);
  $("sheet-error").hidden = true;
  for (const element of [$("plan"), $("move"), $("task-save"), $("task-toggle"), $("task-trash")]) {
    element.hidden = !state.sheetEditable;
  }
  setSheetBusy(state.saving);
  $("sheet").setAttribute("aria-label", row.title);
  $("sheet").showModal();
  // A text field in focus would open the phone's keyboard over the sheet.
  if (document.activeElement?.matches?.("input, textarea, select")) $("sheet-form").focus();
}

/** What a field shows for a value. A browser changes line breaks when a field takes text. */
function shownAs(field, value) {
  const typed = field.value;
  field.value = value;
  const shown = field.value;
  field.value = typed;
  return shown;
}

/** The list the sheet's picker shows: the one of a button that waits for Save, or the task's. */
function sheetList(row) {
  return state.sheetRetry?.action.list_id ?? row.list_id;
}

/**
 * Puts a task into the sheet and remembers what its fields show for it.
 * `mine` names the fields the user changed. They keep what the user typed.
 */
function fillSheet(row, mine = {}) {
  state.sheetShown = {
    title: shownAs($("task-title"), row.title),
    description: shownAs($("task-description"), row.description),
    due: row.due ?? "",
  };
  if (!("title" in mine)) $("task-title").value = row.title;
  if (!("description" in mine)) $("task-description").value = row.description;
  if (!("due" in mine) && !("clear_due" in mine)) $("task-due").value = row.due ?? "";
  $("task-list").value = sheetList(row);
  $("task-toggle").textContent = t(row.status === "open" ? "web_mark_done" : "action_reopen");
  $("task-cancel").hidden = !state.sheetEditable || row.status === "cancelled";
  markDraft();
}

/** While a request of the sheet is under way, its fields and buttons wait. The close button stays in reach. */
function setSheetBusy(busy) {
  $("sheet-form").classList.toggle("saving", busy);
  $("task-save").disabled = busy;
  for (const field of [$("task-title"), $("task-description"), $("task-due"), $("task-list")]) {
    field.disabled = busy || !state.sheetEditable;
  }
  markDraft();
}

/**
 * The task of the sheet is gone from this view. The sheet keeps the text in sight
 * and offers only its close button, because nothing can be saved from it any more.
 */
function endSheet(message) {
  state.sheetRetry = null;
  state.sheetEditable = false;
  $("sheet-error").textContent = message;
  $("sheet-error").hidden = false;
  for (const element of [$("plan"), $("move"), $("task-save"), $("task-toggle"), $("task-cancel"), $("task-trash")]) {
    element.hidden = true;
  }
}

/** A date typed into the date field is newer than a date button that waits for Save. */
function dropWaitingDate() {
  const retry = state.sheetRetry;
  if (!retry) return;
  const rest = { ...retry.action };
  delete rest.preset;
  delete rest.clear_due;
  state.sheetRetry = Object.keys(rest).length > 0 ? { ...retry, action: rest } : null;
}

/** What the user changed in the fields of the sheet, as the members of a change. */
function draft() {
  const shown = state.sheetShown;
  const changed = {};
  if (!state.sheetTask || !shown) return changed;
  const title = $("task-title").value.trim();
  if (title !== shown.title.trim()) changed.title = title;
  if ($("task-description").value !== shown.description) changed.description = $("task-description").value;
  const due = $("task-due").value;
  if (due !== shown.due) Object.assign(changed, due ? { due } : { clear_due: true });
  return changed;
}

/** The close button says when closing drops what the user changed. While a request runs, the changes are on their way. */
function markDraft() {
  const drops = !state.saving && Object.keys(draft()).length > 0;
  $("sheet-close").textContent = t(drops ? "web_discard" : "web_close");
}

/** Whether the sheet is open and shows this task. */
function sheetShows(task) {
  return $("sheet").open && state.sheetTask?.id === task.id;
}

/**
 * Runs the action of a button or a picker in the sheet. Changed fields go to
 * the server in the same request, so a tap on a button drops no typed text.
 */
function sheetChange(action, options = { adopt: true }) {
  const task = state.sheetTask;
  if (!task || state.saving) return;
  state.sheetRetry = null;
  if (Object.keys(changedFields(action)).length > 0) return saveSheet(action, options);
  $("sheet").close();
  return change(task.id, action, options);
}

/** The changed fields that go out with an action. A tapped date wins over a date in the date field. */
function changedFields(action) {
  const fields = draft();
  if ("preset" in action || "clear_due" in action) {
    delete fields.due;
    delete fields.clear_due;
  }
  return fields;
}

/**
 * Sends the changed fields of the sheet, with the action of a tapped button if
 * there is one. The sheet stays open until the server accepted the change.
 */
async function saveSheet(action = {}, options = {}) {
  const task = state.sheetTask;
  if (!task || state.saving) return;
  const { undo, optimistic = null } = options;
  const body = { ...changedFields(action), ...action };
  if (Object.keys(body).length === 0) return $("sheet").close();
  const kind = state.kind;
  const number = ++state.sent;
  // The version guards the text. A date, a list, a status, and Trash apply to the newest task.
  const text = "title" in body || "description" in body;
  state.saving = true;
  // The fields and buttons wait. Text typed now would not be part of the request.
  setSheetBusy(true);
  // The row waits too: no checkbox and no second sheet changes the task meanwhile.
  state.pending.set(task.id, null);
  render();
  try {
    const answer = await api(
      "PATCH",
      `/api/tasks/${task.id}`,
      text ? { ...body, version: task.version } : body,
      optimistic ? { view: "none" } : pageSize(),
    );
    state.pending.delete(task.id);
    const current = confirmed(number);
    state.sheetRetry = null;
    if (sheetShows(task)) $("sheet").close();
    if (optimistic) showRow(kind, task.id, answer.task, current);
    else showAnswer(kind, number, answer.view, current);
    if (undo && state.view) toast(undo.text, undo.action);
  } catch (failure) {
    state.pending.delete(task.id);
    const open = sheetShows(task);
    // The button the user tapped still counts while the sheet stays open:
    // the next Save sends its action with the fields.
    if (open) state.sheetRetry = Object.keys(action).length > 0 ? { action, options } : null;
    render();
    if (failure.status === 401) return report(failure);
    const stale = failure.code === "stale_version";
    const gone = failure.code === "task_not_found";
    const message = stale
      ? t("web_stale")
      : gone
        ? t("web_task_missing")
        : failure.status
          ? failure.message
          : t("web_not_saved");
    if (!open) {
      // The sheet was closed while the request ran, so the message names the task.
      toast(`${task.title}: ${message}`);
      if (!failure.status) load();
      return;
    }
    if (gone) return endSheet(message);
    // The fields keep what the user typed.
    $("sheet-error").textContent = message;
    $("sheet-error").hidden = false;
    $("task-list").value = sheetList(state.sheetTask);
    // The task may be another one on the server now: changed elsewhere, or changed by this
    // request when only its answer was lost. The sheet takes the task as it is.
    if (stale || !failure.status) await rebase(task);
  } finally {
    state.saving = false;
    setSheetBusy(false);
    restartWhenIdle();
  }
}

/**
 * After the server refused text for a stale version: reads the task again and
 * shows its current values in the fields the user did not change. The next Save
 * sends only the changed fields, with the current version, so it cannot undo
 * what was changed elsewhere in the task.
 */
async function rebase(task) {
  await load();
  if (!sheetShows(task)) return;
  const current = findRow(state.view, task.id);
  // The task left this view, for example to Trash on a computer. A Save would be refused again and again.
  if (!current) return endSheet(t("web_task_left"));
  const mine = draft();
  state.sheetTask = current;
  fillSheet(current, mine);
}

function trashTask() {
  const task = state.sheetTask;
  sheetChange(
    { deleted: true },
    {
      adopt: true,
      undo: {
        text: t("deleted"),
        action: () => change(task.id, { deleted: false }, { adopt: true }),
      },
    },
  );
}

/** Closes or reopens the task of the sheet. In All tasks the row keeps its place, as after a tap on its checkbox. */
function sheetStatus(status) {
  sheetChange({ status }, state.kind === "all" ? { optimistic: { status } } : { adopt: true });
}

/** Exchanges a pairing code for a session. */
async function pair(code) {
  if (state.pairing) return;
  state.pairing = true;
  let failure = null;
  try {
    // An unfinished sign-out comes first, so the old cookie is gone before a new one arrives.
    if (stored(SIGN_OUT_KEY) && !(await finishSignOut())) return;
    // The task screen shows while the code is checked. A server that has to start takes seconds.
    showApp();
    await api("POST", "/api/session", { code }, {}, { across: true });
  } catch (error) {
    failure = error;
  } finally {
    state.pairing = false;
  }
  if (!failure) {
    // A new session. What still arrives for the session before belongs to nobody.
    newEpoch();
    $("pair-code").value = "";
    showApp();
    return load();
  }
  if (!failure.status && !state.view) {
    // The code was not used. It stays in the field for the next try.
    showPairing(t("web_offline"));
    $("pair-code").value = code;
    return;
  }
  const wrong = failure.code === "unknown_code" || failure.status === 400;
  // The phone may be paired already. Then a refused code changes nothing, and its tasks show.
  showApp();
  return load(wrong ? t("web_pair_failed") : failure.status ? failure.message : undefined);
}

/** Takes a pairing code out of the address, so it is used once. */
function codeFromAddress() {
  const code = readCode(location.hash);
  if (code) history.replaceState(null, "", location.pathname);
  return code;
}

/**
 * Uses the pairing code of a link. A phone that is paired already is asked first,
 * so a link cannot replace a pairing without a question. The server says whether
 * the phone is paired, because a paired phone can be without kept tasks.
 */
async function useLink(code) {
  if (state.pairing) return;
  if (!stored(SIGN_OUT_KEY)) {
    state.pairing = true;
    showApp();
    let paired = false;
    let reached = true;
    try {
      await api("GET", "/api/session", undefined, {}, { across: true });
      paired = true;
    } catch (failure) {
      reached = failure.status === 401;
    }
    state.pairing = false;
    if (!reached) {
      // Without an answer the code cannot be used now. It waits in the field of the pairing screen.
      if (state.view) return load();
      showPairing(t("web_offline"));
      $("pair-code").value = code;
      return;
    }
    if (paired && !confirm(t("web_pair_replace"))) return load();
  }
  return pair(code);
}

let signOutRequest = null;

/**
 * Asks the server to remove the session cookie. Page script cannot remove it.
 * Until the server answered, the page keeps a note and shows no task. One
 * request runs at a time, so a late answer cannot remove a newer cookie.
 */
function finishSignOut() {
  signOutRequest ??= (async () => {
    try {
      await api("DELETE", "/api/session", undefined, {}, { across: true });
    } catch {
      $("pair-error").textContent = t("web_sign_out_later");
      $("pair-error").hidden = false;
      return false;
    }
    store(SIGN_OUT_KEY, null);
    return true;
  })().finally(() => {
    signOutRequest = null;
  });
  return signOutRequest;
}

async function signOut() {
  $("menu").close();
  // The tasks leave the screen and the phone at once, with or without a connection.
  store(SIGN_OUT_KEY, true);
  showPairing();
  await finishSignOut();
}

async function setLanguage(language) {
  state.language = language;
  store(LANGUAGE_KEY, language);
  applyTexts();
  $("menu").close();
  await load();
}

async function setView(kind) {
  state.kind = kind;
  state.filter = "";
  state.addList = "";
  state.view = kind === "all" ? drawable(stored(VIEW_KEY)) : null;
  state.problem = null;
  applyTexts();
  $("menu").close();
  await load();
}

/**
 * Reads the page's texts. Kept texts show at once and the current ones follow,
 * so the page does not wait for a server that has to start.
 */
async function loadStrings() {
  const current = fetch("/strings.json")
    .then(async (response) => {
      if (!response.ok) return;
      state.strings = await response.json();
      store(STRINGS_KEY, state.strings);
      applyTexts();
      if (!$("app").hidden) render();
    })
    .catch(() => {
      // The kept texts are used without a connection.
    });
  state.strings = stored(STRINGS_KEY);
  if (!state.strings) await current;
}

/**
 * Starts the service worker, which keeps the page files, and follows releases.
 * When the worker of a new release takes over, the page starts again with its files.
 */
function watchRelease() {
  const workers = navigator.serviceWorker;
  if (!workers) return;
  // The first worker of a phone takes over a page that it did not serve. That is no new release.
  let controlled = Boolean(workers.controller);
  workers.register("/sw.js").catch(() => {});
  workers.addEventListener("controllerchange", () => {
    if (controlled) {
      state.release = true;
      restartWhenIdle();
    }
    controlled = true;
  });
}

/** Starts the page again for a new release, unless that would drop something typed or under way. */
function restartWhenIdle() {
  if (!state.release) return;
  const undo = !$("toast").hidden && !$("toast-undo").hidden;
  const busy =
    $("add-title").value ||
    $("pair-code").value ||
    $("sheet").open ||
    $("menu").open ||
    state.pending.size > 0 ||
    state.adding.some((task) => !task.failed) ||
    state.saving ||
    state.pairing ||
    undo;
  if (!busy) location.reload();
}

/** Keeps the sheets and the message above the phone's keyboard, which covers the bottom of the page. */
function watchKeyboard() {
  const viewport = window.visualViewport;
  if (!viewport) return;
  const update = () => {
    // A zoomed page shows a smaller part too. Only an unzoomed page measures the keyboard.
    const zoomed = Math.abs(viewport.scale - 1) > 0.01;
    const covered = zoomed ? 0 : window.innerHeight - viewport.height - viewport.offsetTop;
    document.documentElement.style.setProperty("--keyboard", `${covered > 100 ? Math.round(covered) : 0}px`);
  };
  viewport.addEventListener("resize", update);
  viewport.addEventListener("scroll", update);
}

function wire() {
  $("add").addEventListener("submit", addTask);
  $("add-list").addEventListener("change", (event) => {
    state.addList = event.target.value;
  });
  $("filter").addEventListener("change", (event) => {
    state.filter = event.target.value;
    state.addList = "";
    render();
  });
  $("refresh").addEventListener("click", () => load());
  $("more").addEventListener("click", loadMore);
  $("menu-open").addEventListener("click", () => $("menu").showModal());
  $("menu-close").addEventListener("click", () => $("menu").close());
  $("sign-out").addEventListener("click", signOut);
  for (const button of document.querySelectorAll("[data-view]")) {
    button.addEventListener("click", () => setView(button.dataset.view));
  }
  for (const button of document.querySelectorAll("[data-lang]")) {
    button.addEventListener("click", () => setLanguage(button.dataset.lang));
  }
  $("sheet-close").addEventListener("click", () => $("sheet").close());
  // Save is the form's submit button, so the keyboard's Enter key saves too.
  $("sheet-form").addEventListener("submit", (event) => {
    event.preventDefault();
    saveSheet(state.sheetRetry?.action, state.sheetRetry?.options);
  });
  for (const field of [$("task-title"), $("task-description")]) {
    field.addEventListener("input", markDraft);
    field.addEventListener("change", markDraft);
  }
  for (const type of ["input", "change"]) {
    $("task-due").addEventListener(type, () => {
      dropWaitingDate();
      markDraft();
    });
  }
  $("task-toggle").addEventListener("click", () => sheetStatus(state.sheetTask.status === "open" ? "done" : "open"));
  $("task-cancel").addEventListener("click", () => sheetStatus("cancelled"));
  $("task-trash").addEventListener("click", trashTask);
  for (const button of document.querySelectorAll("[data-preset]")) {
    button.addEventListener("click", () =>
      sheetChange(button.dataset.preset === "clear" ? { clear_due: true } : { preset: button.dataset.preset }),
    );
  }
  $("task-list").addEventListener("change", (event) => {
    if (event.target.value !== state.sheetTask?.list_id) return sheetChange({ list_id: event.target.value });
    // The list of the task was chosen again. A move that waited for Save is off.
    if (state.sheetRetry?.action.list_id) {
      const rest = { ...state.sheetRetry.action };
      delete rest.list_id;
      state.sheetRetry = Object.keys(rest).length > 0 ? { ...state.sheetRetry, action: rest } : null;
    }
  });
  for (const dialog of [$("sheet"), $("menu")]) dialog.addEventListener("close", restartWhenIdle);
  $("pair-form").addEventListener("submit", (event) => {
    event.preventDefault();
    const code = readCode($("pair-code").value);
    if (code) pair(code);
    else showPairing(t("web_pair_failed"));
  });
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState !== "visible") return restartWhenIdle();
    // A page that stays open for days asks for a new release when it comes back to the screen.
    navigator.serviceWorker
      ?.getRegistration()
      .then((registration) => registration?.update())
      .catch(() => {});
    if (!$("app").hidden) load();
  });
  // A pairing link opened in a tab that already shows the page changes only the part after `#`.
  window.addEventListener("hashchange", () => {
    const code = codeFromAddress();
    if (code) useLink(code);
  });
  window.addEventListener("online", () => load());
}

async function start() {
  wire();
  watchKeyboard();
  await loadStrings();
  state.language = pickLanguage(stored(LANGUAGE_KEY), navigator.language);
  applyTexts();
  watchRelease();

  if (stored(SIGN_OUT_KEY)) {
    // A sign-out that did not reach the server finishes now, before any task shows.
    showPairing();
    await finishSignOut();
    const code = codeFromAddress();
    if (code) pair(code);
    return;
  }
  // The kept view shows at once. The current one replaces it when it arrives.
  state.view = drawable(stored(VIEW_KEY));
  state.adding = unsentBodies().map((body) => ({ body, failed: true }));
  const code = codeFromAddress();
  if (code) return useLink(code);
  try {
    showApp();
  } catch {
    // A view kept by another release of the page that this one cannot draw.
    state.view = null;
    store(VIEW_KEY, null);
    showApp();
  }
  await load();
}

start();
