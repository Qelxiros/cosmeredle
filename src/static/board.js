/**
 * Renders the guess grid and the legend.
 *
 * Everything here builds DOM nodes and assigns `textContent`. The previous
 * implementation interpolated character fields into an `innerHTML` template
 * string. Those fields come from Coppermind wiki markup by way of the
 * server's cache, and the same code path re-rendered unvalidated localStorage
 * on every page load, so a single crafted name — or anything able to write to
 * the origin's storage — executed script in the page.
 */

import { COLUMNS, LEGEND, describeStatus } from "./statuses.js";

const CELL_STAGGER_MS = 150;
const FLIP_DURATION_MS = 600;
const EMPTY = "—"; // Character fields are non-optional but may be empty.

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

  const row = document.createElement("div");
  row.className = "board__row";
  row.setAttribute("role", "group");
  row.setAttribute("aria-label", `Guess: ${result.character.name}`);

  COLUMNS.forEach((column, index) => {
    const { treatment, label } = describeStatus(result[column.key]);
    const value = values[column.key] || EMPTY;

    const cell = document.createElement("div");
    cell.className = `cell cell--${treatment}`;
    // The column heading is hidden on narrow screens and a div grid conveys no
    // header association anyway, so each cell names its own column and status.
    // This is also what keeps the board meaningful without colour vision.
    cell.setAttribute("aria-label", `${column.heading}: ${value}. ${label}.`);

    if (animated) {
      cell.classList.add("cell--animated");
      cell.style.animationDelay = `${index * CELL_STAGGER_MS}ms`;
    }

    const labelEl = document.createElement("span");
    labelEl.className = "cell__label";
    labelEl.setAttribute("aria-hidden", "true");
    labelEl.textContent = column.short;

    const valueEl = document.createElement("span");
    valueEl.className = "cell__value";
    valueEl.setAttribute("aria-hidden", "true");
    valueEl.textContent = value;

    cell.append(labelEl, valueEl);
    row.appendChild(cell);
  });

  return row;
}

/** Total time the flip animation needs before the next guess should land. */
export const rowAnimationMs = COLUMNS.length * CELL_STAGGER_MS + FLIP_DURATION_MS;

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
