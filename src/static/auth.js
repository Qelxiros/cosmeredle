/**
 * Sign-in / sign-up dialog and the header's session display.
 *
 * The dialog is a native `<dialog>` opened with `showModal()`, which supplies
 * the focus trap, the Escape handler, the inert background and the backdrop.
 *
 * Authentication is a signed session cookie.
 */

import { ApiError, authenticate, fetchCurrentUser, logout } from "./api.js";
import { showToast } from "./toast.js";

/**
 * Shortest password the sign-up form will submit. A convenience for the
 * player, not a control: the server does not check length, so a direct POST
 * bypasses this.
 */
const MIN_PASSWORD_LENGTH = 8;

/**
 * @param {object} elements
 * @param {HTMLDialogElement} elements.dialog
 * @param {HTMLFormElement} elements.form
 * @param {HTMLElement} elements.title
 * @param {HTMLButtonElement} elements.submit
 * @param {HTMLInputElement} elements.username
 * @param {HTMLInputElement} elements.password
 * @param {HTMLElement} elements.signedOut Container for the log in / sign up buttons.
 * @param {HTMLElement} elements.signedIn Container for the username and log out.
 * @param {HTMLElement} elements.usernameDisplay
 * @param {HTMLButtonElement} elements.loginButton
 * @param {HTMLButtonElement} elements.signupButton
 * @param {HTMLButtonElement} elements.logoutButton
 * @param {HTMLButtonElement} elements.closeButton
 * @param {(session: {username: string, guesses: string[]}|null) => void} elements.onSession
 *   Called whenever the signed-in player changes, including the initial
 *   cookie check. The board is filed per player, so it has to move with this.
 */
export function createAuth(elements) {
  /** @type {"login"|"signup"} */
  let mode = "login";
  /** @type {{username: string, guesses: string[]}|null} */
  let session = null;

  function renderSession() {
    const signedIn = session !== null;
    elements.signedOut.hidden = signedIn;
    elements.signedIn.hidden = !signedIn;
    elements.usernameDisplay.textContent = session?.username ?? "";
  }

  /** @param {{username: string, guesses: string[]}|null} next */
  function setSession(next) {
    session = next;
    renderSession();
    elements.onSession(session);
  }

  /** @returns {Promise<{username: string, guesses: string[]}|null>} */
  async function readSession() {
    try {
      return await fetchCurrentUser();
    } catch {
      // A failed session check should not block the game.
      return null;
    }
  }

  function open(nextMode) {
    mode = nextMode;
    const isLogin = mode === "login";

    elements.title.textContent = isLogin ? "Log In" : "Sign Up";
    elements.submit.textContent = isLogin ? "Log In" : "Create Account";
    elements.password.autocomplete = isLogin ? "current-password" : "new-password";
    elements.password.minLength = isLogin ? 0 : MIN_PASSWORD_LENGTH;

    elements.form.reset();
    elements.dialog.showModal();
    elements.username.focus();
  }

  function close() {
    elements.dialog.close();
  }

  async function submit(event) {
    event.preventDefault();

    const username = elements.username.value.trim();
    const password = elements.password.value;
    if (username === "" || password === "") return;

    elements.submit.disabled = true;
    try {
      const name = await authenticate(mode, { username, password });
      // `/login` and `/signup` only echo the name back; `/me` is what also
      // carries the guesses the server has for this account today.
      setSession((await readSession()) ?? { username: name, guesses: [] });
      close();
      showToast(
        mode === "login" ? "Logged in successfully." : "Account created.",
        "success",
      );
    } catch (error) {
      showToast(describeAuthError(error, mode));
    } finally {
      elements.submit.disabled = false;
    }
  }

  /**
   * A duplicate username comes back as a bodyless 409, so that case is named
   * here rather than shown as a bare status code.
   * @param {unknown} error
   * @param {"login"|"signup"} attemptedMode
   * @returns {string}
   */
  function describeAuthError(error, attemptedMode) {
    if (!(error instanceof ApiError)) return "Something went wrong. Try again.";
    if (attemptedMode === "signup" && error.status === 409) {
      return "That username is already taken.";
    }
    return error.message;
  }

  async function signOut() {
    elements.logoutButton.disabled = true;
    try {
      await logout();
      setSession(null);
      showToast("Logged out.", "success");
    } catch (error) {
      showToast(error instanceof ApiError ? error.message : "Could not log out.");
    } finally {
      elements.logoutButton.disabled = false;
    }
  }

  /** Reads the existing session cookie, if any, on page load. */
  async function refresh() {
    setSession(await readSession());
  }

  elements.loginButton.addEventListener("click", () => open("login"));
  elements.signupButton.addEventListener("click", () => open("signup"));
  elements.logoutButton.addEventListener("click", signOut);
  elements.closeButton.addEventListener("click", close);
  elements.form.addEventListener("submit", submit);

  // A click landing on the dialog element itself is a click on the backdrop;
  // clicks inside the content are retargeted to their own element.
  elements.dialog.addEventListener("click", (event) => {
    if (event.target === elements.dialog) close();
  });

  elements.dialog.addEventListener("close", () => elements.form.reset());

  renderSession();

  return { refresh, isOpen: () => elements.dialog.open, getSession: () => session };
}
