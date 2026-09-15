/**
 * Per-day persistence of the player's guesses in localStorage.
 *
 * Anything read back out of here is treated as untrusted input. It is not
 * signed, the player can edit it, and any other script that ever runs on this
 * origin can write it — so every entry is re-validated against the shape the
 * server documents before it is allowed near the renderer. The old code
 * `JSON.parse`d it and fed the result straight into `innerHTML`.
 *
 * NOTE: the puzzle day is computed here in US Eastern time, while the server
 * rolls the answer over on its own local clock (`Local::now()` in
 * src/answer.rs). If the server is not running in US Eastern the two
 * disagree for part of the day.
 */

import { isValidGuessResult } from "./api.js";

const KEY_PREFIX = "cosmeredle:state:";
const SCHEMA_VERSION = 1;

/** @returns {string} Today's date in America/New_York as YYYY-MM-DD. */
function puzzleDate() {
  // `en-CA` formats as YYYY-MM-DD, which saves reassembling the parts by hand.
  return new Intl.DateTimeFormat("en-CA", {
    timeZone: "America/New_York",
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }).format(new Date());
}

/**
 * localStorage throws rather than returning null when the browser has disabled
 * site data, so every access is guarded. A player with storage switched off
 * gets a working game that simply does not persist across reloads.
 * @returns {Storage|null}
 */
function storage() {
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

const emptyState = () => ({ version: SCHEMA_VERSION, guesses: [], solved: false });

/**
 * @param {unknown} value
 * @returns {{version: number, guesses: object[], solved: boolean}|null}
 */
function parseState(value) {
  if (value === null || typeof value !== "object") return null;
  if (value.version !== SCHEMA_VERSION) return null;
  if (!Array.isArray(value.guesses)) return null;

  const guesses = value.guesses.filter(isValidGuessResult);

  // A partially-valid array means something tampered with or corrupted the
  // entry; keeping the good half would show the player a misleading board.
  if (guesses.length !== value.guesses.length) return null;

  return {
    version: SCHEMA_VERSION,
    guesses,
    solved: value.solved === true,
  };
}

/**
 * Loads today's state, discarding anything that fails validation.
 * @returns {{version: number, guesses: object[], solved: boolean}}
 */
export function loadState() {
  const store = storage();
  if (!store) return emptyState();

  const key = KEY_PREFIX + puzzleDate();
  let raw;
  try {
    raw = store.getItem(key);
  } catch {
    return emptyState();
  }
  if (raw === null) return emptyState();

  let parsed = null;
  try {
    parsed = parseState(JSON.parse(raw));
  } catch {
    parsed = null;
  }

  if (parsed === null) {
    try {
      store.removeItem(key);
    } catch {
      // Nothing useful to do if removal fails too.
    }
    return emptyState();
  }

  return parsed;
}

/**
 * @param {{version: number, guesses: object[], solved: boolean}} state
 */
export function saveState(state) {
  const store = storage();
  if (!store) return;

  try {
    store.setItem(KEY_PREFIX + puzzleDate(), JSON.stringify(state));
  } catch {
    // Quota exhausted or storage disabled mid-session; the game still plays.
  }
}

/**
 * Drops state from previous puzzle days. Without this every day the player
 * visits leaves another entry behind forever.
 */
export function pruneOldState() {
  const store = storage();
  if (!store) return;

  const keepKey = KEY_PREFIX + puzzleDate();
  try {
    const stale = [];
    for (let i = 0; i < store.length; i += 1) {
      const key = store.key(i);
      if (key !== null && key.startsWith(KEY_PREFIX) && key !== keepKey) {
        stale.push(key);
      }
    }
    stale.forEach((key) => store.removeItem(key));
  } catch {
    // Best-effort cleanup only.
  }
}
