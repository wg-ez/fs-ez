/* Dossiers virtuels : renommage en ligne et déplacement par glisser-déposer. */
(function () {
  "use strict";

  var app = window.fsEz.app || {};
  var qsa = window.fsEz.qsa;
  var each = window.fsEz.each;
  var drag = null;

  function folderRow(target) {
    return target instanceof Element ? target.closest("[data-folder-row]") : null;
  }

  function dropTarget(target) {
    return target instanceof Element ? target.closest("[data-folder-drop]") : null;
  }

  function closeRename(except) {
    each(qsa("[data-folder-rename]"), function (form) {
      if (form !== except) {
        form.hidden = true;
      }
    });
    each(qsa("[data-folder-row].is-renaming"), function (row) {
      if (!except || row !== except.closest("[data-folder-row]")) {
        row.classList.remove("is-renaming");
      }
    });
  }

  function folderRowById(id) {
    if (!id) {
      return null;
    }
    if (window.CSS && typeof window.CSS.escape === "function") {
      return document.querySelector("[data-folder-row='" + window.CSS.escape(id) + "']");
    }
    var rows = qsa("[data-folder-row]");
    for (var index = 0; index < rows.length; index += 1) {
      if (rows[index].getAttribute("data-folder-row") === id) {
        return rows[index];
      }
    }
    return null;
  }

  function openRename(id) {
    var row = folderRowById(id);
    if (!row) {
      return;
    }
    var form = row.querySelector("[data-folder-rename]");
    if (!form) {
      return;
    }
    closeRename(form);
    row.classList.add("is-renaming");
    form.hidden = false;
    var input = window.fsEz.focusName(form, true);
    if (input) {
      input.select();
    }
  }

  function showError(message) {
    app.showToast(message || "Error", true);
  }

  function createForm() {
    return document.querySelector("[data-folder-create]");
  }

  function createRow(form) {
    return form ? form.closest("[data-folder-create-row]") : null;
  }

  function closeCreate() {
    var row = createRow(createForm());
    if (row) {
      row.hidden = true;
    }
  }

  /* Zone statique hors scroll virtuel : `..` puis le placeholder de création. */
  function staticRowsBody() {
    return document.querySelector("[data-folder-static]");
  }

  /* Placeholder inséré après la ligne `..`, sinon en tête de la zone statique. */
  function placeCreateRow(row, anchorId) {
    var body = staticRowsBody();
    if (!body) {
      return;
    }
    var anchor = anchorId ? folderRowById(anchorId) : null;
    var up = body.querySelector("[data-folder-up]");
    if (!anchor || !body.contains(anchor)) {
      anchor = up;
    }
    if (anchor && anchor.parentNode === body) {
      body.insertBefore(row, anchor.nextSibling);
      return;
    }
    body.insertBefore(row, body.firstChild);
  }

  function openCreate(parent, anchorId) {
    var form = createForm();
    var row = createRow(form);
    if (!form || !row) {
      return;
    }
    closeRename(null);
    placeCreateRow(row, anchorId);
    var field = form.querySelector("[data-folder-create-parent]");
    if (field) {
      field.value = parent || "";
    }
    row.hidden = false;
    var input = form.querySelector("[data-folder-create-name]");
    if (input) {
      input.value = "";
      input.focus();
    }
  }

  /* htmx 2 convertit les tableaux en clés répétées, le serveur émet `data-changed` après succès. */
  function postValues(url, values) {
    if (!window.htmx) {
      return;
    }
    window.htmx
      .ajax("POST", url, {
        target: "#toast",
        swap: "innerHTML",
        values: values
      })
      .catch(function () {
        showError();
      });
  }

  document.addEventListener("dragstart", function (event) {
    if (!(event.target instanceof Element) || !event.dataTransfer) {
      return;
    }
    var row = folderRow(event.target);
    if (row) {
      var id = row.getAttribute("data-folder-row") || "";
      var selected = app.selectedFolderIds ? app.selectedFolderIds() : [];
      var whole = selected.indexOf(id) !== -1 && selected.length > 1;
      var ids = whole ? selected : [id];
      drag = { kind: whole ? "folders" : "folder", id: id, ids: ids };
      event.dataTransfer.setData("application/x-fs-ez-folder", ids.join(","));
      event.dataTransfer.setData("text/plain", "");
      event.dataTransfer.effectAllowed = "move";
      row.classList.add("is-dragging");
      return;
    }
    var file = event.target.closest("[data-file-drop][data-file-id]");
    if (!file) {
      return;
    }
    var id = file.getAttribute("data-file-id");
    var selected = app.selectedFileIds ? app.selectedFileIds() : [];
    var whole = selected.indexOf(id) !== -1 && selected.length > 1;
    var ids = whole ? selected : [id];
    drag = { kind: "files", ids: ids, whole: whole };
    event.dataTransfer.setData("application/x-fs-ez-files", ids.join(","));
    event.dataTransfer.setData("text/plain", "");
    event.dataTransfer.effectAllowed = "move";
  });

  document.addEventListener("dragover", function (event) {
    var target = dropTarget(event.target);
    if (!target) {
      return;
    }
    app.allowDrop(event, "move");
    app.markDropTarget(target, "[data-folder-drop]");
  });

  document.addEventListener("dragleave", function (event) {
    var target = dropTarget(event.target);
    if (target && !target.contains(event.relatedTarget)) {
      target.classList.remove("is-drop");
    }
  });

  /* Drop unique : déplacement fichiers/dossiers d'abord, tag `text/plain` ensuite. */
  document.addEventListener("drop", function (event) {
    var target = dropTarget(event.target);
    if (!target) {
      return;
    }
    var folder = target.getAttribute("data-folder-drop") || "";
    if (!drag) {
      var tag = event.dataTransfer ? event.dataTransfer.getData("text/plain") : "";
      /* `/virtual-folders/{id}/tags` exige un id : `..` à la racine ne peut pas recevoir de tag. */
      if (!tag || !folder || !window.htmx) {
        return;
      }
      event.preventDefault();
      target.classList.remove("is-drop");
      var ready = app.folderSelectionReady
        ? app.folderSelectionReady()
        : window.Promise.resolve();
      ready.then(function () {
        var files = app.selectedFileIds ? app.selectedFileIds() : [];
        var folders = app.selectedFolderIds ? app.selectedFolderIds() : [];
        var mixed = folders.indexOf(folder) !== -1 && (files.length > 0 || folders.length > 1);
        if (mixed) {
          var values = {
            ids: files,
            tags: tag,
            action: "add"
          };
          if (files.length > 0 && app.outOfBandIds) {
            values.oob_ids = app.outOfBandIds(files);
          }
          if (folders.length > 0) {
            values.folder_ids = folders;
          }
          window.htmx
            .ajax("POST", "/partials/files/tags", {
              target: "#toast",
              swap: "innerHTML",
              values: values
            })
            .catch(function () {
              showError();
            });
          return;
        }
        window.htmx
          .ajax("POST", "/partials/virtual-folders/" + folder + "/tags", {
            target: "#toast",
            swap: "innerHTML",
            values: { tags: tag }
          })
          .catch(function () {
            showError();
          });
      });
      return;
    }
    event.preventDefault();
    target.classList.remove("is-drop");
    if (drag.kind === "folder") {
      if (drag.id !== folder) {
        postValues("/partials/virtual-folders/" + drag.id + "/move", {
          parent: folder
        });
      }
    } else if (drag.kind === "folders") {
      var folderIds = drag.ids.filter(function (id) {
        return id !== folder;
      });
      if (folderIds.length > 0) {
        postValues("/partials/virtual-folders/move-selection", {
          folder: folder,
          folder_ids: folderIds
        });
      }
    } else if (drag.kind === "files") {
      var values = { folder: folder, ids: drag.ids };
      if (drag.whole && app.addSelectionScope) {
        app.addSelectionScope(values);
      }
      postValues("/partials/files/move", values);
    }
    drag = null;
  });

  document.addEventListener("dragend", function () {
    drag = null;
    each(qsa("[data-folder-drop].is-drop"), function (target) {
      target.classList.remove("is-drop");
    });
    each(qsa("[data-folder-row].is-dragging"), function (row) {
      row.classList.remove("is-dragging");
    });
  });

  document.addEventListener("click", function (event) {
    if (!(event.target instanceof Element)) {
      return;
    }
    if (event.target.closest("#file-context-menu")) {
      return;
    }
    if (!event.target.closest("[data-folder-row]")) {
      closeRename(null);
    }
    if (!event.target.closest("[data-folder-create-row]")) {
      closeCreate();
    }
  });

  document.addEventListener("keydown", function (event) {
    if (event.key !== "Escape") {
      return;
    }
    closeCreate();
    closeRename(null);
  });

  /* htmx photographie la page avant le swap : fermer le placeholder avant évite de le restaurer au retour. */
  document.addEventListener("htmx:beforeHistorySave", closeCreate);
  document.addEventListener("htmx:historyRestore", closeCreate);

  document.addEventListener("htmx:beforeSwap", function (event) {
    var target = event.target;
    if (target instanceof Element && (target.id === "files-page" || target.id === "work")) {
      closeCreate();
    }
  });

  app.folderCreate = openCreate;
  app.folderOpenRename = openRename;
  window.fsEz.app = app;
})();
