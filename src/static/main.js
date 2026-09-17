/**
 * Entry point: wires the modules together and owns the game's session state.
 */

import {
  ApiError,
  fetchCharacterNames,
  fetchPuzzleDay,
  submitGuess,
} from "./api.js";
import { createAuth } from "./auth.js";
import { createAutocomplete } from "./autocomplete.js";
import {
  describeGuess,
  renderGuess,
  renderHeadings,
  renderHistory,
  renderLegend,
  rowAnimationMs,
} from "./board.js";
import { createStorage } from "./storage.js";
import { initToasts, showToast } from "./toast.js";

/**
 * @param {string} id
 * @returns {HTMLElement} Throws rather than failing silently later if the
 *   markup and the script have drifted apart.
 */
function byId(id) {
  const element = document.getElementById(id);
  if (element === null) throw new Error(`Missing required element #${id}`);
  return element;
}

const el = {
  guessForm: byId("guess-form"),
  guessInput: byId("guess-input"),
  guessSubmit: byId("guess-submit"),
  characterOptions: byId("character-options"),
  board: byId("board"),
  boardHead: byId("board-head"),
  boardStatus: byId("board-status"),
  banner: byId("win-banner"),
  bannerDetail: byId("win-banner-detail"),
  legendItems: byId("legend-items"),
};

const PLACEHOLDER = {
  ready: "Enter character name...",
  loading: "Loading today's puzzle...",
  restoring: "Restoring your board...",
  solved: "Solved — come back tomorrow",
  unavailable: "Unavailable — reload to try again",
};

/** `/day` has to succeed before anything is playable, so it gets more than one go. */
const DAY_ATTEMPTS = 3;
const DAY_RETRY_MS = 1500;
/** POSTs are rate limited server-side; a restore that trips it waits and retries. */
const GUESS_ATTEMPTS = 3;
const RATE_LIMIT_FALLBACK_MS = 4000;
/** How many restores may fail before we stop asking the server for more. */
const RESTORE_FAILURE_LIMIT = 3;

initToasts({ assertive: byId("toast-assertive"), polite: byId("toast-polite") });

const auth = createAuth({
  dialog: byId("auth-dialog"),
  form: byId("auth-form"),
  title: byId("auth-title"),
  submit: byId("auth-submit"),
  username: byId("auth-username"),
  password: byId("auth-password"),
  closeButton: byId("auth-close"),
  signedOut: byId("auth-signed-out"),
  signedIn: byId("auth-signed-in"),
  usernameDisplay: byId("auth-username-display"),
  loginButton: byId("login-button"),
  signupButton: byId("signup-button"),
  logoutButton: byId("logout-button"),
  passwordDialog: byId("password-dialog"),
  passwordForm: byId("password-form"),
  passwordUsername: byId("password-username"),
  passwordCurrent: byId("password-current"),
  passwordNew: byId("password-new"),
  passwordConfirm: byId("password-confirm"),
  passwordSubmit: byId("password-submit"),
  passwordButton: byId("password-button"),
  passwordCloseButton: byId("password-close"),
  onSession: (session) => {
    openBoard(session);
  },
});

/** @type {string[]} */
let characterNames = [];
let busy = false;

/** @type {string|null} The day `/day` reported; null until it has answered. */
let puzzleDay = null;
/** @type {ReturnType<typeof createStorage>|undefined} */
let store;
/** @type {{version: number, guesses: object[], solved: boolean}|undefined} */
let state;
/**
 * Bumped every time a board is opened. A restore started for one player must
 * not write rows into the board of whoever logged in while it was running.
 */
let boardGeneration = 0;

const autocomplete = createAutocomplete({
  input: el.guessInput,
  list: el.characterOptions,
  getNames: () => characterNames,
});

/** @param {number} ms @returns {Promise<void>} */
const delay = (ms) => new Promise((resolve) => window.setTimeout(resolve, ms));

/**
 * @param {boolean} enabled
 * @param {string} placeholder
 */
function setControls(enabled, placeholder) {
  el.guessInput.disabled = !enabled;
  el.guessSubmit.disabled = !enabled;
  el.guessInput.placeholder = placeholder;
  // Never pull focus out from under the auth dialog.
  if (enabled && !auth.isOpen()) el.guessInput.focus();
}

/** Puts the board back in whichever state its guesses call for. */
function settleControls() {
  if (state !== undefined && state.solved) {
    setControls(false, PLACEHOLDER.solved);
  } else {
    setControls(true, PLACEHOLDER.ready);
  }
}

