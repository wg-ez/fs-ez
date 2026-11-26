/* Recherche globale : l'URL est l'unique état, le DOM et le badge n'en sont qu'un miroir. */
(function () {
  "use strict";

  var qs = window.fsEz.qs;
  var qsa = window.fsEz.qsa;
  var each = window.fsEz.each;
  var preservedParams = window.fsEz.preservedParams;
  var navigateWork = window.fsEz.navigateWork;
  var normalizeTag = window.fsEz.normalizeTag;
  var activeTagNames = window.fsEz.activeTagNames;
  var form = qs(".topbar__search");
  if (!form) {
    return;
  }
  var details = qs(".searchbar__filters", form);
  if (!details) {
    return;
  }
  var input = qs("input[name='q']", form);
  var panel = qs(".searchbar__panel", details);
  var badge = qs(".searchbar__badge", details);
  var all = qs(".searchbar__all", details);

  function rebind() {
    panel = qs(".searchbar__panel", details);
    badge = qs(".searchbar__badge", details);
    all = qs(".searchbar__all", details);
  }

  function syncQuery() {
    if (input) {
      input.value = new URLSearchParams(window.location.search).get("q") || "";
    }
  }

  /* Le tri et le filtre vivent dans l'URL après htmx : les champs cachés suivent l'URL au submit. */
  function syncPreserved() {
    each(qsa("input[data-search-preserve]", form), function (field) {
      field.parentNode.removeChild(field);
    });
    var params = preservedParams();
    each(["sort", "filter", "view"], function (name) {
      each(params.getAll(name), function (value) {
        var field = document.createElement("input");
        field.type = "hidden";
        field.name = name;
        field.value = value;
        field.setAttribute("data-search-preserve", "");
        form.appendChild(field);
      });
    });
  }

  /* Capture : champs cachés à jour puis navigation partielle, le formulaire natif reste le repli. */
  document.addEventListener(
    "submit",
    function (event) {
      if (event.target !== form) {
        return;
      }
      syncPreserved();
      event.preventDefault();
      navigateWork(form.getAttribute("action") || "/", form);
    },
    true
  );

  function updateBadge(count) {
    if (badge) {
      badge.textContent = String(count);
      badge.hidden = count === 0;
    }
    if (all) {
      all.classList.toggle("is-on", count === 0);
    }
  }

  function excludedTagNames() {
    var names = [];
    each(new URLSearchParams(window.location.search).getAll("not_tag"), function (value) {
      var name = normalizeTag(value);
      if (name && names.indexOf(name) === -1) {
        names.push(name);
      }
    });
    return names;
  }

  function excludedField(box) {
    return box.parentNode ? qs("input[data-not-tag]", box.parentNode) : null;
  }

  /* Blacklist locale : champ caché `not_tag` actif et marque visuelle sur l'entrée. */
  function markExcluded(box, excluded) {
    if (excluded) {
      box.checked = false;
    }
    var field = excludedField(box);
    if (field) {
      field.disabled = !excluded;
    }
    if (excluded) {
      box.setAttribute("data-excluded", "");
    } else {
      box.removeAttribute("data-excluded");
    }
    var label = box.closest(".dropdown__item");
    if (label) {
      label.classList.toggle("dropdown__item--excluded", excluded);
    }
  }

  function tagCheckbox(target) {
    if (!(target instanceof Element)) {
      return null;
    }
    var box = target.closest("input[name='tag']");
    if (box) {
      return box;
    }
    var label = target.closest(".dropdown__item");
    return label ? qs("input[name='tag']", label) : null;
  }

  function selectionCount() {
    return (
      qsa("input[name='tag']:checked", panel).length +
      qsa("input[data-not-tag]:not([disabled])", panel).length
    );
  }

  function syncFromLocation() {
    if (!panel) {
      return;
    }
    var tags = activeTagNames();
    var excluded = excludedTagNames();
    each(qsa("input[name='tag']", panel), function (box) {
      var name = normalizeTag(box.value);
      var excludedHere = excluded.indexOf(name) !== -1;
      box.checked = !excludedHere && tags.indexOf(name) !== -1;
      markExcluded(box, excludedHere);
    });
    updateBadge(tags.length + excluded.length);
  }

  function submit() {
    if (typeof form.requestSubmit === "function") {
      form.requestSubmit();
    } else {
      form.submit();
    }
  }

  function close() {
    each(qsa(".searchbar__filters[open]"), function (menu) { menu.open = false; });
  }

  /* Le panneau est rechargé seulement à l'ouverture : les tags ont pu changer depuis le rendu. */
  function refresh() {
    if (!details.open || !panel || !window.htmx) {
      return;
    }
    var url = details.getAttribute("data-search-filter-url");
    if (!url) {
      return;
    }
    window.htmx.ajax("GET", url + window.location.search, {
      target: panel,
      swap: "outerHTML"
    });
  }

  document.addEventListener("change", function (event) {
    var target = event.target;
    if (
      !(target instanceof Element) ||
      !form.contains(target) ||
      !target.matches("input[name='tag']")
    ) {
      return;
    }
    if (target.checked) {
      markExcluded(target, false);
    }
    updateBadge(selectionCount());
    submit();
  });

  document.addEventListener("click", function (event) {
    var target = event.target;
    if (!(target instanceof Element)) {
      return;
    }
    var box = tagCheckbox(target);
    if (box && panel && panel.contains(box) && event.altKey) {
      event.preventDefault();
      markExcluded(box, !box.hasAttribute("data-excluded"));
      updateBadge(selectionCount());
      submit();
      return;
    }
    var clearAll = target.closest(".searchbar__all");
    if (clearAll && panel) {
      each(qsa("input[name='tag']", panel), function (item) {
        item.checked = false;
        markExcluded(item, false);
      });
      details.open = false;
      updateBadge(0);
      return;
    }
    if (!target.closest(".searchbar__filters")) {
      close();
    }
  });

  document.addEventListener(
    "toggle",
    function (event) {
      if (
        event.target instanceof Element &&
        event.target.matches(".searchbar__filters") &&
        event.target.open
      ) {
        refresh();
      }
    },
    true
  );

  document.addEventListener("htmx:afterSwap", function () {
    rebind();
    syncFromLocation();
  });
  document.addEventListener("htmx:historyRestore", function () {
    syncQuery();
    syncFromLocation();
  });
  window.addEventListener("popstate", function () {
    syncQuery();
    syncFromLocation();
  });

  syncFromLocation();
})();
