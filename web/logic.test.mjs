// Tests of the pure functions of the page. Run them with: node --test web/logic.test.mjs

import assert from "node:assert/strict";
import { test } from "node:test";

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

const view = () => ({
  view: "all",
  next_offset: null,
  lists: [
    { id: "inbox", name: "Inbox", count: 1 },
    { id: "work", name: "Work", count: 2 },
  ],
  groups: [
    {
      key: "today",
      label: "Today",
      count: 2,
      rows: [
        { id: "a", list_id: "inbox", title: "Buy milk", status: "open" },
        { id: "b", list_id: "work", title: "Write report", status: "open" },
      ],
    },
    {
      key: "undated",
      label: "Sometime",
      count: 1,
      rows: [{ id: "c", list_id: "work", title: "Call Ann", status: "open" }],
    },
  ],
});

test("the local date follows the phone's clock around midnight", () => {
  assert.equal(localDate(new Date(2026, 10, 15, 0, 30)), "2026-11-15");
  assert.equal(localDate(new Date(2026, 10, 15, 23, 30)), "2026-11-15");
  assert.equal(localDate(new Date(2026, 0, 5, 12, 0)), "2026-01-05");
});

test("a pairing code is read from an address, a fragment, and typed text", () => {
  assert.equal(readCode("#code=K7QF2M9XTB4H"), "K7QF2M9XTB4H");
  assert.equal(readCode("https://tasks.example.net/#code=K7QF2M9XTB4H"), "K7QF2M9XTB4H");
  assert.equal(readCode("k7qf-2m9x-tb4h"), "K7QF2M9XTB4H");
  assert.equal(readCode(" K7QF 2M9X TB4H "), "K7QF2M9XTB4H");
  // The letters O, I, and L are read as the digits they look like.
  assert.equal(readCode("OIL0-0000-0000"), "011000000000");
  for (const text of ["", "#", "K7QF2M9XTB4", "K7QF2M9XTB4HH", "K7QF2M9XTB4U", "#token=abc", null]) {
    assert.equal(readCode(text), null, String(text));
  }
});

test("the language follows a saved choice and then the browser", () => {
  assert.equal(pickLanguage(null, "da-DK"), "da");
  assert.equal(pickLanguage(null, "DA"), "da");
  assert.equal(pickLanguage(null, "en-GB"), "en");
  assert.equal(pickLanguage(null, undefined), "en");
  assert.equal(pickLanguage("en", "da-DK"), "en");
  assert.equal(pickLanguage("da", "en-US"), "da");
  assert.equal(pickLanguage("xx", "de"), "en");
});

test("the list filter hides other rows, drops empty groups, and counts again", () => {
  const groups = view().groups;
  assert.equal(filterGroups(groups, ""), groups);
  const work = filterGroups(groups, "work");
  assert.deepEqual(
    work.map((group) => [group.key, group.count, group.rows.map((row) => row.id)]),
    [
      ["today", 1, ["b"]],
      ["undated", 1, ["c"]],
    ],
  );
  const inbox = filterGroups(groups, "inbox");
  assert.deepEqual(inbox.map((group) => group.key), ["today"]);
  assert.equal(groups[0].count, 2, "the view itself is not changed");
});

test("a row is found and patched in place, and the lists count again", () => {
  const original = view();
  assert.equal(findRow(original, "c").title, "Call Ann");
  assert.equal(findRow(original, "x"), null);
  assert.equal(findRow(null, "a"), null);

  const patched = patchRow(original, "a", { status: "done" });
  assert.equal(findRow(patched, "a").status, "done");
  assert.equal(findRow(original, "a").status, "open", "the original view is not changed");
  assert.deepEqual(
    patched.groups.map((group) => group.rows.map((row) => row.id)),
    [["a", "b"], ["c"]],
    "the row keeps its place",
  );

  // A closed row no longer counts for its list. A list without open rows counts zero.
  const counted = recount(patched);
  assert.deepEqual(
    counted.lists.map((list) => [list.id, list.count]),
    [
      ["inbox", 0],
      ["work", 2],
    ],
  );
  assert.equal(patched.lists[0].count, 1, "the view itself is not changed");
  assert.equal(rowCount(original), 3);
  assert.equal(rowCount(null), 0);
});

test("a kept view is drawn only when it has the shape of a view", () => {
  const good = view();
  assert.equal(drawable(good), good);
  for (const broken of [null, "text", {}, { view: "all" }, { lists: [], groups: [{}] }, { lists: [], groups: [{ rows: [{}] }] }]) {
    assert.equal(drawable(broken), null, JSON.stringify(broken));
  }
});

test("a next page continues the last group", () => {
  const first = view();
  const page = {
    view: "history",
    next_offset: null,
    groups: [
      // The server counts the whole group: five rows, of which two are loaded after this page.
      { key: "undated", label: "Sometime", count: 5, rows: [{ id: "d", list_id: "work" }] },
      { key: "done", label: "Done", count: 1, rows: [{ id: "e", list_id: "work" }] },
    ],
  };
  const joined = appendPage(first, page);
  assert.deepEqual(
    joined.groups.map((group) => [group.key, group.count, group.rows.length]),
    [
      ["today", 2, 2],
      ["undated", 5, 2],
      ["done", 1, 1],
    ],
  );
  assert.equal(joined.next_offset, null);
  assert.equal(first.groups[1].rows.length, 1, "the first page is not changed");
});

test("a new task id is 32 lowercase hexadecimal characters", () => {
  const id = newId();
  assert.match(id, /^[0-9a-f]{32}$/);
  assert.notEqual(id, newId());
  assert.equal(newId((bytes) => bytes.fill(255)), "f".repeat(32));
});
