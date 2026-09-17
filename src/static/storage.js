/**
 * Per-day, per-player persistence of the board in localStorage.
 *
 * Anything read back out of here is treated as untrusted input. It is not
 * signed, the player can edit it, and any other script that ever runs on this
 * origin can write it — so every entry is re-validated against the shape the
 * server documents before it is allowed near the renderer.
 *
 * Which day a saved board belongs to is the server's call, not the browser's.
 * The server rolls the answer over on `Local::now()` (src/answer.rs) and
 * reports the current day from `/day`; this module just files state under
 * whatever it is told, and the caller does not open a board at all until
 * `/day` has answered. Computing the day in the browser would put the board
 * and the answer on different days for part of each day for any player in
 * another timezone: a solved puzzle would reappear unsolved, or a fresh
 * puzzle would open with yesterday's guesses on it.
 *
 * Boards are also filed per player. A browser is shared, the server records
 * guesses against the account (`guess.user_id`), and localStorage is not
 * cleared by logging out — so without the account in the key the next player
 * to log in on this machine inherits the last one's board.
 */

import { isValidGuessResult } from "./api.js";

const KEY_PREFIX = "cosmeredle:state:";
const SCHEMA_VERSION = 1;

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
 * Binds persistence to one puzzle day and one player for as long as that
 * pairing is on screen.
 *
 * Freezing the day here rather than recomputing it per call is deliberate: the
 * board on screen belongs to the day it was loaded for, and a tab left open
 * across a rollover should not start writing half of that board into the next
 * day's slot. Logging in or out replaces the whole object instead.
 *
 * @param {string} day The day from `/day`, `YYYY-MM-DD`.
 * @param {string|null} username The signed-in player, or null for a guest.
 *   The two namespaces cannot collide: a guest's board is keyed `guest` and an
 *   account's `user:<name>`.
 * @returns {{day: string, load: function, save: function, prune: function}}
 */
export function createStorage(day, username) {
  const store = storage();
  const dayPrefix = `${KEY_PREFIX}${day}:`;
  const key = dayPrefix + (username === null ? "guest" : `user:${username}`);

  return {
    day,

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
     *
     * Only whole days go: every board filed under today survives, including
     * the other accounts' on a shared browser, which are still theirs to come
     * back to until the day rolls over.
     */
    prune() {
      if (store === null) return;

      try {
        const stale = [];
        for (let i = 0; i < store.length; i += 1) {
          const k = store.key(i);
          if (k !== null && k.startsWith(KEY_PREFIX) && !k.startsWith(dayPrefix)) {
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
