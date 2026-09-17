/**
 * Sign-in / sign-up and change-password dialogs, and the header's session
 * display.
 *
 * Both dialogs are native `<dialog>`s opened with `showModal()`, which supplies
 * the focus trap, the Escape handler, the inert background and the backdrop.
 *
 * Authentication is a signed session cookie.
 */

import {
  ApiError,
  authenticate,
  changePassword,
  fetchCurrentUser,
  logout,
} from "./api.js";
import { showToast } from "./toast.js";

/**
 * Shortest password either form will submit. `handle_signup` enforces this
 * server-side too; `handle_change_password` does not, so on that form it is a
 * convenience for the player rather than a control, and a direct POST can still
 * set a shorter one.
 */
const MIN_PASSWORD_LENGTH = 8;

/** Assumed wait when a 429 arrives without a header naming one. */
const RATE_LIMIT_FALLBACK_MS = 4000;

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
 * @param {HTMLDialogElement} elements.passwordDialog
 * @param {HTMLFormElement} elements.passwordForm
 * @param {HTMLInputElement} elements.passwordUsername Hidden; for password managers.
 * @param {HTMLInputElement} elements.passwordCurrent
 * @param {HTMLInputElement} elements.passwordNew
 * @param {HTMLInputElement} elements.passwordConfirm
 * @param {HTMLButtonElement} elements.passwordSubmit
 * @param {HTMLButtonElement} elements.passwordButton Opens the dialog.
 * @param {HTMLButtonElement} elements.passwordCloseButton
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

  function openPassword() {
    if (session === null) return;

    elements.passwordForm.reset();
    // After the reset, which would otherwise blank it.
    elements.passwordUsername.value = session.username;
    elements.passwordDialog.showModal();
    elements.passwordCurrent.focus();
  }

  function closePassword() {
    elements.passwordDialog.close();
  }

  /**
   * Changing the password ends the session it was changed from: axum-login
   * stores the bcrypt hash as the session's auth hash and re-checks it on every
   * request (see api.js). Signing straight back in with the new password is
   * what keeps the player on their own board — an anonymous board is a
   * different localStorage key, so being dropped to one looks like today's
   * guesses have been lost.
   */
  async function submitPasswordChange(event) {
    event.preventDefault();

    const username = session?.username;
    if (username === undefined) return;

    const current = elements.passwordCurrent.value;
    const next = elements.passwordNew.value;
    const confirmed = elements.passwordConfirm.value;
    if (current === "" || next === "") return;

    if (next.length < MIN_PASSWORD_LENGTH) {
      showToast(
        `Your new password must be at least ${MIN_PASSWORD_LENGTH} characters.`,
      );
      return;
    }
    if (next !== confirmed) {
      showToast("The two new passwords do not match.");
      return;
    }
    if (next === current) {
      showToast("That is already your password.");
      return;
    }

    elements.passwordSubmit.disabled = true;
    try {
      await changePassword({ current, next });
    } catch (error) {
      showToast(describePasswordError(error));
      return;
    } finally {
      elements.passwordSubmit.disabled = false;
    }

    // Past this point the password *has* changed, so nothing below may report
    // a failure in a way that reads as though it had not.
    closePassword();
    try {
      await authenticate("login", { username, password: next });
      setSession((await readSession()) ?? { username, guesses: [] });
      showToast("Password changed.", "success");
    } catch {
      setSession(null);
      showToast("Password changed. Log in again with your new password.", "success");
    }
  }

  /**
   * A wrong current password comes back as `Error::User`, whose text
   * ("incorrect username or password") is written for `/login` and reads as a
   * non sequitur here, where no username was in question.
   * @param {unknown} error
   * @returns {string}
   */
  function describePasswordError(error) {
    if (!(error instanceof ApiError)) return "Something went wrong. Try again.";
    if (error.status === 400) return "Your current password is incorrect.";
    if (error.status === 401) return "Your session has expired. Log in again.";
    if (error.status === 429) {
      const seconds = Math.ceil((error.retryAfterMs ?? RATE_LIMIT_FALLBACK_MS) / 1000);
      return `Too many requests. Try again in ${seconds} ${seconds === 1 ? "second" : "seconds"}.`;
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

  elements.passwordButton.addEventListener("click", openPassword);
  elements.passwordCloseButton.addEventListener("click", closePassword);
  elements.passwordForm.addEventListener("submit", submitPasswordChange);

  // A click landing on the dialog element itself is a click on the backdrop;
  // clicks inside the content are retargeted to their own element.
  elements.dialog.addEventListener("click", (event) => {
    if (event.target === elements.dialog) close();
  });
  elements.passwordDialog.addEventListener("click", (event) => {
    if (event.target === elements.passwordDialog) closePassword();
  });

  // Also clears the typed passwords out of the DOM on Escape.
  elements.dialog.addEventListener("close", () => elements.form.reset());
  elements.passwordDialog.addEventListener("close", () =>
    elements.passwordForm.reset(),
  );

  renderSession();

  return {
    refresh,
    // The board must not pull focus out of either dialog.
    isOpen: () => elements.dialog.open || elements.passwordDialog.open,
    getSession: () => session,
  };
}
