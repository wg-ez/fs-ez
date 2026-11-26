/* Écouteurs globaux restants : sélection de lignes, clics de coquille, raccourcis et htmx. */
(function () {
  "use strict";

  var app = window.fsEz.app || {};
  var qs = window.fsEz.qs;
  var qsa = window.fsEz.qsa;
  var each = window.fsEz.each;
  var body = document.body;

  function eventElement(event) {
    return event.target instanceof Element ? event.target : null;
  }

  /* Details du groupe porte par la ligne du declencheur, commun aux ouvertures view/groupe. */
  function editorRowDetails(target, rowSelector) {
    var row = target.closest(rowSelector);
    return row ? row.querySelector("details.sidebar__group") : null;
  }

  function openRowEditor(target, rowSelector, mode, openColor, openRename) {
    var details = editorRowDetails(target, rowSelector);
    if (mode === "color") {
      openColor(details);
    } else {
      openRename(details);
    }
  }

  /* Bascule d'un formulaire de creation : ferme si visible, sinon ouvre avec le groupe du declencheur. */
  function toggleCreateForm(event, options) {
    var trigger = event.target.closest(options.trigger);
    if (!trigger) {
      return false;
    }
    event.preventDefault();
    var form = qs(options.form);
    app.closeAllEditors(form);
    if (form && !form.hidden) {
      options.close(null);
    } else {
      options.open(form, trigger.getAttribute("data-view-group"));
    }
    return true;
  }

  /* Fermetures au clic hors formulaire, ordre d'origine conserve. */
  var outsideClosers = [
    ["[data-group-create-form]", app.closeGroupCreate],
    ["[data-view-group-create-form]", app.closeViewGroupCreate],
    ["[data-view-rename]", app.closeViewRename],
    ["[data-view-group-rename]", app.closeViewGroupRename],
    ["[data-view-group-edit='color']", app.closeViewGroupColor],
    ["[data-view-edit='color']", app.closeViewColor],
    ["[data-view-editor]", app.closeViewEditors],
    ["[data-group-rename]", app.closeGroupRename],
    ["[data-group-edit='color']", app.closeGroupColor],
    ["[data-group-tag-form], [data-group-tag-open]", app.closeGroupTagForms],
    [".sidebar__row", app.closeTagEditors]
  ];

  /* Clic simple = selection exclusive, ctrl/meta ajoute ou retire, shift etend la plage depuis l'ancre. */
  function rowSelectTarget(target) {
    if (!(target instanceof Element)) {
      return null;
    }
    var row = target.closest(".table__row[data-file-id]");
    if (!row || row.classList.contains("is-renaming")) {
      return null;
    }
    if (
      target.closest(
        "a, button, input, textarea, select, label, [data-remove-tag], [data-file-action]"
      )
    ) {
      return null;
    }
    return row;
  }

  function selectableRowsIn(row) {
    var scope = row.closest("#files-page") || document;
    return Array.prototype.slice.call(
      scope.querySelectorAll(".table__row[data-file-id]")
    );
  }

  /* Ctrl/clic cree une selection de texte sous Firefox, shift en cree une aussi : on bloque le defaut. */
  document.addEventListener("mousedown", function (event) {
    var target = eventElement(event);
    if (
      (event.shiftKey || event.ctrlKey || event.metaKey) &&
      rowSelectTarget(target)
    ) {
      event.preventDefault();
    }
  });

  document.addEventListener("click", function (event) {
    var target = eventElement(event);
    if (!target) {
      return;
    }
    var row = rowSelectTarget(target);
    if (!row) {
      if (
        !target.closest(".table__row[data-file-id]") &&
        !target.closest("[data-folder-row], [data-folder-up]") &&
        !target.closest("#file-context-menu")
      ) {
        app.clearFileSelection();
      }
      return;
    }
    var rows = selectableRowsIn(row);
    if (event.shiftKey && app.selectRange && app.selectRange(row)) {
      /* Plage étendue hors DOM par `selectRange`, ancre déjà mise à jour. */
    } else if (event.ctrlKey || event.metaKey) {
      /* Ctrl+A reste global : le ctrl/clic n'ajoute qu'une exception, sans vider la sélection. */
      app.setRowSelected(row, !app.rowSelected(row));
      if (app.setSelectionAnchor) {
        app.setSelectionAnchor(row);
      }
    } else if (app.selectOnlyFileRow) {
      app.selectOnlyFileRow(row);
    } else {
      each(rows, function (candidate) {
        app.setRowSelected(candidate, candidate === row);
      });
    }
    each(qsa(".table__row.is-active"), function (candidate) {
      candidate.classList.remove("is-active");
    });
    app.lastSelectRow = row;
  });

  document.addEventListener("click", function (event) {
    var target = eventElement(event);
    if (!target) {
      return;
    }

    var railToggle = target.closest(".js-rail-toggle");
    if (railToggle) {
      if (app.desktopViewport()) {
        app.setSidebarHidden(!app.sidebarHidden());
        app.persistSidebarToggle();
      } else if (app.railIsOpen()) {
        app.closeRail();
      } else {
        app.openRail();
      }
      return;
    }

    if (
      toggleCreateForm(event, {
        trigger: "[data-view-create-open]",
        form: "[data-view-create-form]",
        close: app.closeViewEditors,
        open: app.openViewCreate
      })
    ) {
      return;
    }

    if (
      toggleCreateForm(event, {
        trigger: "[data-view-group-create-open]",
        form: "[data-view-group-create-form]",
        close: app.closeViewGroupCreate,
        open: app.openViewGroupCreate
      })
    ) {
      return;
    }

    var viewGroupEdit = target.closest("[data-view-group-edit-open]");
    if (viewGroupEdit) {
      event.preventDefault();
      event.stopPropagation();
      app.closeAllEditors(null);
      openRowEditor(
        viewGroupEdit,
        "[data-view-group-row]",
        viewGroupEdit.getAttribute("data-view-group-edit-open"),
        app.openViewGroupColor,
        app.openViewGroupRename
      );
      return;
    }

    var viewClose = target.closest("[data-view-editor-close]");
    if (viewClose) {
      event.preventDefault();
      app.closeViewEditors(null);
      return;
    }

    var viewRename = target.closest("[data-view-rename-open]");
    if (viewRename) {
      event.preventDefault();
      event.stopPropagation();
      app.closeAllEditors(null);
      app.openViewRename(viewRename.closest("[data-view-row]"));
      return;
    }

    var viewColor = target.closest("[data-view-edit-open='color']");
    if (viewColor) {
      event.preventDefault();
      event.stopPropagation();
      app.closeAllEditors(null);
      app.openViewColor(viewColor.closest("[data-view-row]"));
      return;
    }

    if (
      toggleCreateForm(event, {
        trigger: "[data-group-create-open]",
        form: "[data-group-create-form]",
        close: app.closeGroupCreate,
        open: app.openGroupCreate
      })
    ) {
      return;
    }

    var groupAdd = target.closest("[data-group-tag-open]");
    if (groupAdd) {
      event.preventDefault();
      event.stopPropagation();
      app.closeAllEditors(null);
      app.openGroupTagForm(editorRowDetails(groupAdd, "[data-group-row]"));
      return;
    }

    var groupEdit = target.closest("[data-group-edit-open]");
    if (groupEdit) {
      event.preventDefault();
      event.stopPropagation();
      app.closeAllEditors(null);
      openRowEditor(
        groupEdit,
        "[data-group-row]",
        groupEdit.getAttribute("data-group-edit-open"),
        app.openGroupColor,
        app.openGroupRename
      );
      return;
    }

    var editAction = target.closest("[data-tag-edit-open]");
    if (editAction) {
      event.preventDefault();
      event.stopPropagation();
      app.closeAllEditors(null);
      app.openTagEditor(
        editAction.closest(".sidebar__row"),
        editAction.getAttribute("data-tag-edit-open")
      );
      return;
    }

    var menu = target.closest("details.dropdown");
    app.closeMenus(menu);

    each(outsideClosers, function (entry) {
      if (!target.closest(entry[0])) {
        entry[1](null);
      }
    });

    var sidebar = qs(".sidebar");
    if (
      body.classList.contains("is-rail-open") &&
      sidebar &&
      !sidebar.contains(target)
    ) {
      app.closeRail();
    }
  });

  document.addEventListener("dblclick", function (event) {
    var target = eventElement(event);
    if (!target) {
      return;
    }
    var resizer = target.closest("[data-col-resizer]");
    if (resizer) {
      var resizerTable = resizer.closest("table[data-col-resizable]");
      if (resizerTable) {
        event.preventDefault();
        app.resetColumnWidths(resizerTable);
        return;
      }
    }
    var pill = target.closest("[data-remove-tag]");
    if (!pill) {
      return;
    }
    event.preventDefault();
    app.removeFileTag(pill);
  });

  document.addEventListener("htmx:afterSwap", function (event) {
    var target = eventElement(event);
    if (target) {
      app.restoreGroupState(target);
      app.initColumnTables(document);
      app.initDropdowns(target);
      var createForm = qs("[data-view-create-form]");
      if (createForm) {
        app.placeViewCreatePanel(createForm);
      }
      if (target.id === "files-page" || target.id === "work") {
        app.restoreSelection(true);
      }
      app.focusGroupRename();
      app.focusCreationError();
    }
    dismissToasts(document);
  });

  document.addEventListener("keydown", function (event) {
    if (event.key === "Escape") {
      /* Un menu ouvert capte l'Escape, l'editeur qui le contient reste affiche. */
      if (app.closeOpenDropdown()) {
        event.preventDefault();
        return;
      }
      app.closeAllEditors(null);
      app.closeRail();
      return;
    }

    if (event.defaultPrevented) {
      return;
    }
    if (app.isTyping(event.target)) {
      return;
    }

    if (
      (event.ctrlKey || event.metaKey) &&
      !event.altKey &&
      (event.key === "a" || event.key === "A")
    ) {
      if (qs("#files-page")) {
        event.preventDefault();
        app.selectAllFiles();
      }
      return;
    }

    if (event.altKey || event.ctrlKey || event.metaKey) {
      return;
    }

    if (event.key === "Delete" && qs("#files-page")) {
      event.preventDefault();
      app.deleteSelection();
      return;
    }
    if (event.key === "Enter" && qs("#files-page") && app.openActiveRow()) {
      event.preventDefault();
      return;
    }
    if (event.key === "F2" && qs("#files-page") && app.renameActiveRow()) {
      event.preventDefault();
      return;
    }
    if (event.key === "/") {
      var search = qs("#global-search");
      if (search) {
        event.preventDefault();
        search.focus();
        search.select();
      }
      return;
    }
    if (event.key === "j") {
      event.preventDefault();
      app.moveRow(1);
      return;
    }
    if (event.key === "k") {
      event.preventDefault();
      app.moveRow(-1);
      return;
    }
    if (event.key === " ") {
      event.preventDefault();
      app.toggleRowSelection();
    }
  });

  document.body.addEventListener("htmx:confirm", function (event) {
    var trigger = eventElement(event);
    if (!trigger) {
      return;
    }
    var element = trigger.closest("[data-confirm]");
    if (!element) {
      return;
    }
    event.preventDefault();
    if (window.confirm(element.getAttribute("data-confirm") || "")) {
      event.detail.issueRequest(true);
    }
  });

  /* Toute pastille reste 5 s, y compris celles rendues par htmx ou ajoutees par `showToast`. */
  var TOAST_MS = 5000;

  function dismissToasts(root) {
    each((root || document).querySelectorAll(".toast"), function (toast) {
      if (toast.getAttribute("data-toast-timer") === "1") {
        return;
      }
      toast.setAttribute("data-toast-timer", "1");
      window.setTimeout(function () {
        toast.remove();
      }, TOAST_MS);
    });
  }

  function fallbackCopy(value) {
    var area = document.createElement("textarea");
    area.value = value;
    area.setAttribute("readonly", "");
    area.className = "sr-only";
    document.body.appendChild(area);
    area.select();
    var copied = false;
    try {
      copied = document.execCommand("copy");
    } catch (error) {}
    document.body.removeChild(area);
    return copied;
  }

  function copyText(value, done) {
    if (window.navigator.clipboard && window.isSecureContext) {
      window.navigator.clipboard.writeText(value).then(
        function () {
          done(true);
        },
        function () {
          done(fallbackCopy(value));
        }
      );
      return;
    }
    done(fallbackCopy(value));
  }

  document.addEventListener("click", function (event) {
    var target = eventElement(event);
    if (!target) {
      return;
    }
    var button = target.closest("[data-clipboard-target]");
    if (!button || button.getAttribute("data-clipboard-busy") === "1") {
      return;
    }
    var source = qs(
      button.getAttribute("data-clipboard-target"),
      button.closest(".settings__secret") || document
    );
    if (!source) {
      return;
    }
    copyText(source.textContent, function (copied) {
      if (!copied) {
        return;
      }
      var original = button.textContent;
      button.setAttribute("data-clipboard-busy", "1");
      button.textContent = button.getAttribute("data-clipboard-copied") || original;
      window.setTimeout(function () {
        button.textContent = original;
        button.removeAttribute("data-clipboard-busy");
      }, TOAST_MS);
    });
  });

  dismissToasts(document);
  window.fsEz.app = app;
})();
