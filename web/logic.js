// Pure functions of the phone page. They touch no document and no network,
// so `node --test web/logic.test.mjs` can run them.

/** The local calendar date as YYYY-MM-DD. The server groups tasks by this date. */
export function localDate(date = new Date()) {
  const pad = (number) => String(number).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

/**
 * Reads a pairing code from typed text, from an address, or from the part after `#`.
 * Returns the 12 characters, or null when the text holds no code.
 */
export function readCode(text) {
  if (typeof text !== "string") return null;
  const marker = text.lastIndexOf("code=");
  const raw = marker >= 0 ? text.slice(marker + 5).split(/[&#]/)[0] : text;
  const code = raw
    .toUpperCase()
    .replace(/[\s-]/g, "")
    .replace(/O/g, "0")
    .replace(/[IL]/g, "1");
  return /^[0-9A-HJKMNP-TV-Z]{12}$/.test(code) ? code : null;
}

/** Danish for a Danish browser, English otherwise. A saved choice wins. */
export function pickLanguage(saved, browser) {
  if (saved === "da" || saved === "en") return saved;
  return typeof browser === "string" && browser.toLowerCase().startsWith("da") ? "da" : "en";
}

/** The groups of a view for one list: other rows hidden, empty groups dropped, counts renewed. */
export function filterGroups(groups, listId) {
  if (!listId) return groups;
  return groups
    .map((group) => {
      const rows = group.rows.filter((row) => row.list_id === listId);
      return { ...group, rows, count: rows.length };
    })
    .filter((group) => group.rows.length > 0);
}

/** The row with this id, or null. */
export function findRow(view, id) {
  for (const group of view?.groups ?? []) {
    const row = group.rows.find((candidate) => candidate.id === id);
    if (row) return row;
  }
  return null;
}

/** A copy of the view in which one row has other values. The row keeps its place. */
export function patchRow(view, id, patch) {
  return {
    ...view,
    groups: view.groups.map((group) => ({
      ...group,
      rows: group.rows.map((row) => (row.id === id ? { ...row, ...patch } : row)),
    })),
  };
}

/**
 * A copy of the All tasks view whose lists count their open rows again. All tasks holds
 * every open task, so the count of a list is the number of its open rows.
 */
export function recount(view) {
  const open = new Map();
  for (const group of view.groups) {
    for (const row of group.rows) {
      if (row.status === "open") open.set(row.list_id, (open.get(row.list_id) ?? 0) + 1);
    }
  }
  return { ...view, lists: view.lists.map((list) => ({ ...list, count: open.get(list.id) ?? 0 })) };
}

/**
 * Appends the next page of a view. A group that continues across pages stays one group.
 * Its count is the server's: the number of rows of the whole group, loaded or not.
 */
export function appendPage(view, page) {
  const groups = view.groups.map((group) => ({ ...group, rows: [...group.rows] }));
  for (const group of page.groups) {
    const last = groups[groups.length - 1];
    if (last && last.key === group.key) {
      last.rows.push(...group.rows);
      last.count = group.count;
    } else {
      groups.push({ ...group, rows: [...group.rows] });
    }
  }
  return { ...page, groups };
}

/** The number of rows a view holds on the phone. */
export function rowCount(view) {
  return (view?.groups ?? []).reduce((count, group) => count + group.rows.length, 0);
}

/** A kept view that this page can draw, or null. A release can change the shape of a view. */
export function drawable(view) {
  const ok =
    Array.isArray(view?.lists) &&
    Array.isArray(view?.groups) &&
    view.groups.every((group) => Array.isArray(group?.rows) && group.rows.every((row) => typeof row?.id === "string"));
  return ok ? view : null;
}

/** A new task identifier: 32 lowercase hexadecimal characters from 16 random bytes. */
export function newId(random = (bytes) => crypto.getRandomValues(bytes)) {
  const bytes = random(new Uint8Array(16));
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}
