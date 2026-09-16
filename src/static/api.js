/**
 * Every call to the server lives here.
 *
 * Two things this centralises:
 *
 *  - Response decoding. The server is not consistent about content types:
 *    `/me` returns a JSON-encoded string (`Json(user.username)`) while
 *    `/login` and `/signup` return a bare `String`, which axum serves as
 *    text/plain. The old frontend read `/me` with `.text()` and so showed
 *    the username wrapped in literal quote characters.
 *
 *  - Errors. `Error::User` is a 400 with a useful plain-text body, and the
 *    500 path has its own message. The old frontend threw all of that away
 *    and surfaced a bare status code instead.
 *
 * Authentication is a signed session cookie (axum-login + tower-sessions),
 * carried automatically by `credentials: "same-origin"`.
 */

import { isKnownStatus } from "./statuses.js";

/** Longest server error message we will put in front of a user. */
const MAX_ERROR_LENGTH = 200;

export class ApiError extends Error {
  /**
   * @param {string} message
   * @param {number} [status] HTTP status, or undefined for a transport failure.
   */
  constructor(message, status) {
    super(message);
    this.name = "ApiError";
    this.status = status;
  }
}

/**
 * Pulls the server's own error text out of a failed response, falling back to
 * something generic when the body is empty or unreadable.
 * @param {Response} response
 * @returns {Promise<ApiError>}
 */
async function toApiError(response) {
  let body = "";
  try {
    body = (await response.text()).trim();
  } catch {
    // An unreadable body is not itself worth reporting.
  }

  if (body.length > MAX_ERROR_LENGTH) {
    body = `${body.slice(0, MAX_ERROR_LENGTH)}…`;
  }

  return new ApiError(body || `Request failed (${response.status})`, response.status);
}

/**
 * @param {string} path
 * @param {RequestInit} [options]
 * @returns {Promise<Response>}
 * @throws {ApiError} On a non-2xx response or a network failure.
 */
async function request(path, options = {}) {
  let response;
  try {
    response = await fetch(path, {
      credentials: "same-origin",
      ...options,
    });
  } catch {
    throw new ApiError("Could not reach the server. Check your connection.");
  }

  if (!response.ok) {
    throw await toApiError(response);
  }

  return response;
}

/**
 * @param {string} path
 * @param {unknown} body Serialised as JSON.
 * @returns {Promise<Response>}
 */
function post(path, body) {
  return request(path, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
}

/** The shape `/day` promises: `%Y-%m-%d`, per `server::day`. */
const DAY_PATTERN = /^\d{4}-\d{2}-\d{2}$/;

/**
 * The puzzle day the server is currently serving an answer for.
 *
 * This is authoritative. The browser cannot derive it: the server rolls the
 * answer over on `Local::now()` (src/answer.rs), which is whatever timezone
 * the deployment happens to run in, not the player's and not a fixed zone.
 *
 * Returned as a bare `String`, so text/plain rather than JSON.
 * @returns {Promise<string>} YYYY-MM-DD.
 */
export async function fetchPuzzleDay() {
  const response = await request("/day");
  const day = (await response.text()).trim();

  if (!DAY_PATTERN.test(day)) {
    throw new ApiError("The server reported an unreadable puzzle day.");
  }

  return day;
}

/**
 * Guessable character names, sorted by the server.
 * @returns {Promise<string[]>}
 */
export async function fetchCharacterNames() {
  const response = await request("/list");
  const names = await response.json();

  if (!Array.isArray(names)) {
    throw new ApiError("Character list was malformed.");
  }

  return names.filter((name) => typeof name === "string");
}

/**
 * @param {string} name
 * @returns {Promise<object>} A validated guess result.
 */
export async function submitGuess(name) {
  const response = await post("/guess", name);
  const result = await response.json();

  if (!isValidGuessResult(result)) {
    throw new ApiError("The server returned an unreadable guess result.");
  }

  return result;
}

/**
 * Guards the renderer — and anything replayed out of localStorage — against
 * payloads that are not shaped the way `GuessResponse` says they should be.
 * @param {unknown} value
 * @returns {boolean}
 */
export function isValidGuessResult(value) {
  if (value === null || typeof value !== "object") return false;

  const { name, world, book, species, abilities, character } = value;

  const statusesOk = [name, world, book, species, abilities].every(isKnownStatus);
  if (!statusesOk) return false;

  if (character === null || typeof character !== "object") return false;
  if (typeof character.name !== "string" || character.name === "") return false;
  if (!Array.isArray(character.abilities)) return false;
  if (!character.abilities.every((a) => typeof a === "string")) return false;

  // The remaining character fields are non-optional `String`s server-side, so
  // they may be empty but should never be absent or another type.
  return ["world", "introduced", "species", "nationality", "nation", "ethnicity"].every(
    (key) => typeof character[key] === "string",
  );
}

/**
 * The signed-in user's name, or null when the session cookie is absent or
 * expired. `/me` answers 401 in that case, which is not an error worth
 * surfacing to the player.
 * @returns {Promise<string|null>}
 */
export async function fetchCurrentUser() {
  try {
    const response = await request("/me");
    const username = await response.json();
    return typeof username === "string" ? username : null;
  } catch (error) {
    if (error instanceof ApiError && error.status === 401) {
      return null;
    }
    throw error;
  }
}

/**
 * @param {"login"|"signup"} mode
 * @param {{username: string, password: string}} credentials
 * @returns {Promise<string>} The authenticated username.
 */
export async function authenticate(mode, credentials) {
  const response = await post(mode === "login" ? "/login" : "/signup", credentials);
  return (await response.text()).trim();
}

/** @returns {Promise<void>} */
export async function logout() {
  await request("/logout", { method: "POST" });
}
