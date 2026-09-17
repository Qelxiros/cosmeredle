/**
 * Every call to the server lives here.
 *
 * Three things this centralises:
 *
 *  - Response decoding. The server is not consistent about content types:
 *    `/me` returns a JSON object while `/login` and `/signup` return a bare
 *    `String`, which axum serves as text/plain, so each response is decoded
 *    according to what the route actually sends.
 *
 *  - Errors. `Error::User` is a 400 with a useful plain-text body, and the
 *    500 path has its own message; both are surfaced to the player rather
 *    than a bare status code.
 *
 *  - Rate limiting. Every POST goes through `tower_governor` (see main.rs),
 *    so a 429 is an ordinary outcome rather than a fault, and the wait it
 *    asks for is parsed off the response for the caller to honour.
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
   * @param {number} [retryAfterMs] How long the server asked us to wait, on a 429.
   */
  constructor(message, status, retryAfterMs) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.retryAfterMs = retryAfterMs;
  }
}

/** Longest wait we will take from a server header at face value. */
const MAX_RETRY_AFTER_MS = 30_000;

/**
 * `tower_governor` answers a rate-limited request with `x-ratelimit-after`;
 * `retry-after` is the standard spelling and is read as a fallback. Both are
 * whole seconds.
 * @param {Response} response
 * @returns {number|undefined} Milliseconds to wait, if the server named one.
 */
function retryAfterMs(response) {
  for (const header of ["x-ratelimit-after", "retry-after"]) {
    const raw = response.headers.get(header);
    const seconds = raw === null ? Number.NaN : Number(raw.trim());
    if (Number.isFinite(seconds) && seconds >= 0) {
      return Math.min(seconds * 1000, MAX_RETRY_AFTER_MS);
    }
  }
  return undefined;
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

  return new ApiError(
    body || `Request failed (${response.status})`,
    response.status,
    retryAfterMs(response),
  );
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
 * The signed-in player and the guesses the server has recorded for them today,
 * or null when the session cookie is absent or expired — `/me` is behind
 * `login_required!` and answers 401 in that case, which is not an error worth
 * surfacing to the player.
 *
 * `/me` sends `server::User`, an object; reading it as a bare string (which is
 * what the route used to send) leaves every signed-in player looking signed
 * out. The guess list is `db::get_guesses`, scoped to the server's current day.
 * @returns {Promise<{username: string, guesses: string[]}|null>}
 */
export async function fetchCurrentUser() {
  let body;
  try {
    const response = await request("/me");
    body = await response.json();
  } catch (error) {
    if (error instanceof ApiError && error.status === 401) {
      return null;
    }
    throw error;
  }

  if (body === null || typeof body !== "object") return null;
  if (typeof body.username !== "string" || body.username === "") return null;

  return {
    username: body.username,
    guesses: Array.isArray(body.guesses)
      ? body.guesses.filter((guess) => typeof guess === "string")
      : [],
  };
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
