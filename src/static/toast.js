/**
 * Transient notifications.
 *
 * Messages are announced to screen readers through two persistent live
 * regions: errors are assertive, confirmations are polite. Inserting a
 * `role="alert"` node into a single region tends to double-announce, so the
 * regions are declared in the markup and only their children change.
 *
 * Message text is always set with `textContent`. Some of what lands here is
 * server-controlled (error bodies), so it must never be parsed as HTML.
 */

const VISIBLE_MS = 4000;
const FADE_MS = 300;

/** @type {{error: HTMLElement, success: HTMLElement}|null} */
let regions = null;

/**
 * @param {{assertive: HTMLElement, polite: HTMLElement}} elements
 */
export function initToasts({ assertive, polite }) {
  regions = { error: assertive, success: polite };
}

/**
 * @param {string} message
 * @param {"error"|"success"} [variant]
 */
export function showToast(message, variant = "error") {
  if (regions === null) {
    console.warn("showToast called before initToasts:", message);
    return;
  }

  const toast = document.createElement("div");
  toast.className = `toast toast--${variant}`;
  toast.textContent = message;

  const region = regions[variant] ?? regions.error;
  region.appendChild(toast);

  window.setTimeout(() => {
    toast.classList.add("toast--leaving");
    window.setTimeout(() => toast.remove(), FADE_MS);
  }, VISIBLE_MS);
}
