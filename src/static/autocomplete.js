/**
 * Character-name autocomplete, wired as an ARIA combobox.
 *
 * Behaviour changes worth noting against the old implementation:
 *
 *  - Tab is no longer hijacked to cycle suggestions when nothing is
 *    highlighted. Swallowing Tab unconditionally trapped keyboard focus in
 *    the input. Arrow keys cycle; Tab accepts a highlighted suggestion and
 *    otherwise moves focus the way the player expects.
 *  - Suggestions are ranked (exact, then prefix, then word-start, then
 *    substring, then alphabetically) rather than sorted by a comparator that
 *    returned 0 for nearly every pair.
 *  - Options are built with `textContent`, never interpolated into HTML.
 */

const MAX_SUGGESTIONS = 15;

/** Rank buckets; lower sorts first. */
function rank(name, query) {
  const lower = name.toLowerCase();
  if (lower === query) return 0;
  if (lower.startsWith(query)) return 1;
  // Matches the start of any word, e.g. "kal" against "Adolin Kholin".
  if (lower.includes(` ${query}`)) return 2;
  return 3;
}

/**
 * @param {object} options
 * @param {HTMLInputElement} options.input
 * @param {HTMLElement} options.list
 * @param {() => string[]} options.getNames Called on each keystroke so the
 *   combobox picks up the character list as soon as it has loaded.
 */
export function createAutocomplete({ input, list, getNames }) {
  let activeIndex = -1;
  let options = [];

  const isOpen = () => !list.hidden;

  function close() {
    if (!isOpen()) return;
    list.hidden = true;
    list.replaceChildren();
    options = [];
    activeIndex = -1;
    input.setAttribute("aria-expanded", "false");
    input.removeAttribute("aria-activedescendant");
  }

  function setActive(index) {
    if (options.length === 0) return;

    if (activeIndex >= 0) {
      options[activeIndex].setAttribute("aria-selected", "false");
    }

    activeIndex = index;

    if (activeIndex < 0) {
      input.removeAttribute("aria-activedescendant");
      return;
    }

    const option = options[activeIndex];
    option.setAttribute("aria-selected", "true");
    input.setAttribute("aria-activedescendant", option.id);
    option.scrollIntoView({ block: "nearest" });
  }

  function commit(name) {
    input.value = name;
    close();
    input.focus();
  }

  function open(matches) {
    options = matches.map((name, index) => {
      const option = document.createElement("li");
      option.id = `character-option-${index}`;
      option.className = "combobox-list__option";
      option.setAttribute("role", "option");
      option.setAttribute("aria-selected", "false");
      option.textContent = name;
      // mousedown fires before the input's blur, so the click is not lost to
      // the list closing first.
      option.addEventListener("mousedown", (event) => {
        event.preventDefault();
        commit(name);
      });
      return option;
    });

    list.replaceChildren(...options);
    list.hidden = false;
    activeIndex = -1;
    input.setAttribute("aria-expanded", "true");
    input.removeAttribute("aria-activedescendant");
  }

  function refresh() {
    const query = input.value.trim().toLowerCase();
    if (query === "") {
      close();
      return;
    }

    const matches = getNames()
      .filter((name) => name.toLowerCase().includes(query))
      .map((name) => ({ name, rank: rank(name, query) }))
      .sort((a, b) => a.rank - b.rank || a.name.localeCompare(b.name))
      .slice(0, MAX_SUGGESTIONS)
      .map((match) => match.name);

    if (matches.length === 0) {
      close();
      return;
    }

    open(matches);
  }

  input.addEventListener("input", refresh);

  input.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && isOpen()) {
      event.stopPropagation(); // Do not also close the auth dialog.
      close();
      return;
    }

    if (!isOpen() || options.length === 0) return;

    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        setActive((activeIndex + 1) % options.length);
        break;
      case "ArrowUp":
        event.preventDefault();
        setActive((activeIndex - 1 + options.length) % options.length);
        break;
      case "Enter":
      case "Tab":
        if (activeIndex >= 0) {
          event.preventDefault();
          commit(options[activeIndex].textContent);
        }
        break;
      default:
        break;
    }
  });

  // `contains` keeps the list open while the player interacts with it; the old
  // check compared against the list element itself and so closed on any click
  // that landed on a suggestion's padding.
  document.addEventListener("pointerdown", (event) => {
    if (!input.contains(event.target) && !list.contains(event.target)) {
      close();
    }
  });

  return { close, refresh };
}
