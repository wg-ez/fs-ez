/* Helpers DOM partages par les scripts fs-ez, namespace stable `window.fsEz`, ES5 uniquement. */
(function () {
  "use strict";

  var fsEz = window.fsEz || {};

  /* Retourne le premier noeud correspondant, `root` vaut `document` par defaut. */
  function qs(selector, root) {
    return (root || document).querySelector(selector);
  }

  /* Retourne un vrai tableau, compatible `forEach`/`indexOf` sans conversion NodeList. */
  function qsa(selector, root) {
    return Array.prototype.slice.call(
      (root || document).querySelectorAll(selector)
    );
  }

  /* Parcourt tableau ou NodeList, l'index est passe en second argument. */
  function each(list, fn) {
    if (!list) {
      return list;
    }
    for (var index = 0; index < list.length; index += 1) {
      fn(list[index], index);
    }
    return list;
  }

  /* JSON localStorage tolerant, objets seuls, fallback si absent ou corrompu. */
  function readJson(key, fallback) {
    try {
      var raw = window.localStorage.getItem(key);
      var parsed = raw ? JSON.parse(raw) : null;
      if (parsed && typeof parsed === "object") {
        return parsed;
      }
    } catch (error) {}
    return fallback;
  }

  /* Ecrit du JSON localStorage sans jamais lever, quota ou mode prive ignores. */
  function writeJson(key, value) {
    try {
      window.localStorage.setItem(key, JSON.stringify(value));
    } catch (error) {}
  }

  /* Supprime une cle localStorage sans jamais lever. */
  function removeKey(key) {
    try {
      window.localStorage.removeItem(key);
    } catch (error) {}
  }

  /* Focus le champ `name` du formulaire, selection texte optionnelle, retourne l'input. */
  function focusName(form, selectAll) {
    var input = form ? form.querySelector("input[name='name']") : null;
    if (!input) {
      return null;
    }
    input.focus();
    if (selectAll) {
      input.select();
    }
    return input;
  }

  /* Masque tous les formulaires `selector` sauf `except`. */
  function hideForms(selector, except) {
    each(qsa(selector), function (form) {
      if (form !== except) {
        form.hidden = true;
      }
    });
  }

  /* Paramètres de liste de l'URL courante, la vue est déduite du chemin. */
  function preservedParams() {
    var params = new URLSearchParams(window.location.search);
    var match = window.location.pathname.match(/^\/views\/(\d+)$/);
    if (match && params.get("view") === null) {
      params.set("view", match[1]);
    }
    return params;
  }

  /* GET HTML partage : meme origine, `Accept` text/html, `signal` optionnel, rejette si statut non OK. */
  function fetchHtml(url, options) {
    var init = {
      credentials: "same-origin",
      headers: { Accept: "text/html" }
    };
    if (options && options.signal) {
      init.signal = options.signal;
    }
    return window.fetch(url, init).then(function (response) {
      if (!response.ok) {
        throw new Error("html request failed");
      }
      return response.text();
    });
  }

  /* POST partage : meme origine, `headers`/`body` fournis par l'appelant, ne rejette jamais sur statut. */
  function post(url, options) {
    options = options || {};
    return window.fetch(url, {
      method: "POST",
      credentials: "same-origin",
      headers: options.headers || {},
      body: options.body
    });
  }

  fsEz.qs = qs;
  fsEz.qsa = qsa;
  fsEz.each = each;
  fsEz.readJson = readJson;
  fsEz.writeJson = writeJson;
  fsEz.removeKey = removeKey;
  fsEz.focusName = focusName;
  fsEz.hideForms = hideForms;
  fsEz.preservedParams = preservedParams;
  fsEz.fetchHtml = fetchHtml;
  fsEz.post = post;
  window.fsEz = fsEz;
})();
