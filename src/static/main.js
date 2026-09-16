/**
 * Entry point: wires the modules together and owns the game's session state.
 */

import { ApiError, fetchCharacterNames, fetchPuzzleDay, submitGuess } from "./api.js";
import { createAuth } from "./auth.js";
import { createAutocomplete } from "./autocomplete.js";
import { renderGuess, renderHeadings, renderHistory, renderLegend, rowAnimationMs } from "./board.js";
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
  banner: byId("win-banner"),
  bannerDetail: byId("win-banner-detail"),
  legendItems: byId("legend-items"),
};

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
});

/** @type {string[]} */
let characterNames = [];
let busy = false;

// Both stay undefined until `/day` answers and `restoreBoard` can bind
// persistence to a real puzzle day; the handlers below check before reading.
/** @type {ReturnType<typeof createStorage>|undefined} */
let store;
/** @type {{version: number, guesses: object[], solved: boolean}|undefined} */
let state;

const autocomplete = createAutocomplete({
  input: el.guessInput,
  list: el.characterOptions,
  getNames: () => characterNames,
});

/**
 * The server looks guesses up by exact string (`cache.get(&guess)`), so
 * "kaladin" would be rejected as an unrecognised character. Resolving against
 * the known list first lets players type without matching capitalisation.
 * @param {string} typed
 * @returns {string}
 */
function resolveName(typed) {
  const lower = typed.toLowerCase();
  return characterNames.find((name) => name.toLowerCase() === lower) ?? typed;
}

/** @param {string} name @returns {boolean} */
function alreadyGuessed(name) {
  const lower = name.toLowerCase();
  return state.guesses.some((guess) => guess.character.name.toLowerCase() === lower);
}

function renderSolvedState() {
  if (!state.solved) return;

  const answer = state.guesses.find((guess) => guess.name === "Correct");
  const count = state.guesses.length;
  el.bannerDetail.textContent =
    `${answer?.character.name ?? "The character"} — solved in ` +
    `${count} ${count === 1 ? "guess" : "guesses"}.`;
  el.banner.hidden = false;

  el.guessInput.disabled = true;
  el.guessSubmit.disabled = true;
  el.guessInput.placeholder = "Solved — come back tomorrow";
}

async function onSubmit(event) {
  event.preventDefault();
  if (state === undefined || busy || state.solved) return;

  const typed = el.guessInput.value.trim();
  if (typed === "") return;

  const name = resolveName(typed);
  if (alreadyGuessed(name)) {
    showToast(`You have already guessed ${name}.`);
    return;
  }

  busy = true;
  el.guessInput.value = "";
  el.guessInput.disabled = true;
  el.guessSubmit.disabled = true;
  autocomplete.close();

  try {
    const result = await submitGuess(name);

    state.guesses.push(result);
    if (result.name === "Correct") state.solved = true;
    store.save(state);

    renderGuess(el.board, result, true);
    await new Promise((resolve) => window.setTimeout(resolve, rowAnimationMs));
    renderSolvedState();
  } catch (error) {
    showToast(error instanceof ApiError ? error.message : "Could not submit that guess.");
  } finally {
    busy = false;
    if (!state.solved) {
      el.guessInput.disabled = false;
      el.guessSubmit.disabled = false;
      el.guessInput.focus();
    }
  }
}

/**
 * Typing a letter anywhere on the page jumps to the guess box. Skipped while
 * the player is in any field, while the auth dialog is open, and for shortcuts.
 * @param {KeyboardEvent} event
 */
function onGlobalKeydown(event) {
  if (event.ctrlKey || event.metaKey || event.altKey) return;
  if (state === undefined || auth.isOpen() || state.solved) return;
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
 * Opens persistence against the server's puzzle day and puts that day's board
 * back on screen.
 *
 * The day has to come off the wire — only the server knows which puzzle it is
 * currently answering — so the board cannot be restored until `/day` returns.
 * Until then the guess controls stay disabled: submitting into an unknown day
 * would file the result under the wrong key.
 */
async function restoreBoard() {
  /** @type {string|null} */
  let day = null;

  try {
    day = await fetchPuzzleDay();
  } catch (error) {
    showToast(
      error instanceof ApiError
        ? `Could not confirm today's puzzle: ${error.message}`
        : "Could not confirm today's puzzle.",
    );
  }

  store = createStorage(day);
  state = store.load();
  store.prune();

  renderHistory(el.board, state.guesses);

  if (state.solved) {
    renderSolvedState();
  } else {
    el.guessInput.disabled = false;
    el.guessSubmit.disabled = false;
    el.guessInput.focus();
  }
}

function init() {
  renderHeadings(el.boardHead);
  renderLegend(el.legendItems);

  // Held until `restoreBoard` has a day to file guesses under. The listeners
  // go on now rather than afterwards so that a submit arriving during the
  // round trip is swallowed here instead of navigating the page.
  el.guessInput.disabled = true;
  el.guessSubmit.disabled = true;

  el.guessForm.addEventListener("submit", onSubmit);
  document.addEventListener("keydown", onGlobalKeydown);

  restoreBoard();
  loadCharacters();
  auth.refresh();
}

init();
