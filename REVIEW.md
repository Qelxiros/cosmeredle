# Cosmeredle review — `dev` @ `1e453c8`

_Review date: 2026-09-16. Scope: all of `src/`, `migrations/`, `Dockerfile`, `Cargo.toml`, `build.rs`, `tags`, git history. Excludes `target/`._

**Headline: the app is currently non-functional end to end.** Every table in `storage/sqlite.db` is empty (0 users, 0 characters, 0 answers) despite four commits of account work. That is not a coincidence — it is the fingerprint of Blockers 1–4 below. Each was verified by replaying the real schema through SQLite.

**What is genuinely good, so it does not get lost:** every query is a compile-time-checked `query!`/`query_as!` macro with real bind parameters — there is no SQL injection anywhere, including the `json_each($1)` paths. The frontend rewrite is strong: zero `innerHTML`/`insertAdjacentHTML`/`eval` sinks (the only hits are comments describing the old code), a genuinely tight CSP, localStorage re-validated through `isValidGuessResult` before it reaches the DOM, and a properly-built ARIA combobox. No secret is or ever was in git history. Cookie flags are correct by default (`HttpOnly`, `SameSite=Strict`, `Secure`). `cargo check` and clippy are clean.

---

## Blockers

**1. `migrations/0001_init.up.sql:2` — signup can never succeed.**
SQLite aliases the rowid only for the exact spelling `INTEGER PRIMARY KEY`. `id INT NOT NULL PRIMARY KEY` is an ordinary NOT NULL column, so `db.rs:36`'s insert (which omits `id`) always fails. Verified against the live DB: `NOT NULL constraint failed: user.id`. Worse, `server.rs:203` maps *any* `sqlx::Error::Database` to `409 CONFLICT`, so every signup reports "username taken". Needs a table-rebuild migration.

**2. `db.rs:60` — login is impossible even if 1 is fixed.**
`get_user_by_username` uses `INNER JOIN guess`; `get_user` (`db.rs:50`) correctly uses `LEFT JOIN`. A user with no guesses yields zero rows → `Ok(None)` → `401`. The only way to get a guess row is an authenticated `/guess`. Accounts are a closed loop with no entry. Use `LEFT JOIN` — better, do not join guesses in the auth path at all.

**3. `db.rs:269` — `/guess` 500s forever on a fresh install.**
`get_answer` uses `fetch_one`, which returns `RowNotFound` on the empty singleton table, not `""`. `answer.rs:40` propagates it, so `store_answer` is never reached and the table never seeds. This makes the `if !yday.is_empty() { … } else { keys[0] }` branch at `answer.rs:41` dead code. Use `fetch_optional`.

**4. `answer.rs:44-49` — reachable panic, and no `CatchPanicLayer`.**
If `all_characters()` is empty (it is, right now), `keys[0]` is out-of-bounds and `% keys.len()` is divide-by-zero. The path to empty is real: `wiki.rs:167` calls `deactivate_characters` *before* re-inserting, and `deactivate_characters(&[])` runs `… WHERE name NOT IN (SELECT value FROM json_each('[]'))`, deactivating **every** character. With no panic layer the connection just drops.

**5. `answer.rs:32` — every future answer is computable by anyone with the repo.**
Two compounding facts: the fallback `"r4nd0mbyt3s"` is a source literal, and `.ok_or(…)` is unreachable because `OsString::from_str` is `Infallible`. Critically, **`.env` is never loaded at runtime** — there is no `dotenvy` direct dependency; it appears in `Cargo.lock` only as a *build-time* dep of the sqlx macros. So `COSMEREDLE_SECRET=abcd` affects compilation only, and the shipped container always runs on the public literal. With `GET /list`, the whole 4-year schedule falls out offline.

**6. `main.rs:76` — `/guess` needs no auth, no cap, no rate limit.**
`GET /list` returns every name; loop `POST /guess` over it with no cookie and the `"name":"Correct"` response is today's answer. A few hundred requests. There is no guess limit anywhere server-side — `state.solved` and `alreadyGuessed` are both client-side over editable localStorage.