/**
 * The server looks guesses up by exact string (`cache.get(&guess)`), so
 * "kaladin" would be rejected as an unrecognised character. Resolving against
 * the known list first lets players type without matching capitalisation, and
 * catches a typo here — where it can be explained — rather than as the
 * server's generic 500 for a name it cannot find.
 * @param {string} typed
 * @returns {string|null} Null if this is not a character. The typed string is
 *   passed through unchecked when the list failed to load, so a player is
 *   never locked out of the game by `/list` being down.
 */
function resolveName(typed) {
  if (characterNames.length === 0) return typed;
  const lower = typed.toLowerCase();
  return characterNames.find((name) => name.toLowerCase() === lower) ?? null;
}

/** @param {string} name @returns {boolean} */
function alreadyGuessed(name) {
  const lower = name.toLowerCase();
  return state.guesses.some((guess) => guess.character.name.toLowerCase() === lower);
}

/**
 * Rows are appended silently, so the result also goes to a live region.
 * @param {string} message
 */
function announce(message) {
  el.boardStatus.textContent = message;
}

function renderSolvedBanner() {
  const answer = state.guesses.find((guess) => guess.name === "Correct");
  const count = state.guesses.length;
  el.bannerDetail.textContent =
    `${answer?.character.name ?? "The character"} — solved in ` +
    `${count} ${count === 1 ? "guess" : "guesses"}.`;
  el.banner.hidden = false;
}

/**
 * Adds a graded guess to the board and to storage.
 * @param {object} result
 * @param {boolean} live True for a guess the player just made, which is worth
 *   animating and reading out; false when replaying a board, where a row per
 *   announcement would bury the one thing the player is waiting to hear.
 */
function recordGuess(result, live) {
  state.guesses.push(result);
  if (result.name === "Correct") state.solved = true;
  store.save(state);

  renderGuess(el.board, result, live);
  if (live) announce(describeGuess(result));

  if (state.solved) {
    // Held back so the banner does not land on top of the flip that is still
    // revealing the row it is announcing. Nothing waits on this: the input is
    // already disabled for the rest of the day.
    window.setTimeout(renderSolvedBanner, live ? rowAnimationMs() : 0);
  }
}

/**
 * @param {unknown} error
 * @returns {string}
 */
function describeGuessError(error) {
  if (!(error instanceof ApiError)) return "Could not submit that guess.";
  if (error.status === 429) {
    const seconds = Math.ceil((error.retryAfterMs ?? RATE_LIMIT_FALLBACK_MS) / 1000);
    return `Guessing too quickly. Try again in ${seconds} ${seconds === 1 ? "second" : "seconds"}.`;
  }
  return error.message;
}

/**
 * Submits one guess, waiting out the rate limiter rather than failing on it.
 * @param {string} name
 * @param {(waitMs: number) => void} [onWait] Called before each wait, so the
 *   player is told why the board has gone quiet instead of guessing at it.
 * @returns {Promise<object>}
 */
async function submitGuessPatiently(name, onWait) {
  for (let attempt = 1; ; attempt += 1) {
    try {
      return await submitGuess(name);
    } catch (error) {
      const limited = error instanceof ApiError && error.status === 429;
      if (!limited || attempt >= GUESS_ATTEMPTS) throw error;
      const wait = error.retryAfterMs ?? RATE_LIMIT_FALLBACK_MS;
      onWait?.(wait);
      await delay(wait);
    }
  }
}

async function onSubmit(event) {
  event.preventDefault();
  if (state === undefined || busy || state.solved) return;

  const typed = el.guessInput.value.trim();
  if (typed === "") return;

  const name = resolveName(typed);
  if (name === null) {
    showToast(`No Cosmere character called "${typed}".`);
    return;
  }
  if (alreadyGuessed(name)) {
    showToast(`You have already guessed ${name}.`);
    return;
  }

  // Only the button goes down: `busy` is what stops a second submit, and the
  // input has to stay live or letters typed while the request is in flight
  // are dropped on the floor.
  busy = true;
  el.guessInput.value = "";
  el.guessSubmit.disabled = true;
  autocomplete.close();

  let held = false;
  const explainWait = (waitMs) => {
    if (held) return;
    held = true;
    const seconds = Math.ceil(waitMs / 1000);
    showToast(
      `Guessing too quickly — holding ${name} for ${seconds} ${seconds === 1 ? "second" : "seconds"}.`,
    );
  };

  try {
    recordGuess(await submitGuessPatiently(name, explainWait), true);
  } catch (error) {
    showToast(describeGuessError(error));
    // Hand the name back so a failure does not cost the player their typing,
    // unless they have already started typing something else.
    if (el.guessInput.value === "") el.guessInput.value = name;
  } finally {
    busy = false;
    settleControls();
  }
}

