/* Navigation partielle de la liste : unique propriétaire du contrat htmx `#work`. */
(function () {
  "use strict";

  var qsa = window.fsEz.qsa;
  var each = window.fsEz.each;

  /* Ancre de plage des tags, le swap de la sidebar remplace les elements et invalide l'ancre. */
  var lastTagLink = null;
  var TAG_LINK_SELECTOR = "#sidebar [data-tag-row] > .sidebar__link[data-tag]";

  document.addEventListener("htmx:afterSwap", function (event) {
    if (event.target instanceof Element && event.target.id === "sidebar") {
      lastTagLink = null;
    }
  });

  /* htmx remplace `#work`, sans htmx le lien navigue ou le formulaire se soumet nativement. */
  function navigateWork(url, source) {
    if (window.fsEz.app && window.fsEz.app.clearFileSelection) {
      window.fsEz.app.clearFileSelection();
    }
    if (window.htmx) {
      window.htmx.ajax("GET", url, {
        source: source,
        target: "#work",
        select: "#work",
        /* La sidebar vient du même rendu serveur, son repère actif suit l'URL sans logique JS dupliquée. */
        selectOOB: "#sidebar:outerHTML",
        swap: "outerHTML",
        /* Chaîne `"true"` : htmx pousse le chemin final de la requête, `true` brut pousserait `/true`. */
        push: "true"
      });
      return;
    }
    if (source) {
      source.submit();
      return;
    }
    window.location.assign(url);
  }

  function normalizeTag(value) {
    return (value || "").trim().toLowerCase();
  }

  /* Noms normalisés et dédupliqués d'un paramètre d'URL (`tag` ou `not_tag`). */
  function tagNames(param) {
    var names = [];
    each(new URLSearchParams(window.location.search).getAll(param), function (value) {
      var name = normalizeTag(value);
      if (name && names.indexOf(name) === -1) {
        names.push(name);
      }
    });
    return names;
  }

  function activeTagNames() {
    return tagNames("tag");
  }

  /* Clic simple = tag exclusif, ctrl/meta bascule, shift etend la plage, alt blackliste. */
  function tagSelectionUrl(link, event) {
    var links = qsa(TAG_LINK_SELECTOR);
    var current = links.indexOf(link);
    var anchor = lastTagLink ? links.indexOf(lastTagLink) : -1;
    var active = tagNames("tag");
    var excluded = tagNames("not_tag");
    var selected = [];
    each(links, function (candidate) {
      if (active.indexOf(normalizeTag(candidate.getAttribute("data-tag"))) !== -1) {
        selected.push(candidate);
      }
    });
    if (event.altKey) {
      var name = normalizeTag(link.getAttribute("data-tag"));
      var excludedAt = excluded.indexOf(name);
      if (excludedAt === -1) {
        excluded.push(name);
        var selectedAt = selected.indexOf(link);
        if (selectedAt !== -1) {
          selected.splice(selectedAt, 1);
        }
      } else {
        excluded.splice(excludedAt, 1);
      }
    } else if (event.shiftKey && anchor !== -1 && current !== -1) {
      var on = selected.indexOf(link) === -1;
      var from = Math.min(anchor, current);
      var to = Math.max(anchor, current);
      for (var index = from; index <= to; index += 1) {
        var position = selected.indexOf(links[index]);
        if (on && position === -1) {
          selected.push(links[index]);
        } else if (!on && position !== -1) {
          selected.splice(position, 1);
        }
      }
    } else if (event.ctrlKey || event.metaKey) {
      var checked = selected.indexOf(link);
      if (checked === -1) {
        selected.push(link);
      } else {
        selected.splice(checked, 1);
      }
    } else {
      selected = [link];
      excluded = [];
    }
    if (!event.altKey) {
      each(selected, function (candidate) {
        var selectedName = normalizeTag(candidate.getAttribute("data-tag"));
        var excludedPosition = excluded.indexOf(selectedName);
        if (excludedPosition !== -1) {
          excluded.splice(excludedPosition, 1);
        }
      });
    }
    lastTagLink = link;
    var url = new URL(link.href, window.location.origin);
    url.searchParams.delete("tag");
    url.searchParams.delete("not_tag");
    each(links, function (candidate) {
      var candidateName = candidate.getAttribute("data-tag") || "";
      if (selected.indexOf(candidate) !== -1) {
        url.searchParams.append("tag", candidateName);
      }
      if (excluded.indexOf(normalizeTag(candidateName)) !== -1) {
        url.searchParams.append("not_tag", candidateName);
      }
    });
    return url.pathname + url.search;
  }

  /* Ctrl/clic, shift/clic et alt/clic selectionnent du texte : on bloque le defaut, comme pour les fichiers. */
  document.addEventListener("mousedown", function (event) {
    var target = event.target;
    if (
      !(target instanceof Element) ||
      !(event.shiftKey || event.ctrlKey || event.metaKey || event.altKey)
    ) {
      return;
    }
    if (target.closest(TAG_LINK_SELECTOR)) {
      event.preventDefault();
    }
  });

  /* Tous les liens de la sidebar rechargent `#work` : le serveur rend tri et filtres dans `href`. */
  function navigateSidebar(event) {
    if (event.defaultPrevented || event.button !== 0) {
      return;
    }
    var target = event.target;
    if (!(target instanceof Element)) {
      return;
    }
    var link = target.closest("#sidebar a.sidebar__link[href]");
    if (!link) {
      return;
    }
    if (link.hasAttribute("data-tag")) {
      event.preventDefault();
      navigateWork(tagSelectionUrl(link, event));
      return;
    }
    if (event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) {
      return;
    }
    event.preventDefault();
    navigateWork(link.getAttribute("href"));
  }

  document.addEventListener("click", navigateSidebar);
  window.fsEz.navigateWork = navigateWork;
  window.fsEz.normalizeTag = normalizeTag;
  window.fsEz.activeTagNames = activeTagNames;
})();
