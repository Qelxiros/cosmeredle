/**
 * Renders the guess grid and the legend.
 *
 * Everything here builds DOM nodes and assigns `textContent`, never an
 * `innerHTML` template string. Character fields come from Coppermind wiki
 * markup by way of the server's cache, and this same code path re-renders
 * stored state on every page load, so interpolating either into HTML would
 * let a crafted name — or anything able to write to the origin's storage —
 * execute script in the page.
 *
 * The grid is a real ARIA table: a `row` of `columnheader`s over rows of
 * `cell`s. Everything a cell means is in its text — the value, the column
 * name on narrow screens where the header row is hidden, and the status as
 * assistive-only text — so nothing depends on colour or on an `aria-label`
 * that browsers are entitled to drop.
 */

import { COLUMNS, LEGEND, describeStatus } from "./statuses.js";

const CELL_STAGGER_MS = 150;
const FLIP_DURATION_MS = 600;
const EMPTY = "—"; // Character fields are non-optional but may be empty.

/**
 * Evaluated per call rather than once: the player can flip the system setting,
 * or move the window to a display with a different one, without reloading.
 */
const reducedMotion = () => window.matchMedia("(prefers-reduced-motion: reduce)").matches;

/**
 * Builds the species label, e.g. "Human (Alethi)".
 *
 * `species` is blank for most human characters, and the sub-classification
 * lives in whichever of three fields the wiki infobox happened to use.
 * @param {object} character
 * @returns {string}
 */
function speciesLabel(character) {
  const species = character.species || "Human";
  const subspecies = character.nationality || character.nation || character.ethnicity;
  return subspecies ? `${species} (${subspecies})` : species;
}

/**
 * The displayed value for each column, keyed to match `COLUMNS`.
 * @param {object} character
 * @returns {Record<string, string>}
 */
function cellValues(character) {
  return {
    name: character.name,
    world: character.world,
    book: character.introduced,
    species: speciesLabel(character),
    abilities: character.abilities.join(", ") || "Uninvested",
  };
}

/**
 * @param {object} result A guess result validated by `isValidGuessResult`.
 * @param {boolean} animated
 * @returns {HTMLElement}
 */
function buildRow(result, animated) {
  const values = cellValues(result.character);
  const animate = animated && !reducedMotion();

  const row = document.createElement("div");
  row.className = "board__row";
  row.setAttribute("role", "row");

  COLUMNS.forEach((column, index) => {
    const { treatment, label } = describeStatus(result[column.key]);
    const value = values[column.key] || EMPTY;

    const cell = document.createElement("div");
    cell.className = `cell cell--${treatment}`;
    cell.setAttribute("role", "cell");

    if (animate) {
      cell.classList.add("cell--animated");
      cell.style.animationDelay = `${index * CELL_STAGGER_MS}ms`;
    }

    // Shown only on narrow screens, where the heading row is hidden; on wider
    // screens the `columnheader` supplies the same word to assistive tech.
    const labelEl = document.createElement("span");
    labelEl.className = "cell__label";
    labelEl.textContent = column.short;

    const valueEl = document.createElement("span");
    valueEl.className = "cell__value";
    valueEl.textContent = value;

    // Sighted players read the status off the cell's colour and the legend;
    // this is the same information as text, for everyone who cannot.
    const statusEl = document.createElement("span");
    statusEl.className = "visually-hidden";
    statusEl.textContent = label;

    cell.append(labelEl, valueEl, statusEl);
    row.appendChild(cell);
  });

  return row;
}

/**
 * Total time the flip animation needs before a follow-up — the solved banner —
 * should land. Zero when the player has asked for reduced motion, so nothing
 * waits on an animation that is not going to play.
 * @returns {number}
 */
export function rowAnimationMs() {
  return reducedMotion() ? 0 : COLUMNS.length * CELL_STAGGER_MS + FLIP_DURATION_MS;
}

/**
 * A guess as one line of text, for the live region. Rows are appended to the
 * table silently, so without this a screen-reader player has to go hunting
 * through the board after every guess to find out what happened.
 * @param {object} result
 * @returns {string}
 */
export function describeGuess(result) {
  const values = cellValues(result.character);
  return COLUMNS.map((column) => {
    const { label } = describeStatus(result[column.key]);
    // Full stops rather than commas: an ability list is itself comma-separated.
    return `${column.heading}: ${values[column.key] || EMPTY}. ${label}.`;
  }).join(" ");
}

/**
 * @param {HTMLElement} container
 * @param {object} result
 * @param {boolean} animated
 */
export function renderGuess(container, result, animated) {
  // Newest guess on top.
  container.insertBefore(buildRow(result, animated), container.firstChild);
}

/**
 * @param {HTMLElement} container
 * @param {object[]} results In the order they were guessed.
 */
export function renderHistory(container, results) {
  container.replaceChildren();
  results.forEach((result) => renderGuess(container, result, false));
}

/**
 * Column headings, generated so they cannot drift from `COLUMNS`.
 * @param {HTMLElement} head
 */
export function renderHeadings(head) {
  head.replaceChildren(
    ...COLUMNS.map((column) => {
      const cell = document.createElement("div");
      cell.setAttribute("role", "columnheader");
      cell.textContent = column.heading;
      return cell;
    }),
  );
}

/**
 * Legend swatches, generated from the same table the cells are coloured from.
 * @param {HTMLElement} list
 */
export function renderLegend(list) {
  list.replaceChildren(
    ...LEGEND.map((entry) => {
      const item = document.createElement("li");
      item.className = "legend__item";

      const swatch = document.createElement("span");
      swatch.className = `legend__swatch cell--${entry.treatment}`;
      swatch.setAttribute("aria-hidden", "true");

      const text = document.createElement("span");
      text.textContent = entry.text;

      item.append(swatch, text);
      return item;
    }),
  );
}
