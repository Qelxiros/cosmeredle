/**
 * Per-day persistence of the player's guesses in localStorage.
 *
 * Anything read back out of here is treated as untrusted input. It is not
 * signed, the player can edit it, and any other script that ever runs on this
 * origin can write it — so every entry is re-validated against the shape the
 * server documents before it is allowed near the renderer. The old code
 * `JSON.parse`d it and fed the result straight into `innerHTML`.
 *
 * Which day a saved board belongs to is the server's call, not the browser's.
 * The server rolls the answer over on `Local::now()` (src/answer.rs) and
 * reports the current day from `/day`; this module just files state under
 * whatever it is told. The previous version computed the day itself in
 * US Eastern, so for any deployment not running in US Eastern — and for every
 * player far enough east or west of it — the board and the answer belonged to
 * different days for part of each day: a solved puzzle would reappear
 * unsolved, or a fresh puzzle would open with yesterday's guesses on it.
 */

import { isValidGuessResult } from "./api.js";

const KEY_PREFIX = "cosmeredle:state:";
/** Last day `/day` reported, so an unreachable server does not lose a board. */
const LAST_DAY_KEY = "cosmeredle:last-day";
const SCHEMA_VERSION = 1;
const DAY_PATTERN = /^\d{4}-\d{2}-\d{2}$/;

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

/**
 * The device's own date, used only when the server has never been reached on
 * this browser. It is a guess at the puzzle day and is treated as one.
 * `en-CA` formats as YYYY-MM-DD, which saves reassembling the parts by hand.
 * @returns {string} YYYY-MM-DD.
 */
function deviceDate() {
  return new Intl.DateTimeFormat("en-CA", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }).format(new Date());
}

/**
 * @param {Storage} store
 * @returns {string|null} The last day the server reported, if it still looks
 *   like a day.
 */
function readLastDay(store) {
  let raw;
  try {
    raw = store.getItem(LAST_DAY_KEY);
  } catch {
    return null;
  }
  return typeof raw === "string" && DAY_PATTERN.test(raw) ? raw : null;
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
 * Binds persistence to one puzzle day for the lifetime of the page.
 *
 * Freezing the day here rather than recomputing it per call is deliberate: the
 * board on screen belongs to the day it was loaded for, and a tab left open
 * across a rollover should not start writing half of that board into the next
 * day's slot.
 *
 * @param {string|null} serverDay The day from `/day`, or null if the request
 *   failed. A null day means state is filed under the last day the server was
 *   known to be serving, falling back to the device's date on a browser that
 *   has never reached the server. Pruning is skipped in that case — deleting
 *   another day's board on a guessed date is the one mistake worth avoiding.
 * @returns {{day: string, trusted: boolean, load: function, save: function, prune: function}}
 */
export function createStorage(serverDay) {
  const store = storage();
  const trusted = serverDay !== null;

  if (store !== null && trusted) {
    try {
      store.setItem(LAST_DAY_KEY, serverDay);
    } catch {
      // Only costs us the fallback on a later offline load.
    }
  }

  const day = serverDay ?? (store !== null ? readLastDay(store) : null) ?? deviceDate();
  const key = KEY_PREFIX + day;

  return {
    day,
    trusted,

    /**
     * Loads this day's state, discarding anything that fails validation.
     * @returns {{version: number, guesses: object[], solved: boolean}}
     */
    load() {
      if (store === null) return emptyState();

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
    },

    /**
     * @param {{version: number, guesses: object[], solved: boolean}} state
     */
    save(state) {
      if (store === null) return;

      try {
        store.setItem(key, JSON.stringify(state));
      } catch {
        // Quota exhausted or storage disabled mid-session; the game still plays.
      }
    },

    /**
     * Drops state from previous puzzle days. Without this every day the player
     * visits leaves another entry behind forever.
     */
    prune() {
      if (store === null || !trusted) return;

      try {
        const stale = [];
        for (let i = 0; i < store.length; i += 1) {
          const k = store.key(i);
          if (k !== null && k.startsWith(KEY_PREFIX) && k !== key) {
            stale.push(k);
          }
        }
        stale.forEach((k) => store.removeItem(k));
      } catch {
        // Best-effort cleanup only.
      }
    },
  };
}