**7. `server.rs:198` — unauthenticated CPU-exhaustion DoS.**
`hash(a.password, 12)` runs synchronously on a tokio worker (~250–400 ms). `backend.rs:37` gets this right with `spawn_blocking`; signup does not — and `handle_signup` then calls `handle_login`, so each signup costs *two* cost-12 bcrypts. Fire N concurrent signups with ~60-byte bodies where N = worker count and the entire server, including the cron and session-reaper, stalls.

**8. `wiki.rs:169` — the sync self-destructs on its second run, silently.**
`insert_character` sets `active = (universe == "Cosmere")`, so non-Cosmere pages land with `active = 0`. But `character_present` filters `AND active`, so it reports them absent, the sync re-fetches and re-`INSERT`s, and the `character.name` PK conflict propagates through `?` — aborting the whole loop at the same character every hour. `main.rs:99`'s `.map(|_| ())` discards the error, so there is no log line at all. Also means wiki edits never propagate to existing rows, and `active` only ever goes false — nothing sets it back. An `INSERT … ON CONFLICT(name) DO UPDATE` fixes all three.

**9. `answer.rs:18,29,54` — the answer is not a function of the date.**
`ANSWER` is process-local, so a restart at 14:00 re-rolls the day's puzzle mid-day; players scored against X all morning, everyone after gets Y. Days with no traffic are silently skipped (`server.rs:128` is the only caller of `today()`). And there is a genuine TOCTOU: the read guard is dropped at `:51` before the write at `:54`, so a request landing between A's `store_answer` and A's cache write reads the *new* value, advances again, and clobbers — two answers on one day. Deriving the answer as a pure function of the date collapses this, the race, and most of the `answer` table away.

**10. `server.rs:117` — one guess burns a character forever.**
The dedupe reads `user.guesses`, which comes from `get_user`'s join with **no day filter**, i.e. every guess ever. Storage is per-day (PK `(user_id, guess, day)`) but the check is not. Guess Kaladin on day 1, get `400 "Already guessed!"` on day 2 and every day after. The correctly day-scoped `db::get_guesses` (`db.rs:85`) exists and has zero callers.

**11. `server.rs:121` vs `:126` — guesses are persisted before they are validated.**
The `spawn` fires at `:121`; `get_character` validates at `:126`. Dropping a `JoinHandle` does not cancel the task, so a typo is written anyway — then `fetch_one`'s `RowNotFound` becomes `500 "Odium's influence has blocked your request"` for what is plainly a `400`. Combined with 10, a misspelling permanently burns that string for the user. This is the single most common failure path in the game.

**12. `Dockerfile` — every container replacement is a full data wipe, and the build context is 3.4 GB.**
`RUN mkdir storage && touch storage/sqlite.db` bakes the DB into the image layer with no `VOLUME` and no compose file, so accounts, guesses and sessions die on every redeploy. Separately there is no `.dockerignore`: verified 11,747 files / 3.4 GB of `target/` — plus `.env` — shipped to the daemon on every build. `.env` is safe today only because the `COPY` lines are selective; one `COPY . .` bakes it in.

---

## Disagreements

