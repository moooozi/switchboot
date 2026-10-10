import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import ts from "typescript";

// Run the actual TypeScript modules without adding a test framework or invoking Tauri.
function moduleUrl(path, imports = {}) {
  let code = ts.transpileModule(readFileSync(new URL(path, import.meta.url), "utf8"), {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext },
  }).outputText;
  for (const [specifier, url] of Object.entries(imports)) code = code.replaceAll(`"${specifier}"`, JSON.stringify(url));
  return `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`;
}
const typesUrl = moduleUrl("../src/lib/types.ts");
const storeUrl = moduleUrl("../src/lib/stores/undoRedo.ts", { "svelte/store": import.meta.resolve("svelte/store") });
const { UndoRedoManager, undoRedoStore } = await import(storeUrl);
const { OrderManager } = await import(moduleUrl("../src/lib/orderManager.ts", {
  "./stores/undoRedo": storeUrl, "./types": typesUrl,
}));
const mockInvokeUrl = `data:text/javascript,export const invoke = (...args) => globalThis.invokeFixture(...args);`;
const mockUpdaterUrl = `data:text/javascript,export const check = () => globalThis.updateFixture();`;
const { ApiService } = await import(moduleUrl("../src/lib/api.ts", {
  "@tauri-apps/api/core": mockInvokeUrl, "@tauri-apps/plugin-updater": mockUpdaterUrl,
}));

function deferred() {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
const entry = (id, next = false) => ({ id, description: `Entry ${id}`, is_bootnext: next, is_current: false });

function manager(api) {
  undoRedoStore.clear();
  const entries = [entry(1, true), entry(2)];
  const result = new OrderManager(entries, undefined, undefined, undefined, api);
  result.setDiscoveredEntries(entries);
  return result;
}

test("undo waits for success and rejects concurrent history operations", async () => {
  const store = new UndoRedoManager();
  const pending = deferred();
  let calls = 0;
  store.addChange({ action: "fixture", description: "fixture", undoCommand: () => { calls++; return pending.promise; }, redoCommand() {} });
  const undo = store.undo();
  assert.equal(store.isBusy, true);
  assert.equal(await store.undo(), false);
  assert.equal(await store.redo(), false);
  assert.equal(calls, 1);
  pending.resolve();
  assert.equal(await undo, true);
  assert.equal(store.canUndo(), false);
  assert.equal(store.canRedo(), true);
});

test("failed undo and redo retain the history cursor", async () => {
  const store = new UndoRedoManager();
  let failUndo = true;
  let failRedo = true;
  store.addChange({ action: "fixture", description: "fixture", async undoCommand() { if (failUndo) throw Error("undo failed"); }, async redoCommand() { if (failRedo) throw Error("redo failed"); } });
  await assert.rejects(store.undo(), /undo failed/);
  assert.equal(store.canUndo(), true);
  assert.equal(store.canRedo(), false);
  assert.equal(store.isBusy, false);
  failUndo = false;
  await store.undo();
  await assert.rejects(store.redo(), /redo failed/);
  assert.equal(store.canUndo(), false);
  assert.equal(store.canRedo(), true);
  failRedo = false;
  await store.redo();
  assert.equal(store.canUndo(), true);
});

test("firmware failures propagate without recording or advancing history", async () => {
  let fail = false;
  const api = { async setBootNext() { if (fail) throw Error("firmware failure"); }, async unsetBootNext() {} };
  const order = manager(api);
  fail = true;
  await assert.rejects(order.setBootNext(2), /firmware failure/);
  assert.equal(undoRedoStore.canUndo(), false);
  fail = false;
  await order.setBootNext(2);
  fail = true;
  await assert.rejects(undoRedoStore.undo(), /firmware failure/);
  assert.equal(undoRedoStore.canUndo(), true);
  assert.equal(undoRedoStore.canRedo(), false);
});

test("initial mutations, undo and local reorders cannot overlap", async () => {
  const pending = deferred();
  let calls = 0;
  const order = manager({ setBootNext() { calls++; return pending.promise; } });
  const operation = order.setBootNext(2);
  await order.setBootNext(1);
  order.moveEntry(0, "down");
  assert.deepEqual(order.getBootEntries().map((item) => item.id), [1, 2]);
  assert.equal(await undoRedoStore.undo(), false);
  assert.equal(calls, 1);
  pending.resolve();
  await operation;
  assert.equal(undoRedoStore.canUndo(), true);
});

test("firmware setup no-ops preserve the prior state and real changes are undoable", async () => {
  const calls = [];
  const order = manager({ async setBootToFirmwareSetup() { calls.push(true); }, async unsetBootToFirmwareSetup() { calls.push(false); } });
  order.setFirmwareSetupState(true);
  await order.setBootToFirmwareSetup();
  assert.deepEqual(calls, []);
  await order.unsetBootToFirmwareSetup();
  await undoRedoStore.undo();
  await undoRedoStore.redo();
  assert.deepEqual(calls, [false, true, false]);
});

test("local order snapshots survive add, remove, undo and redo", async () => {
  const order = manager({});
  order.addToBootOrder(entry(3));
  await undoRedoStore.undo();
  assert.deepEqual(order.getBootEntries().map((item) => item.id), [1, 2]);
  await undoRedoStore.redo();
  order.removeFromBootOrder(2);
  await undoRedoStore.undo();
  assert.deepEqual(order.getBootEntries().map((item) => item.id), [1, 2, 3]);
});

test("API read failures are errors, not false firmware state", async () => {
  globalThis.invokeFixture = async () => { throw Error("read failed"); };
  const errors = [], busy = [];
  const api = new ApiService({ onerror: (error) => errors.push(error), onbusychange: (value) => busy.push(value) });
  await assert.rejects(api.getBootToFirmwareSetupState(), /read failed/);
  assert.equal(errors.length, 1);
  assert.deepEqual(busy, [true, false]);
});

test("API remains busy until all overlapping requests finish", async () => {
  const first = deferred(), second = deferred();
  globalThis.invokeFixture = (command) => command === "get_os" ? first.promise : second.promise;
  const busy = [];
  const api = new ApiService({ onbusychange: (value) => busy.push(value) });
  const a = api.getOS(), b = api.getBootToFirmwareSetupState();
  first.resolve("windows");
  await a;
  assert.equal(busy.at(-1), true);
  second.resolve(false);
  await b;
  assert.equal(busy.at(-1), false);
});

test("saving establishes the baseline from the submitted snapshot, not mutable UI state", async () => {
  undoRedoStore.clear();
  const source = readFileSync(new URL("../src/routes/+page.svelte", import.meta.url), "utf8");
  const save = source.slice(source.indexOf("  async function saveOrder()"), source.indexOf("  async function runUiOperation"));
  const pending = deferred();
  let submitted;
  const api = { setBootOrder(order) { submitted = order; return pending.promise; } };
  const harness = new Function("apiService", "undoRedoStore", "entries", `
    let busy = false, changed = true, bootEntries = entries, originalOrder = [], originalEntries = [];
    const runUiOperation = (operation) => operation();
    ${save}
    return { saveOrder, change: (entries) => { bootEntries = entries; }, baseline: () => originalOrder };
  `)(api, undoRedoStore, [entry(1), entry(2)]);
  const saving = harness.saveOrder();
  harness.change([entry(2), entry(1)]);
  pending.resolve();
  await saving;
  assert.deepEqual(submitted, [1, 2]);
  assert.deepEqual(harness.baseline(), [1, 2]);
});
