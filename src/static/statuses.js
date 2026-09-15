/**
 * The one place guess statuses are described.
 *
 * The server sends three different status enums (see `GuessResponse` in
 * src/server.rs) and the UI has to colour, label and explain all of them.
 * Previously the colours lived in a JS lookup table and the explanations
 * lived in hand-written legend markup, so the two drifted apart freely.
 * Both are now derived from this module.
 *
 *   BinaryStatus  (name, world)     Correct | Incorrect
 *   TernaryStatus (book, species)   Correct | Adjacent | Incorrect
 *   SetStatus     (abilities)       Equal | Overlap | Subset | Superset | Disjoint
 *
 * SetStatus reads as "the answer is a ___ of your guess", so `Superset`
 * means the answer has abilities your guess lacked.
 */

/**
 * Maps every status the server can send to a visual treatment and a short
 * label. The five treatments — `correct`, `partial`, `incorrect`, `fewer`,
 * `more` — each have a matching `.cell--<treatment>` rule in styles.css.
 *
 * The label is rendered into the cell as assistive text, so the meaning of a
 * cell never depends on colour alone.
 */
const STATUSES = {
  // BinaryStatus + TernaryStatus
  Correct: { treatment: "correct", label: "Correct" },
  Adjacent: { treatment: "partial", label: "Close" },
  Incorrect: { treatment: "incorrect", label: "Incorrect" },

  // SetStatus
  Equal: { treatment: "correct", label: "Exact match" },
  Overlap: { treatment: "partial", label: "Some in common" },
  Subset: { treatment: "fewer", label: "Answer has fewer" },
  Superset: { treatment: "more", label: "Answer has more" },
  Disjoint: { treatment: "incorrect", label: "Nothing in common" },
};

const UNKNOWN = { treatment: "incorrect", label: "Unknown" };

/**
 * @param {unknown} status A status string from the server.
 * @returns {{treatment: string, label: string}} Never throws; an unrecognised
 *   status degrades to a neutral cell rather than an unstyled one.
 */
export function describeStatus(status) {
  return (typeof status === "string" && STATUSES[status]) || UNKNOWN;
}

/** @returns {boolean} True if this status string is one the server can send. */
export function isKnownStatus(status) {
  return typeof status === "string" && Object.hasOwn(STATUSES, status);
}

/** Legend entries, in the order they should be shown to the player. */
export const LEGEND = [
  { treatment: "correct", text: "Correct / exact match" },
  { treatment: "partial", text: "Close / some in common" },
  { treatment: "incorrect", text: "Incorrect / nothing in common" },
  { treatment: "fewer", text: "Answer has fewer abilities" },
  { treatment: "more", text: "Answer has more abilities" },
];

/**
 * The five board columns, in display order.
 *
 * `short` is the label printed inside each cell on narrow screens, where the
 * heading row is hidden and a column is only ~70px wide. `heading` is still
 * what the desktop header and every accessible name use.
 */
export const COLUMNS = [
  { key: "name", heading: "Name", short: "Name" },
  { key: "world", heading: "Home World", short: "World" },
  { key: "book", heading: "Introduced In", short: "Book" },
  { key: "species", heading: "Species", short: "Species" },
  { key: "abilities", heading: "Abilities", short: "Abilities" },
];