/**
 * Typing a letter anywhere on the page jumps to the guess box. Skipped while
 * the player is in any field, while the auth dialog is open, while the board
 * is not accepting guesses, and for shortcuts.
 * @param {KeyboardEvent} event
 */
function onGlobalKeydown(event) {
  if (event.ctrlKey || event.metaKey || event.altKey) return;
  if (state === undefined || auth.isOpen() || state.solved) return;
  if (el.guessInput.disabled) return;
  if (!/^[a-zA-Z]$/.test(event.key)) return;

  const active = document.activeElement;
  if (active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement) return;

  el.guessInput.focus();
}

async function loadCharacters() {
  try {
    characterNames = await fetchCharacterNames();
    autocomplete.refresh();
  } catch (error) {
    showToast(
      error instanceof ApiError
        ? `Could not load the character list: ${error.message}`
        : "Could not load the character list.",
    );
  }
}

/**
 * @returns {Promise<string|null>} The server's puzzle day, or null if it could
 *   not be established. The browser cannot substitute its own: filing a board
 *   under the wrong day shows tomorrow's player yesterday's guesses.
 */
async function loadPuzzleDay() {
  for (let attempt = 1; ; attempt += 1) {
    try {
      return await fetchPuzzleDay();
    } catch (error) {
      if (attempt >= DAY_ATTEMPTS) {
        showToast(
          error instanceof ApiError
            ? `Could not confirm today's puzzle: ${error.message}`
            : "Could not confirm today's puzzle.",
        );
        return null;
      }
      await delay(DAY_RETRY_MS);
    }
  }
}

/**
 * Re-grades the guesses the server has for this account today but this browser
 * does not — a second device, or cleared storage.
 *
 * The server stores guesses as bare names (`db::get_guesses`); the statuses
 * that colour a row are computed per request and never persisted, so the only
 * way to recover a row is to ask for it again. `/guess` is happy to re-answer
 * a name it has already recorded, but it is rate limited, hence the pacing.
 *
 * @param {string[]} serverGuesses
 * @param {number} generation The board this restore belongs to.
 */
async function restoreFromServer(serverGuesses, generation) {
  const known = new Set(state.guesses.map((guess) => guess.character.name.toLowerCase()));
  const missing = serverGuesses.filter((name) => !known.has(name.toLowerCase()));
  if (missing.length === 0) return;

  setControls(false, PLACEHOLDER.restoring);
  let failures = 0;

  for (const name of missing) {
    if (generation !== boardGeneration) return;
    try {
      const result = await submitGuessPatiently(name);
      if (generation !== boardGeneration) return;
      recordGuess(result, false);
    } catch {
      failures += 1;
      if (failures >= RESTORE_FAILURE_LIMIT) break;
    }
  }

  if (generation !== boardGeneration) return;
  if (failures > 0) showToast("Some of your earlier guesses could not be restored.");

  const restored = missing.length - failures;
  if (restored > 0) {
    announce(`Restored ${restored} earlier ${restored === 1 ? "guess" : "guesses"}.`);
  }
  settleControls();
}

/**
 * Puts one player's board for today on screen and binds persistence to it.
 *
 * Called for the initial cookie check and again on every login and logout,
 * because whose board this is changes with the session.
 * @param {{username: string, guesses: string[]}|null} session
 */
async function openBoard(session) {
  // Nothing can be filed until the server has said which day it is answering.
  if (puzzleDay === null) return;

  const generation = (boardGeneration += 1);

  store = createStorage(puzzleDay, session?.username ?? null);
  state = store.load();
  store.prune();

  el.banner.hidden = true;
  renderHistory(el.board, state.guesses);
  if (state.solved) renderSolvedBanner();
  settleControls();

  if (session !== null) await restoreFromServer(session.guesses, generation);
}

async function start() {
  puzzleDay = await loadPuzzleDay();
  if (puzzleDay === null) {
    setControls(false, PLACEHOLDER.unavailable);
    return;
  }

  // Resolves the session, which opens the board for whoever it belongs to.
  await auth.refresh();
}

function init() {
  renderHeadings(el.boardHead);
  renderLegend(el.legendItems);

  // Held until `start` has a day to file guesses under. The listeners go on
  // now rather than afterwards so that a submit arriving during the round trip
  // is swallowed here instead of navigating the page.
  setControls(false, PLACEHOLDER.loading);

  el.guessForm.addEventListener("submit", onSubmit);
  document.addEventListener("keydown", onGlobalKeydown);

  start();
  loadCharacters();
}

init();