- **`main.rs:66` — `Key::try_generate()` mints a new signing key every boot.** Defensible for one hobby instance; it breaks the moment there is a second replica or a rolling deploy, and it makes the `SqliteStore` persistence and the expiry-reaper task pointless. Derive it from the same secret as Blocker 5.
- **`db.rs:14` — SQLite is in rollback-journal mode with a 10-connection pool.** sqlx 0.8 deliberately does not set `journal_mode`, so writers block readers → `SQLITE_BUSY` → 500 under concurrency. Worth correcting: `busy_timeout` *is* already set (5 s default) and foreign keys *are* enforced, so WAL is the only genuine gap.
- **`db.rs:230` — `WHERE universe = "Cosmere"` relies on SQLite's double-quoted-string misfeature.** Confirmed it resolves as a string literal today (`SELECT "NotAColumn"` returns the text). One dependency bump from `no such column: Cosmere` at runtime. Use single quotes.
- **`main.rs:84-88` — layer ordering sends every CSS/JS request through axum-login.** `nest_service("/static", …)` is registered *before* the `.layer()` calls, so each static asset does a session load against SQLite. Session failures also occur outside `TraceLayer` and so are not traced.
- **`board.js:69,78,83` — the board is invisible to screen readers.** Every cell is a role-less `<div>` whose children are both `aria-hidden`, with all meaning in an `aria-label`. ARIA forbids naming generic elements and Chrome prunes those labels, so a blind player gets nothing past the row's `"Guess: Kaladin"`. The comment at `board.js:66` claims this is what makes the board work without colour vision — it is the opposite.
- **`main.js:130` — every guess costs a hard 1350 ms of disabled input.** `styles.css:690` honours `prefers-reduced-motion`, but the JS timer does not. Compounding: `onGlobalKeydown` calls `focus()` on a *disabled* input, so letters typed during that window are silently dropped.
- **The board is never reconciled with the server.** Nothing reads guesses back (no endpoint does), and `storage.js`'s key has no user component — so a second device shows an empty board you cannot re-fill (Blocker 10 rejects the re-guesses), and a shared browser shows the previous user's board after re-login.
- **`storage.js` — `trusted` is computed, returned, documented, and never read.** When `/day` fails, `main.js:196` enables play anyway; guesses get graded against today's answer and written under *yesterday's* key, with `prune()` skipped so the mixed board persists.
- **`server.rs:198`/`backend.rs:38` — bcrypt silently truncates at 72 bytes.** bcrypt 0.19.3 ships `non_truncating_hash`/`non_truncating_verify`; the fix is a one-word change.
- **No server-side password or username validation** (`server.rs:196`). `auth.js:15`'s `minLength` is a client attribute, bypassed by a direct POST. With no rate limiting or lockout, online guessing is viable against every account.
- **Username enumeration** — the `409` is a clean oracle, and `backend.rs:38`'s `Option::filter` skips bcrypt entirely on a miss, giving a 2–3 order-of-magnitude timing gap that survives fixing the status code.
- **`answer.rs:36` — `year().div(4).mul(4)` freezes the permutation for four years,** then reshuffles discontinuously on an untested path that will fire exactly once in production. Reads like a typo either way.
- **`server.rs:141` — `book` is typed `TernaryStatus` but only ever `Correct`/`Incorrect`,** while `statuses.js:59` advertises a "Close" state that column can never show.
- **`Dockerfile:1` — single-stage, runs as root,** shipping rustc, cargo, `sqlx-cli` and the full source to run one binary. Relatedly, `sqlx migrate run` at build time is currently *required* (there is no `.sqlx/`); committing `cargo sqlx prepare` output and setting `SQLX_OFFLINE=true` deletes the slowest layer, the baked DB and the `DATABASE_URL` coupling at once.

---

## Nitpicks

