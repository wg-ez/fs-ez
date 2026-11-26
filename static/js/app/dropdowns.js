/* Menus déroulants, éditeurs inline groupe/tag/vue, état des dossiers et toast. */
(function () {
  "use strict";

  var app = window.fsEz.app || {};
  var qs = window.fsEz.qs;
  var qsa = window.fsEz.qsa;
  var each = window.fsEz.each;
  var readJson = window.fsEz.readJson;
  var writeJson = window.fsEz.writeJson;
  var focusName = window.fsEz.focusName;
  var hideForms = window.fsEz.hideForms;

  function closeMenus(except) {
    each(qsa("details.dropdown[open]"), function (menu) {
      if (menu === except) return;
      /* Un sous-menu ouvert ne referme pas ses ancetres, un parent ne referme pas ses sous-menus. */
      if (except && (except.contains(menu) || menu.contains(except))) return;
      menu.open = false;
    });
  }

  /* Ferme le menu ouvert le plus profond, un Escape ne doit pas fermer l'editeur qui le contient. */
  function closeOpenDropdown() {
    var open = qsa("details.dropdown[open]");
    if (!open.length) {
      return false;
    }
    var deepest = open[0];
    each(open, function (menu) {
      if (deepest.contains(menu)) {
        deepest = menu;
      }
    });
    deepest.open = false;
    var summary = deepest.querySelector(":scope > summary");
    if (summary) {
      summary.focus();
    }
    return true;
  }

  function dropdownNode(tag, className, parent) {
    var node = document.createElement(tag);
    node.className = className;
    if (parent) parent.appendChild(node);
    return node;
  }

  function initDropdown(root) {
    if (root.dataset.dropdownInit) return;
    root.dataset.dropdownInit = "1";
    var summary = root.querySelector(":scope > summary");
    if (!summary) return;
    function sync() {
      summary.setAttribute("aria-expanded", root.open ? "true" : "false");
    }
    root.addEventListener("toggle", function () {
      sync();
      if (root.open) closeMenus(root);
    });
    sync();
  }

  /* select[data-dropdown] devient un panneau partage, le select natif reste pour la soumission. */
  function enhanceSelect(select) {
    if (select.dataset.dropdownReady) return;
    select.dataset.dropdownReady = "1";
    var root = dropdownNode("details", "dropdown");
    var summary = dropdownNode("summary", "input dropdown__trigger", root);
    var value = dropdownNode("span", "dropdown__value dropdown__label", summary);
    var panel = dropdownNode("div", "dropdown__panel dropdown__panel--field", root);
    summary.setAttribute("aria-haspopup", "listbox");
    panel.setAttribute("role", "listbox");
    if (select.id) {
      panel.id = select.id + "-panel";
      summary.setAttribute("aria-controls", panel.id);
    }

    var label = select.labels && select.labels[0];
    if (label) {
      label.id = label.id || (select.id || "dropdown") + "-label";
      summary.setAttribute("aria-labelledby", label.id);
      label.addEventListener("click", function (event) {
        /* Le label enveloppe le select : ne pas intercepter les clics du menu ni de son panneau. */
        if (root.contains(event.target)) return;
        event.preventDefault();
        if (select.disabled) {
          return;
        }
        root.open = true;
        summary.focus();
      });
    } else if (select.getAttribute("aria-label")) {
      summary.setAttribute("aria-label", select.getAttribute("aria-label"));
    }

    summary.addEventListener("click", function (event) {
      if (select.disabled) {
        event.preventDefault();
        summary.focus();
      }
    });

    function sync() {
      var option = select.selectedOptions[0];
      value.textContent = option ? option.textContent.trim() : "";
      summary.classList.toggle("is-disabled", select.disabled);
      summary.setAttribute("aria-disabled", select.disabled ? "true" : "false");
      if (select.disabled) {
        root.open = false;
      }
      panel.querySelectorAll(".dropdown__item").forEach(function (item) {
        var source = select.options[Number(item.dataset.optionIndex)];
        var current = source ? source.value === select.value : false;
        item.classList.toggle("is-on", current);
        item.setAttribute("aria-selected", current);
        item.disabled = source ? source.disabled : false;
      });
    }

    function addItem(option) {
      var item = dropdownNode("button", "dropdown__item", panel);
      item.type = "button";
      item.setAttribute("role", "option");
      item.dataset.value = option.value;
      item.dataset.optionIndex = String(option.index);
      item.textContent = option.textContent.trim() || option.value;
      if (option.lang) item.lang = option.lang;
    }

    Array.from(select.children).forEach(function (child) {
      if (child.tagName === "OPTGROUP") {
        var group = dropdownNode("p", "dropdown__group", panel);
        group.textContent = child.getAttribute("label") || "";
        Array.from(child.children).forEach(addItem);
      } else if (child.tagName === "OPTION") {
        addItem(child);
      }
    });

    panel.addEventListener("click", function (event) {
      var item = event.target.closest(".dropdown__item");
      if (!item || select.disabled) return;
      select.value = item.dataset.value;
      sync();
      root.open = false;
      summary.focus();
      select.dispatchEvent(new Event("change", { bubbles: true }));
    });

    select.parentNode.insertBefore(root, select);
    select.hidden = true;
    select.addEventListener("change", sync);
    initDropdown(root);
    sync();
  }

  function initDropdowns(scope) {
    var root = scope instanceof Element ? scope : document;
    if (root instanceof Element && root.matches("details.dropdown")) {
      initDropdown(root);
    }
    root.querySelectorAll("details.dropdown").forEach(initDropdown);
    root.querySelectorAll("select[data-dropdown]").forEach(enhanceSelect);
  }

  function closeInlineEditor(scopeSelector, flagClass, formSelector, except) {
    each(qsa(scopeSelector), function (scope) {
      if (scope === except) {
        return;
      }
      scope.classList.remove(flagClass);
      each(qsa(formSelector, scope), function (form) {
        form.hidden = true;
      });
    });
  }

  /* Focus le champ nom et selectionne son contenu, commun aux renommages inline. */
  function focusNameAndSelect(form) {
    focusName(form, true);
  }

  /* Focus la pastille selectionnee, sinon la premiere, commun a tous les selecteurs de couleur. */
  function focusSwatch(form) {
    var swatch =
      qs(".sidebar__tag-swatch.is-on", form) ||
      qs(".sidebar__tag-swatch", form);
    if (swatch) {
      swatch.focus();
    }
  }

  /* Squelette commun des editeurs inline : fermeture ciblee, `details`, classe d'etat, formulaire, focus. */
  function openInlineEditor(row, options) {
    if (!row) {
      return;
    }
    options.closers(row);
    if (options.openDetails) {
      row.open = true;
    }
    if (options.flagFirst) {
      row.classList.add(options.flagClass);
    }
    var form = qs(options.formSelector, row);
    if (!form) {
      return;
    }
    if (!options.flagFirst) {
      row.classList.add(options.flagClass);
    }
    form.hidden = false;
    options.focus(form);
  }

  /* Wrappers `close*` identiques : classe d'etat, formulaire inline, `except` epargne. */
  var inlineClosers = {};
  each(
    [
      ["closeTagEditors", ".sidebar__row.is-editing", "is-editing", "[data-tag-edit]"],
      ["closeGroupRename", ".sidebar__group.is-renaming", "is-renaming", "[data-group-rename]"],
      ["closeGroupColor", ".sidebar__group.is-group-coloring", "is-group-coloring", "[data-group-edit='color']"],
      ["closeViewGroupRename", ".sidebar__group.is-view-group-renaming", "is-view-group-renaming", "[data-view-group-rename]"],
      ["closeViewGroupColor", ".sidebar__group.is-view-group-coloring", "is-view-group-coloring", "[data-view-group-edit='color']"],
      ["closeViewRename", ".sidebar__row.is-view-renaming", "is-view-renaming", "[data-view-rename]"],
      ["closeViewColor", ".sidebar__row.is-view-coloring", "is-view-coloring", "[data-view-edit='color']"]
    ],
    function (config) {
      var close = function (except) {
        closeInlineEditor(config[1], config[2], config[3], except);
      };
      inlineClosers[config[0]] = close;
      app[config[0]] = close;
    }
  );

  function openTagEditor(row, kind) {
    openInlineEditor(row, {
      flagClass: "is-editing",
      formSelector: "[data-tag-edit='" + kind + "']",
      closers: function (target) {
        inlineClosers.closeTagEditors(target);
      },
      focus: function (form) {
        if (kind !== "name") {
          focusSwatch(form);
          return;
        }
        var name = qs(".sidebar__link [data-tag-name]", row);
        var input = qs("input[name='name']", form);
        if (input) {
          input.value = name ? name.textContent.trim() : "";
          focusNameAndSelect(form);
        }
      }
    });
  }

  function openGroupRename(group) {
    openInlineEditor(group, {
      flagClass: "is-renaming",
      formSelector: "[data-group-rename]",
      flagFirst: true,
      openDetails: true,
      closers: function (target) {
        inlineClosers.closeGroupRename(target);
        inlineClosers.closeGroupColor(null);
      },
      focus: focusNameAndSelect
    });
  }

  function openGroupColor(group) {
    openInlineEditor(group, {
      flagClass: "is-group-coloring",
      formSelector: "[data-group-edit='color']",
      flagFirst: true,
      openDetails: true,
      closers: function (target) {
        inlineClosers.closeGroupColor(target);
        inlineClosers.closeGroupRename(null);
      },
      focus: focusSwatch
    });
  }

  function closeGroupCreate(except) {
    hideForms("[data-group-create-form]", except);
  }

  function closeViewGroupCreate(except) {
    hideForms("[data-view-group-create-form]", except);
  }

  function openViewGroupCreate(form) {
    if (!form) {
      return;
    }
    closeViewGroupCreate(form);
    form.hidden = false;
    clearAndFocusName(form);
  }

  function openViewGroupRename(group) {
    openInlineEditor(group, {
      flagClass: "is-view-group-renaming",
      formSelector: "[data-view-group-rename]",
      flagFirst: true,
      openDetails: true,
      closers: function (target) {
        inlineClosers.closeViewGroupRename(target);
        inlineClosers.closeViewGroupColor(null);
      },
      focus: focusNameAndSelect
    });
  }

  function openViewGroupColor(group) {
    openInlineEditor(group, {
      flagClass: "is-view-group-coloring",
      formSelector: "[data-view-group-edit='color']",
      flagFirst: true,
      openDetails: true,
      closers: function (target) {
        inlineClosers.closeViewGroupColor(target);
        inlineClosers.closeViewGroupRename(null);
      },
      focus: focusSwatch
    });
  }

  function hideSlots(section) {
    each(qsa("[data-view-create-slot]", section), function (slot) {
      slot.hidden = true;
    });
  }

  function closeViewEditors(except) {
    hideForms("[data-view-editor]", except);
    hideSlots(document);
  }

  function openViewRename(row) {
    openInlineEditor(row, {
      flagClass: "is-view-renaming",
      formSelector: "[data-view-rename]",
      closers: function (target) {
        inlineClosers.closeViewRename(target);
        inlineClosers.closeViewColor(null);
        closeViewEditors(null);
      },
      focus: focusNameAndSelect
    });
  }

  function openViewColor(row) {
    openInlineEditor(row, {
      flagClass: "is-view-coloring",
      formSelector: "[data-view-edit='color']",
      closers: function (target) {
        inlineClosers.closeViewColor(target);
        inlineClosers.closeViewRename(null);
        closeViewEditors(null);
      },
      focus: focusSwatch
    });
  }

  function openViewCreate(form, groupId) {
    if (!form) {
      return;
    }
    inlineClosers.closeViewRename(null);
    closeViewEditors(form);
    var group = qs("input[data-view-group-input]", form);
    if (group) {
      group.value = groupId || "";
    }
    placeViewCreatePanel(form);
    form.hidden = false;
    clearAndFocusName(form);
  }

  /* L'éditeur de création rejoint le groupe cliqué, sinon il reste au-dessus des groupes. */
  function placeViewCreatePanel(form) {
    var panel = form.closest("[data-view-create-panel]") || form;
    var section = panel.closest(".sidebar__section");
    var groupInput = qs("input[data-view-group-input]", form);
    var groupId = groupInput ? groupInput.value : "";
    if (!section) {
      return;
    }
    hideSlots(section);
    if (groupId) {
      var slot = section.querySelector(
        '[data-view-create-slot="' + groupId + '"]'
      );
      if (slot) {
        slot.appendChild(panel);
        slot.hidden = false;
        return;
      }
    }
    var anchor = section.querySelector(
      "[data-view-group-create-form], [data-view-create-anchor]"
    );
    if (anchor) {
      anchor.parentNode.insertBefore(panel, anchor.nextSibling);
    }
  }

  /* Vide puis focus le champ nom, sans selectionner, commun a la creation groupe et tag. */
  function clearAndFocusName(form) {
    var input = qs("input[name='name']", form);
    if (input) {
      input.value = "";
    }
    focusName(form);
  }

  function openGroupCreate(form) {
    if (!form) {
      return;
    }
    closeGroupCreate(form);
    form.hidden = false;
    clearAndFocusName(form);
  }

  /* Fermeture groupée de tous les éditeurs, `except` reste visible pour les bascules. */
  function closeAllEditors(except) {
    closeMenus(null);
    inlineClosers.closeViewRename(null);
    inlineClosers.closeTagEditors(null);
    inlineClosers.closeGroupRename(null);
    inlineClosers.closeGroupColor(null);
    inlineClosers.closeViewGroupRename(null);
    inlineClosers.closeViewGroupColor(null);
    inlineClosers.closeViewColor(null);
    closeGroupTagForms(null);
    closeGroupCreate(except);
    closeViewGroupCreate(except);
    closeViewEditors(except);
  }

  function focusGroupRename() {
    focusName(
      qs(
        ".sidebar__group.is-renaming [data-group-rename]:not([hidden]), .sidebar__group.is-view-group-renaming [data-view-group-rename]:not([hidden])"
      ),
      true
    );
  }

  /* État des dossiers de tags gardé par navigateur, prioritaire sur le repli serveur. */
  var groupStateKey = "fs-ez.sidebar.groups";

  function readGroupState() {
    var state = readJson(groupStateKey, null);
    return state && !Array.isArray(state) ? state : {};
  }

  function writeGroupState(state) {
    writeJson(groupStateKey, state);
  }

  function groupDetailsKey(details) {
    var row = details.closest("[data-group-row], [data-view-group-row]");
    if (!row) {
      return null;
    }
    return (
      row.getAttribute("data-group-drag") ||
      row.getAttribute("data-view-group-drag")
    );
  }

  function rememberGroupState(details) {
    if (!details || !details.classList.contains("sidebar__group")) {
      return;
    }
    var key = groupDetailsKey(details);
    if (!key) {
      return;
    }
    var state = readGroupState();
    state[key] = details.open;
    writeGroupState(state);
  }

  function restoreGroupState(root) {
    var scope = root instanceof Element ? root : document;
    var state = readGroupState();
    each(qsa(".sidebar__group", scope), function (group) {
      var key = groupDetailsKey(group);
      if (key && Object.prototype.hasOwnProperty.call(state, key)) {
        group.open = Boolean(state[key]);
      }
    });
  }

  function closeGroupTagForms(except) {
    hideForms("[data-group-tag-form]", except);
  }

  function openGroupTagForm(details) {
    if (!details) {
      return;
    }
    closeGroupTagForms(null);
    details.open = true;
    var form = qs("[data-group-tag-form]", details);
    if (!form) {
      return;
    }
    form.hidden = false;
    clearAndFocusName(form);
  }

  function focusCreationError() {
    focusName(
      qs(
        "[data-group-tag-form]:not([hidden]), [data-group-create-form]:not([hidden]), [data-view-create-form]:not([hidden])"
      ),
      true
    );
  }

  function showToast(message, isError) {
    var toast = document.createElement("div");
    toast.className = isError ? "toast toast--error" : "toast";
    toast.setAttribute("role", "status");
    toast.textContent = message;
    document.body.appendChild(toast);
    window.setTimeout(function () {
      toast.remove();
    }, 5000);
  }

  function isTyping(target) {
    if (!target || !(target instanceof Element)) {
      return false;
    }
    if (target.isContentEditable) {
      return true;
    }
    return Boolean(target.closest("input, textarea, select, [contenteditable='true']"));
  }

  document.addEventListener(
    "toggle",
    function (event) {
      rememberGroupState(event.target);
    },
    true
  );

  app.closeMenus = closeMenus;
  app.closeOpenDropdown = closeOpenDropdown;
  app.initDropdowns = initDropdowns;
  app.openTagEditor = openTagEditor;
  app.openGroupRename = openGroupRename;
  app.openGroupColor = openGroupColor;
  app.closeGroupCreate = closeGroupCreate;
  app.openViewGroupRename = openViewGroupRename;
  app.openViewGroupColor = openViewGroupColor;
  app.closeViewGroupCreate = closeViewGroupCreate;
  app.openViewGroupCreate = openViewGroupCreate;
  app.closeViewEditors = closeViewEditors;
  app.closeAllEditors = closeAllEditors;
  app.openViewRename = openViewRename;
  app.openViewColor = openViewColor;
  app.openViewCreate = openViewCreate;
  app.placeViewCreatePanel = placeViewCreatePanel;
  app.openGroupCreate = openGroupCreate;
  app.closeGroupTagForms = closeGroupTagForms;
  app.openGroupTagForm = openGroupTagForm;
  app.focusGroupRename = focusGroupRename;
  app.focusCreationError = focusCreationError;
  app.restoreGroupState = restoreGroupState;
  app.showToast = showToast;
  app.isTyping = isTyping;
  window.fsEz.app = app;
})();