- `auth.js:96` branches on **500** for "name may already be taken", but the server sends **409** with an empty body → the user sees `"Request failed (409)"`, while a real DB outage gets labelled "name may already be taken".
- `lib.rs:65` uses `println!` inside `IntoResponse` while `tracing_subscriber` is initialized at `main.rs:37` — unfilterable, no level, no span. And no `EnvFilter`, so `RUST_LOG` is ignored entirely.
- Seven genuinely unused direct deps: `ctrlc`, `sha2` (you use `sha3`), `bytes`, `futures-core`, `http-body-util`, `include_dir`, `tower`. `ctrlc` present-but-unused suggests graceful shutdown was started and abandoned — there is none, and `deletion_task.await??` at `main.rs:93` is unreachable.
- `Cargo.toml:26` — `sqlx` with default features drags in `sqlx-mysql`/`sqlx-postgres` and with them `rsa`, which carries unfixed RUSTSEC-2023-0071. Unreachable here, but it will light up `cargo audit` forever. Also `_sqlite` is a private internal feature, and `runtime-tokio` is only present by accident of feature unification.
- `server.rs:33` — `read_to_string("src/index.html")` is a blocking read on the async runtime for every `GET /`, and ties the binary to its CWD. `include_str!` is free; `include_dir` is already in `Cargo.toml`, unused, which looks like the abandoned intent.
- `migrations/0002:5-7` and `0003:47-49` are missing the comma between the `FOREIGN KEY` and `PRIMARY KEY` clauses. Confirmed SQLite tolerates it and both constraints *are* created — but that is undefined-by-luck.
- `wiki.rs:30` — `split_once('=').unwrap()` inside a `LazyLock`. Clean today (all 111 lines have `=`), but a panic there *poisons* the lock, so one bad line breaks every request that formats a wiki field until restart. Also `tags:16` is already corrupt: `hemalurgic construct=Hemalurgic constructFor lifeform pages, not individual characters`.
- `wiki.rs:96` — template keys are not trimmed while values are, so `| born = 1173` yields key `" born "`, falls through `Character::add`'s `_ => {}`, and is silently discarded.
- `db.rs:218` — `character_present` swallows errors via `is_ok_and`, so a transient DB error reports "not present" and triggers the Blocker-8 conflict.
- `main.rs:46` — `*Box::pin(update_cache_cron)` is a no-op on a zero-sized `Copy` fn item; it is identical to `update_cache_cron`.
- `statuses.js:48` uses a bare `STATUSES[status]` lookup where `isKnownStatus` two lines below correctly uses `Object.hasOwn` — unreachable today, but inconsistent.
- No `/health`, no `EXPOSE`, no `USER`, hardcoded `0.0.0.0:3000` with no env override, no graceful shutdown.
- **No tests anywhere** (`grep` for `#[test]`/`#[tokio::test]` → zero), no CI, no README, no `rust-toolchain.toml` (Dockerfile pins 1.97.1, local is 1.98.1). A single signup→login integration test would have caught Blockers 1 and 2 — the two that have kept this from ever working.

---

## Checked and clean — do not "fix" these

Three plausible-sounding concerns did not survive verification:

- **`wiki.rs:80-88` brace counting is correct.** `count <= 0` does not fire on the first char (the slice starts at `{{character`, so `count` is 1 before the test), and the slice cannot panic on multibyte input — `str::find` returns a char-boundary offset and the satisfying char is always a 1-byte `}`.
- **`wiki.rs:118`'s `cont = chunk.clone()` continuation loop is correct.** `mediawiki-0.5.1`'s `continue_from` takes the full response and extracts `["continue"]` internally. It neither loops forever nor stops early. (It does clone the whole 500-item payload each iteration, which is wasteful but harmless.)
- **`backend.rs:21`'s `session_auth_hash` is right as written.** It returns the bcrypt hash, so the session invalidates on password change and not otherwise; `guesses` are not part of the hash, so making a guess does not log the user out.

Also verified clean: no XSS sinks, no SQL injection, no path traversal, no secrets in git history, correct cookie flags, and down-migrations that genuinely reverse their ups (0003 drops `ability` before `character`).

---

## Suggested fix order

1. **Blockers 1 → 2 → 3** are hard bootstrap failures; nothing else is even observable until they are done.
2. **Blocker 8** — the sync destroys itself hourly and says nothing.
3. **Blockers 5 + 9 together** — rewrite `today()` as a pure function of the date with a required secret. That collapses 9, the TOCTOU race, the 4-year prefix, and most of the `answer` table into one change.
4. **Blockers 10 + 11** — guess handling.
5. Then 6, 7, 12 before anything is exposed publicly.
