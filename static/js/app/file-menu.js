/* Sélection de lignes et menu contextuel fichiers, capture et restauration après swap. */
(function () {
  "use strict";

  var app = window.fsEz.app || {};
  var qs = window.fsEz.qs;
  var qsa = window.fsEz.qsa;
  var each = window.fsEz.each;
  var focusName = window.fsEz.focusName;

  /* Ctrl+A couvre toute la requête, pas seulement les lignes rendues par la page courante. */
  var selectAll = false;
  /* Sélection et ancre survivent aux fenêtres du scroll virtuel : ensemble d'ids hors DOM. */
  var selectedFiles = newSelection();
  /* Sélection de dossiers séparée : tags et suppression mixtes, Ctrl+A couvre tout le dossier. */
  var selectedFolders = newSelection();
  /* Ctrl+A dossier : la portée dossier n'existe pas côté serveur, les ids hors DOM sont chargés. */
  var selectAllFolders = false;
  var folderSelectAllController = null;
  var folderSelectAllPromise = null;
  /* Désélection ctrl sous Ctrl+A dossier, jamais réajoutée par le chargement complet. */
  var folderExcludedMap = Object.create(null);
  /* Mode déplacement : le prochain clic droit choisit la destination. */
  var moveMode = false;
  /* Sous Ctrl+A, un clic ctrl ou espace retire une exception sans matérialiser 10 000 ids. */
  var excludedIds = [];
  var excludedMap = Object.create(null);
  var anchorId = null;
  var anchorIndex = null;
  var anchorFolderId = null;
  var anchorNav = null;
  var rangeController = null;
  var rangeGeneration = 0;
  /* Plage dossier séparée : une sélection de dossiers et de fichiers peut coexister. */
  var folderRangeController = null;
  var folderRangeGeneration = 0;
  var ROWS_CHUNK = 500;

  function newSelection() {
    return { ids: [], map: Object.create(null), kinds: Object.create(null) };
  }

  function selectionRemember(selection, id, kind) {
    if (!id) {
      return;
    }
    if (!selection.map[id]) {
      selection.map[id] = true;
      selection.ids.push(id);
    }
    if (kind && !selection.kinds[id]) {
      selection.kinds[id] = kind;
    }
  }

  function selectionForget(selection, id) {
    if (!id) {
      return;
    }
    delete selection.kinds[id];
    if (!selection.map[id]) {
      return;
    }
    delete selection.map[id];
    var index = selection.ids.indexOf(id);
    if (index !== -1) {
      selection.ids.splice(index, 1);
    }
  }

  function selectionClear(selection) {
    selection.ids.length = 0;
    selection.map = Object.create(null);
    selection.kinds = Object.create(null);
  }

  function rows() {
    return qsa(".table__row[data-file-id]");
  }

  /* Ordre visuel du DOM : `..`, dossiers puis fichiers. */
  function navRows() {
    return qsa(
      ".table__row[data-folder-up], .table__row[data-folder-row], .table__row[data-file-id]"
    );
  }

  function upRowCount() {
    return qs(".table__row[data-folder-up]") ? 1 : 0;
  }

  function folderRowsInfo() {
    return app.folderRowsInfo ? app.folderRowsInfo() : null;
  }

  /* Total de dossiers, fenêtre virtuelle comprise, pas seulement les lignes du DOM. */
  function folderCount() {
    var info = folderRowsInfo();
    return info ? info.total : qsa(".table__row[data-folder-row]").length;
  }

  /* Index absolu du dossier : repli DOM quand le scroll virtuel n'expose pas l'index. */
  function folderIndex(row) {
    if (app.folderRowIndex) {
      var index = app.folderRowIndex(row);
      if (index !== null && index !== undefined) {
        return index;
      }
    }
    var fallback = qsa(".table__row[data-folder-row]").indexOf(row);
    return fallback === -1 ? null : fallback;
  }

  /* Les dossiers précèdent toujours les fichiers : l'index visuel reste stable sous scroll virtuel. */
  function navOffset() {
    return upRowCount() + folderCount();
  }

  /* Index visuel unifié : lignes dossier absolues puis index absolu du fichier. */
  function navIndex(row) {
    if (!row) {
      return null;
    }
    if (isUpRow(row)) {
      return 0;
    }
    if (isFolderNavRow(row)) {
      var index = folderIndex(row);
      if (index === null) {
        var visible = navRows().indexOf(row);
        return visible === -1 ? null : visible;
      }
      return upRowCount() + index;
    }
    var fileIndex = absoluteIndex(row);
    return fileIndex === null ? null : navOffset() + fileIndex;
  }

  /* Repli `CSS.escape` absent : parcours de la liste, les ids peuvent contenir des guillemets. */
  function rowById(selector, attribute, id) {
    if (!id) {
      return null;
    }
    if (window.CSS && typeof window.CSS.escape === "function") {
      var found = qs(selector + "[" + attribute + "='" + window.CSS.escape(id) + "']");
      if (found) {
        return found;
      }
    }
    var list = qsa(selector);
    for (var index = 0; index < list.length; index += 1) {
      if (list[index].getAttribute(attribute) === id) {
        return list[index];
      }
    }
    return null;
  }

  function folderNavRowById(id) {
    return rowById("[data-folder-row]", "data-folder-row", id);
  }

  function activeNavRow() {
    return qs(".table__row.is-active");
  }

  function isFolderNavRow(row) {
    return Boolean(
      row && (row.hasAttribute("data-folder-row") || row.hasAttribute("data-folder-up"))
    );
  }

  function isUpRow(row) {
    return Boolean(row && row.hasAttribute("data-folder-up"));
  }

  function moveRow(step) {
    var list = navRows();
    if (list.length === 0) {
      return;
    }
    var current = activeNavRow() || app.lastSelectRow;
    var index = current ? list.indexOf(current) : -1;
    index += step;
    if (index < 0) {
      index = 0;
    }
    if (index >= list.length) {
      index = list.length - 1;
    }
    each(list, function (row, index_) {
      row.classList.toggle("is-active", index_ === index);
    });
    app.lastSelectRow = list[index];
    list[index].scrollIntoView({ block: "nearest" });
  }

  function rowId(row) {
    return row ? row.getAttribute("data-file-id") : null;
  }

  function rowKind(row) {
    return row ? row.getAttribute("data-file-kind") : null;
  }

  function clearRangeRequest() {
    rangeGeneration += 1;
    if (rangeController) {
      rangeController.abort();
      rangeController = null;
    }
    folderRangeGeneration += 1;
    if (folderRangeController) {
      folderRangeController.abort();
      folderRangeController = null;
    }
  }

  function selectedFolderIds() {
    return selectedFolders.ids.slice();
  }

  function clearExcludedFiles() {
    excludedIds = [];
    excludedMap = Object.create(null);
  }

  function excludeFile(id) {
    if (!id || excludedMap[id]) {
      return;
    }
    excludedMap[id] = true;
    excludedIds.push(id);
    selectionForget(selectedFiles, id);
  }

  function includeFile(id, kind) {
    if (!id) {
      return;
    }
    if (excludedMap[id]) {
      delete excludedMap[id];
      var index = excludedIds.indexOf(id);
      if (index !== -1) {
        excludedIds.splice(index, 1);
      }
    }
    selectionRemember(selectedFiles, id, kind);
  }

  function clearSelectedFiles() {
    selectionClear(selectedFiles);
    selectAll = false;
    clearExcludedFiles();
    each(rows(), function (row) {
      row.classList.remove("is-selected");
    });
  }

  function clearSelectedFolders() {
    selectAllFolders = false;
    cancelFolderSelectAll();
    folderExcludedMap = Object.create(null);
    selectionClear(selectedFolders);
    each(qsa(".table__row[data-folder-row].is-selected"), function (row) {
      row.classList.remove("is-selected");
    });
  }

  function cancelFolderSelectAll() {
    folderSelectAllPromise = null;
    if (folderSelectAllController) {
      folderSelectAllController.abort();
      folderSelectAllController = null;
    }
  }

  /* Les actions attendent les ids dossier hors DOM chargés par Ctrl+A. */
  function folderSelectionReady() {
    return folderSelectAllPromise || window.Promise.resolve();
  }

  function folderRowId(row) {
    return row ? row.getAttribute("data-folder-row") : null;
  }

  function folderRowSelected(row) {
    var id = folderRowId(row);
    if (!id) {
      return false;
    }
    if (selectAllFolders) {
      return !folderExcludedMap[id];
    }
    return Boolean(selectedFolders.map[id] || row.classList.contains("is-selected"));
  }

  function setFolderRowSelected(row, selected) {
    var id = folderRowId(row);
    if (!id) {
      return;
    }
    if (selectAllFolders) {
      if (selected) {
        delete folderExcludedMap[id];
        selectionRemember(selectedFolders, id);
        row.classList.add("is-selected");
        return;
      }
      folderExcludedMap[id] = true;
      selectionForget(selectedFolders, id);
      row.classList.remove("is-selected");
      return;
    }
    if (selected) {
      selectionRemember(selectedFolders, id);
      row.classList.add("is-selected");
      return;
    }
    selectionForget(selectedFolders, id);
    row.classList.remove("is-selected");
  }

  function selectOnlyFolderRow(row) {
    clearRangeRequest();
    clearSelectedFiles();
    clearSelectedFolders();
    setFolderRowSelected(row, true);
    setAnchor(row);
    app.lastSelectRow = row;
  }

  function hasSelection() {
    return selectAll || selectedFiles.ids.length > 0 || selectedFolders.ids.length > 0;
  }

  function setAnchor(row) {
    if (isFolderNavRow(row)) {
      if (isUpRow(row)) {
        return;
      }
      anchorFolderId = folderRowId(row);
      anchorId = null;
      anchorIndex = null;
      anchorNav = navIndex(row);
      app.lastSelectRow = row;
      return;
    }
    var id = rowId(row);
    if (!id) {
      return;
    }
    anchorFolderId = null;
    anchorId = id;
    anchorIndex = absoluteIndex(row);
    anchorNav = navIndex(row);
    app.lastSelectRow = row;
  }

  function clearAnchor() {
    anchorId = null;
    anchorIndex = null;
    anchorFolderId = null;
    anchorNav = null;
    app.lastSelectRow = null;
  }

  function syncAnchorRow() {
    if (anchorFolderId) {
      var folderRow = folderNavRowById(anchorFolderId);
      if (folderRow) {
        app.lastSelectRow = folderRow;
        anchorNav = navIndex(folderRow);
      } else {
        app.lastSelectRow = null;
      }
      return;
    }
    if (!anchorId) {
      return;
    }
    var row = fileRowById(anchorId);
    if (row) {
      app.lastSelectRow = row;
      anchorIndex = absoluteIndex(row);
      anchorNav = navIndex(row);
      return;
    }
    app.lastSelectRow = null;
  }

  /* Index absolu dans la requête : le scroll virtuel ajoute son offset à l'index DOM. */
  function absoluteIndex(row) {
    if (app.fileRowIndex) {
      var index = app.fileRowIndex(row);
      if (index !== null && index !== undefined) {
        return index;
      }
    }
    var fallback = rows().indexOf(row);
    return fallback === -1 ? null : fallback;
  }

  function rowsInfo() {
    return app.filesRowsInfo ? app.filesRowsInfo() : null;
  }

  function dropSelectAll() {
    selectAll = false;
    clearExcludedFiles();
  }

  /* Scope serveur de la sélection globale : la première page décrit la requête sans curseur. */
  function selectionScope() {
    if (!selectAll) {
      return null;
    }
    var page = qs("#files-page");
    var url = page ? page.getAttribute("data-scope-url") : "";
    if (!url) {
      return null;
    }
    return { scope: "all", scope_url: url, exclude_ids: excludedIds.slice() };
  }

  /* Recopie le scope sur un corps de formulaire, exceptions Ctrl+A comprises. */
  function addSelectionScope(values) {
    var scope = selectionScope();
    if (scope) {
      values.scope = scope.scope;
      values.scope_url = scope.scope_url;
      values.exclude_ids = scope.exclude_ids;
    }
    return values;
  }

  function rowSelected(row) {
    var id = rowId(row);
    if (selectAll) {
      return !(id && excludedMap[id]);
    }
    return row.classList.contains("is-selected") || Boolean(id && selectedFiles.map[id]);
  }

  function setRowSelected(row, selected) {
    var id = rowId(row);
    if (selectAll) {
      if (selected) {
        includeFile(id, rowKind(row));
      } else {
        excludeFile(id);
      }
      row.classList.toggle("is-selected", selected);
      return;
    }
    if (selected) {
      selectionRemember(selectedFiles, id, rowKind(row));
      row.classList.add("is-selected");
      return;
    }
    selectionForget(selectedFiles, id);
    row.classList.remove("is-selected");
  }

  function toggleRowSelection() {
    if (moveMode) {
      return;
    }
    var row = activeNavRow() || app.lastSelectRow;
    if (!row || isUpRow(row)) {
      return;
    }
    if (isFolderNavRow(row)) {
      setFolderRowSelected(row, !folderRowSelected(row));
      setAnchor(row);
      return;
    }
    setRowSelected(row, !rowSelected(row));
    setAnchor(row);
  }

  /* Ctrl+A : tous les fichiers de la requête et tous les dossiers du dossier courant. */
  function selectAllFiles() {
    if (moveMode) {
      return;
    }
    var list = rows();
    clearRangeRequest();
    clearSelectedFolders();
    clearExcludedFiles();
    selectAll = list.length > 0;
    each(list, function (row) {
      setRowSelected(row, true);
    });
    var folderInfo = folderRowsInfo();
    selectAllFolders = folderCount() > 0;
    each(qsa(".table__row[data-folder-row]"), function (row) {
      setFolderRowSelected(row, true);
    });
    if (selectAllFolders) {
      folderSelectAllPromise = fetchAllFolderIds(folderInfo);
    }
    if (list.length > 0 && (!app.lastSelectRow || list.indexOf(app.lastSelectRow) === -1)) {
      setAnchor(list[list.length - 1]);
    }
  }

  function selectedFileIds() {
    return selectedFiles.ids.slice();
  }

  /* Lignes rendues de la sélection : seules celles-ci peuvent recevoir un swap OOB. */
  function outOfBandIds(ids) {
    var wanted = Object.create(null);
    each(ids, function (id) {
      if (id) {
        wanted[id] = true;
      }
    });
    var visible = [];
    each(rows(), function (row) {
      var id = rowId(row);
      if (id && wanted[id]) {
        visible.push(id);
      }
    });
    /* Clé présente même vide : le serveur ne retombe pas sur toute la sélection. */
    return visible.length > 0 ? visible : "";
  }

  /* Valeur d'attribut par ligne de fragment, `attribute` accepte aussi un extracteur d'objet. */
  function parseFragmentRows(html, selector, attribute) {
    var doc = new window.DOMParser().parseFromString(
      "<table><tbody>" + html + "</tbody></table>",
      "text/html"
    );
    var body = doc.querySelector("tbody");
    if (!body) {
      return [];
    }
    var extract =
      typeof attribute === "function"
        ? attribute
        : function (row) {
            return row.getAttribute(attribute);
          };
    var values = [];
    each(qsa(selector, body), function (row) {
      var value = extract(row);
      if (value) {
        values.push(value);
      }
    });
    return values;
  }

  function rangeEntries(html) {
    return parseFragmentRows(html, ".table__row[data-file-id]", function (row) {
      return {
        id: row.getAttribute("data-file-id"),
        kind: row.getAttribute("data-file-kind")
      };
    });
  }

  /* Découpe une requête d'index en chunks de ROWS_CHUNK, séparateur déduit de l'URL. */
  function fetchChunked(url, from, to, signal, extract) {
    var requests = [];
    for (var offset = from; offset <= to; offset += ROWS_CHUNK) {
      var limit = Math.min(ROWS_CHUNK, to - offset + 1);
      var separator = url.indexOf("?") === -1 ? "?" : "&";
      requests.push(
        window.fsEz
          .fetchHtml(url + separator + "offset=" + offset + "&limit=" + limit, {
            signal: signal
          })
          .then(extract)
      );
    }
    return Promise.all(requests).then(function (chunks) {
      var entries = [];
      each(chunks, function (chunk) {
        entries = entries.concat(chunk);
      });
      return entries;
    });
  }

  function fetchRangeEntries(url, from, to, signal) {
    return fetchChunked(url, from, to, signal, rangeEntries);
  }

  function applyIndexRange(from, to, selected) {
    each(rows(), function (row) {
      var index = absoluteIndex(row);
      if (index !== null && index >= from && index <= to) {
        setRowSelected(row, selected);
      }
    });
  }

  /* Ids dossier d'un fragment de plage, hors DOM. */
  function folderRangeIds(html) {
    return parseFragmentRows(html, ".table__row[data-folder-row]", "data-folder-row");
  }

  function fetchFolderRangeIds(info, from, to, signal) {
    return fetchChunked(info.url, from, to, signal, folderRangeIds);
  }

  function refreshFolderSelectedClasses() {
    each(qsa(".table__row[data-folder-row]"), function (row) {
      row.classList.toggle("is-selected", folderRowSelected(row));
    });
    syncAnchorRow();
  }

  /* Portée dossier absente du serveur : tous les ids enfants sont chargés en fond. */
  function fetchAllFolderIds(info) {
    if (!info || info.total <= info.count) {
      return window.Promise.resolve();
    }
    var controller = new window.AbortController();
    folderSelectAllController = controller;
    return fetchFolderRangeIds(info, 0, info.total - 1, controller.signal)
      .then(function (ids) {
        if (folderSelectAllController !== controller) {
          return;
        }
        folderSelectAllController = null;
        each(ids, function (id) {
          if (!folderExcludedMap[id]) {
            selectionRemember(selectedFolders, id);
          }
        });
        refreshFolderSelectedClasses();
      })
      .catch(function (error) {
        if (folderSelectAllController === controller) {
          folderSelectAllController = null;
        }
        if (error && error.name === "AbortError") {
          return;
        }
        /* Repli : les lignes dossier rendues restent sélectionnées. */
      });
  }

  /* Gardes communes aux plages fichier/dossier : AbortError avant génération, repli propre à l'appelant. */
  function requestRange(config) {
    var generation = config.generation();
    var controller = new window.AbortController();
    config.setController(controller);
    config
      .fetch(controller.signal)
      .then(function (entries) {
        if (generation !== config.generation()) {
          return;
        }
        config.setController(null);
        each(entries, config.apply);
        config.refresh();
      })
      .catch(function (error) {
        if (error && error.name === "AbortError") {
          return;
        }
        if (generation !== config.generation()) {
          return;
        }
        config.setController(null);
        if (config.fallback) {
          config.fallback();
        }
      });
  }

  function requestFolderIndexRange(info, from, to, selected) {
    requestRange({
      generation: function () {
        return folderRangeGeneration;
      },
      setController: function (controller) {
        folderRangeController = controller;
      },
      fetch: function (signal) {
        return fetchFolderRangeIds(info, from, to, signal);
      },
      apply: function (id) {
        if (selected) {
          selectionRemember(selectedFolders, id);
        } else {
          selectionForget(selectedFolders, id);
        }
      },
      /* Pas de repli : les lignes dossier visibles ont déjà reçu leur classe. */
      refresh: refreshFolderSelectedClasses
    });
  }

  function refreshSelectedClasses() {
    each(rows(), function (row) {
      row.classList.toggle("is-selected", rowSelected(row));
    });
    syncAnchorRow();
  }

  function requestIndexRange(from, to, selected) {
    var info = rowsInfo();
    requestRange({
      generation: function () {
        return rangeGeneration;
      },
      setController: function (controller) {
        rangeController = controller;
      },
      fetch: function (signal) {
        return fetchRangeEntries(info.url, from, to, signal);
      },
      apply: function (entry) {
        if (selected) {
          selectionRemember(selectedFiles, entry.id, entry.kind);
        } else {
          selectionForget(selectedFiles, entry.id);
        }
      },
      refresh: refreshSelectedClasses,
      /* Repli : les lignes déjà chargées restent sélectionnables si la requête échoue. */
      fallback: function () {
        applyIndexRange(from, to, selected);
      }
    });
  }

  /* Shift étend depuis l'ancre visuelle : sous-intervalles dossier puis fichier de l'intervalle absolu. */
  function selectRange(row) {
    var target = navIndex(row);
    if (anchorNav === null || target === null) {
      return false;
    }
    var selected = isFolderNavRow(row) ? !folderRowSelected(row) : !rowSelected(row);
    var from = Math.min(anchorNav, target);
    var to = Math.max(anchorNav, target);
    dropSelectAll();
    clearRangeRequest();
    setAnchor(row);
    /* Partie dossier : index unifiés `up` + index dossier. */
    var up = upRowCount();
    var folders = folderCount();
    var folderFrom = Math.max(from, up) - up;
    var folderTo = Math.min(to, up + folders - 1) - up;
    if (folderTo >= folderFrom) {
      each(qsa(".table__row[data-folder-row]"), function (candidate) {
        var index = folderIndex(candidate);
        if (index !== null && index >= folderFrom && index <= folderTo) {
          setFolderRowSelected(candidate, selected);
        }
      });
      var folderInfo = folderRowsInfo();
      if (
        folderInfo &&
        !(folderInfo.first <= folderFrom && folderInfo.first + folderInfo.count > folderTo)
      ) {
        requestFolderIndexRange(folderInfo, folderFrom, folderTo, selected);
      }
    }
    var offset = navOffset();
    var fileFrom = Math.max(from, offset) - offset;
    var fileTo = to - offset;
    if (fileTo >= fileFrom) {
      var info = rowsInfo();
      if (!info || (info.first <= fileFrom && info.first + info.count > fileTo)) {
        applyIndexRange(fileFrom, fileTo, selected);
      } else {
        requestIndexRange(fileFrom, fileTo, selected);
      }
    }
    return true;
  }

  /* Menu contextuel : fichiers, lignes dossier et zone vide sous la liste. */
  var fileMenuContext = null;

  function fileContextMenu() {
    return qs("#file-context-menu");
  }

  function folderCreateForm() {
    return qs("[data-folder-create]");
  }

  function folderCurrentId() {
    var page = qs("#files-page");
    return page ? page.getAttribute("data-folder-current") || "" : "";
  }

  function fileRowById(id) {
    return rowById("[data-file-drop][data-file-id]", "data-file-id", id);
  }

  /* Index des lignes rendues : évite un parcours DOM par id sur une grande sélection. */
  function loadedRowsById() {
    var loaded = Object.create(null);
    each(qsa("[data-file-drop][data-file-id]"), function (row) {
      loaded[rowId(row)] = row;
    });
    return loaded;
  }

  function selectOnlyFileRow(row) {
    clearRangeRequest();
    clearSelectedFiles();
    clearSelectedFolders();
    setRowSelected(row, true);
    setAnchor(row);
  }

  /* Clic hors ligne et hors menu contextuel : vide la selection fichiers et dossiers. */
  function clearFileSelection() {
    /* Mode déplacement : la sélection traverse les navigations jusqu'au dépôt ou à l'annulation. */
    if (moveMode) {
      return;
    }
    clearRangeRequest();
    if (
      selectedFiles.ids.length === 0 &&
      selectedFolders.ids.length === 0 &&
      !selectAll &&
      qsa(".table__row.is-selected").length === 0
    ) {
      clearAnchor();
      return;
    }
    clearSelectedFiles();
    clearSelectedFolders();
    clearAnchor();
  }

  function fileMenuIds(row) {
    if (!row) {
      return selectedFileIds();
    }
    if (!rowSelected(row)) {
      selectOnlyFileRow(row);
    }
    return selectedFileIds();
  }

  function fileMenuTargets(ids) {
    var loaded = loadedRowsById();
    var row = ids.length === 1 ? loaded[ids[0]] || null : null;
    var canDelete = ids.length > 0;
    each(ids, function (id) {
      var candidate = loaded[id];
      var kind = selectedFiles.kinds[id] || (candidate ? rowKind(candidate) : null);
      if (kind === "directory") {
        canDelete = false;
      }
    });
    var kind = selectedFiles.kinds[ids[0]] || (row ? rowKind(row) : null);
    var canRename = Boolean(row) && kind !== "directory";
    return {
      ids: ids,
      row: row,
      canRename: canRename,
      canDelete: canDelete
    };
  }

  function setFileMenuItem(menu, action, enabled) {
    var item = qs("[data-file-action='" + action + "']", menu);
    if (!item) {
      return;
    }
    item.disabled = !enabled;
    var hint = item.getAttribute("data-disabled-title");
    if (enabled) {
      item.removeAttribute("title");
    } else if (hint) {
      item.title = hint;
    }
  }

  function closeFileMenu() {
    var menu = fileContextMenu();
    if (!menu || menu.hidden) {
      return false;
    }
    menu.hidden = true;
    fileMenuContext = null;
    return true;
  }

  function setMenuItems(menu, items, move) {
    each(items, function (entry) {
      var item = qs(entry[0], menu);
      if (item) {
        item.hidden = move ? entry[2] : entry[1];
      }
    });
  }

  function showMenu(menu, x, y) {
    menu.hidden = false;
    menu.style.left = "0px";
    menu.style.top = "0px";
    var bounds = menu.getBoundingClientRect();
    var margin = 8;
    var left = Math.min(x, window.innerWidth - bounds.width - margin);
    var top = Math.min(y, window.innerHeight - bounds.height - margin);
    menu.style.left = Math.max(margin, left) + "px";
    menu.style.top = Math.max(margin, top) + "px";
  }

  function enterMoveMode() {
    if (!hasSelection()) {
      return;
    }
    moveMode = true;
    closeFileMenu();
  }

  function cancelMoveMode() {
    moveMode = false;
    closeFileMenu();
  }

  /* Corps FormData : clés répétées `file_ids`/`folder_ids`, jamais un tableau JSON. */
  function moveSelectionBody(target, files, folders) {
    var body = new window.FormData();
    body.append("folder", target);
    each(files, function (id) {
      body.append("file_ids", id);
    });
    each(folders, function (id) {
      body.append("folder_ids", id);
    });
    var scope = selectionScope();
    if (scope) {
      body.append("scope", scope.scope);
      body.append("scope_url", scope.scope_url);
      each(scope.exclude_ids, function (id) {
        body.append("exclude_ids", id);
      });
    }
    return body;
  }

  /* Toast 200 d'erreur : la sélection reste pour réessayer. */
  function clearSelectionUnlessToastError(xhr) {
    if (
      !xhr ||
      typeof xhr.responseText !== "string" ||
      xhr.responseText.indexOf("toast--error") === -1
    ) {
      clearFileSelection();
    }
  }

  /* Un seul POST pour fichiers et dossiers : la sélection n'est vidée qu'après un succès. */
  function moveSelection(target) {
    folderSelectionReady().then(function () {
      var files = selectedFileIds();
      var folders = selectedFolderIds();
      if (!window.htmx || (files.length === 0 && folders.length === 0 && !selectAll)) {
        return;
      }
      var body = moveSelectionBody(target, files, folders);
      cancelMoveMode();
      window.htmx
        .ajax("POST", "/partials/virtual-folders/move-selection", {
          target: "#toast",
          swap: "innerHTML",
          values: body
        })
        .then(clearSelectionUnlessToastError)
        .catch(function () {
          /* Panne réseau : la sélection reste pour réessayer, le mode déplacement est déjà annulé. */
          app.showToast("Error", true);
        });
    });
  }

  function openContextMenu(x, y, context) {
    var menu = fileContextMenu();
    if (!menu) {
      return;
    }
    closeFileRename(null);
    var move = moveMode;
    var isFile = !move && context.kind === "file";
    var isFolder = !move && context.kind === "folder";
    var canCreate =
      !move &&
      Boolean(folderCreateForm()) &&
      (isFile || isFolder || context.kind === "blank");
    var canFileRename = isFile && context.canRename;
    var canFileDelete = isFile && context.canDelete;
    var canMove = !move && hasSelection();
    if (!move && !canCreate && !canFileRename && !canFileDelete && !isFolder && !canMove) {
      closeFileMenu();
      return;
    }
    setMenuItems(
      menu,
      [
        ["[data-folder-action='create']", !canCreate, true],
        ["[data-folder-action='rename']", !isFolder, true],
        ["[data-folder-action='delete']", !isFolder, true],
        ["[data-file-action='rename']", !isFile, true],
        ["[data-file-action='delete']", !isFile, true],
        ["[data-file-action='move']", !canMove, true],
        ["[data-file-action='move-here']", true, false],
        ["[data-file-action='move-cancel']", true, false]
      ],
      move
    );
    if (!move && isFile) {
      setFileMenuItem(menu, "rename", canFileRename);
      setFileMenuItem(menu, "delete", canFileDelete);
    }
    showMenu(menu, x, y);
    fileMenuContext = move ? { kind: "move", folder: context.folder || "" } : context;
  }

  function closeFileRename(except) {
    each(qsa(".table__row.is-renaming"), function (row) {
      if (row === except) {
        return;
      }
      row.classList.remove("is-renaming");
      var form = qs("[data-file-rename]", row);
      if (form) {
        form.hidden = true;
      }
    });
  }

  /* Sélectionne le nom sans l'extension, un point initial ne compte pas comme extension. */
  function focusFileRename(form) {
    var input = focusName(form);
    if (!input) {
      return;
    }
    var value = input.value;
    var dot = value.lastIndexOf(".");
    input.setSelectionRange(0, dot > 0 ? dot : value.length);
  }

  function startFileRename(context) {
    if (!context || !context.row || context.ids.length !== 1) {
      return;
    }
    var row = context.row;
    closeFileRename(row);
    var form = qs("[data-file-rename]", row);
    if (!form) {
      return;
    }
    row.classList.add("is-renaming");
    form.hidden = false;
    focusFileRename(form);
  }

  /* POST suppression + repli toast : URL propre à l'appelant, `onSuccess` optionnel. */
  function postDeleteToast(url, values, onSuccess) {
    var menu = fileContextMenu();
    var request = window.htmx.ajax("POST", url, {
      target: "#toast",
      swap: "innerHTML",
      values: values
    });
    if (onSuccess) {
      request = request.then(onSuccess);
    }
    request.catch(function () {
      var fallback = menu ? menu.getAttribute("data-delete-error") : "";
      app.showToast(fallback || "Error", true);
    });
  }

  function deleteFiles(ids) {
    if (ids.length === 0 || !window.htmx) {
      return;
    }
    postDeleteToast("/partials/files/delete", addSelectionScope({ ids: ids }));
  }

  /* Suppression groupée : un seul POST évite la file `queue last` que htmx applique au corps. */
  function deleteEntries(files, folders) {
    if (!window.htmx || (files.length === 0 && folders.length === 0)) {
      return;
    }
    postDeleteToast(
      "/partials/virtual-folders/delete-selection",
      addSelectionScope({ file_ids: files, folder_ids: folders }),
      clearSelectionUnlessToastError
    );
  }

  /* Suppression mixte : fichiers puis dossiers sélectionnés, chaque dossier emporte son sous-arbre. */
  function deleteSelection() {
    if (moveMode) {
      return;
    }
    folderSelectionReady().then(function () {
      var ids = [];
      var blocked = false;
      var loaded = loadedRowsById();
      each(selectedFiles.ids.slice(), function (id) {
        var row = loaded[id];
        var kind =
          selectedFiles.kinds[id] || (row ? row.getAttribute("data-file-kind") : null);
        if (kind === "directory") {
          blocked = true;
          return;
        }
        ids.push(id);
      });
      var folders = selectedFolderIds();
      if (blocked || (ids.length === 0 && folders.length === 0)) {
        return;
      }
      closeFileMenu();
      deleteEntries(ids, folders);
    });
  }

  /* Entrée : ouvre le dossier actif, jamais un fichier. */
  function openActiveRow() {
    var row = activeNavRow() || app.lastSelectRow;
    if (!row || !isFolderNavRow(row)) {
      return false;
    }
    var link = row.querySelector("a[data-folder-nav]");
    if (!link) {
      return false;
    }
    navigateFolder(link);
    return true;
  }

  /* F2 : renommage inline de la ligne active, hors mode déplacement. */
  function renameActiveRow() {
    if (moveMode) {
      return false;
    }
    var row = activeNavRow() || app.lastSelectRow;
    if (!row || isUpRow(row)) {
      return false;
    }
    if (row.hasAttribute("data-folder-row")) {
      var folderId = folderRowId(row);
      if (folderId && app.folderOpenRename) {
        app.folderOpenRename(folderId);
        return true;
      }
      return false;
    }
    var id = rowId(row);
    if (!id || rowKind(row) === "directory") {
      return false;
    }
    if (!rowSelected(row)) {
      selectOnlyFileRow(row);
    }
    startFileRename({ row: row, ids: [id], canRename: true });
    return true;
  }

  document.addEventListener("contextmenu", function (event) {
    var target = event.target;
    if (!(target instanceof Element)) {
      return;
    }
    if (!target.closest("#files-page") || target.closest("#file-context-menu")) {
      closeFileMenu();
      return;
    }
    if (target.closest("input, textarea, select")) {
      return;
    }
    /* Mode déplacement prioritaire : une ligne dossier est une cible, pas une action. */
    if (moveMode) {
      var folderTarget = target.closest("[data-folder-drop]");
      event.preventDefault();
      openContextMenu(event.clientX, event.clientY, {
        kind: "move",
        folder: folderTarget
          ? folderTarget.getAttribute("data-folder-drop") || ""
          : folderCurrentId()
      });
      return;
    }
    var folderRow = target.closest("[data-folder-row]");
    if (folderRow) {
      event.preventDefault();
      if (!folderRowSelected(folderRow)) {
        selectOnlyFolderRow(folderRow);
      }
      openContextMenu(event.clientX, event.clientY, {
        kind: "folder",
        row: folderRow,
        folder: folderRow.getAttribute("data-folder-row") || ""
      });
      return;
    }
    var row = target.closest("[data-file-drop][data-file-id]");
    if (row && row.getAttribute("data-file-kind") === "directory") {
      closeFileMenu();
      return;
    }
    if (!row) {
      if (!folderCreateForm()) {
        closeFileMenu();
        return;
      }
      event.preventDefault();
      openContextMenu(event.clientX, event.clientY, { kind: "blank" });
      return;
    }
    var ids = fileMenuIds(row);
    if (ids.length === 0) {
      closeFileMenu();
      return;
    }
    var targets = fileMenuTargets(ids);
    event.preventDefault();
    openContextMenu(event.clientX, event.clientY, {
      kind: "file",
      ids: targets.ids,
      row: targets.row,
      canRename: targets.canRename,
      canDelete: targets.canDelete
    });
  });

  document.addEventListener("click", function (event) {
    var target = event.target;
    if (!(target instanceof Element)) {
      return;
    }
    var folderItem = target.closest("[data-folder-action]");
    if (folderItem) {
      if (!fileMenuContext) {
        return;
      }
      event.preventDefault();
      var folderAction = folderItem.getAttribute("data-folder-action");
      var folderContext = fileMenuContext;
      closeFileMenu();
      if (folderAction === "create") {
        /* Création au niveau courant : la ligne cliquée sert d'ancre, pas de parent. */
        var anchor =
          folderContext.kind === "folder"
            ? folderContext.row.getAttribute("data-folder-row") || ""
            : "";
        if (app.folderCreate) {
          app.folderCreate(folderCurrentId(), anchor);
        }
      } else if (folderAction === "rename") {
        if (folderContext.kind === "folder" && app.folderOpenRename) {
          app.folderOpenRename(
            folderContext.row.getAttribute("data-folder-row") || ""
          );
        }
      } else if (folderAction === "delete") {
        /* Suppression commune fichiers + dossiers : toute la sélection, pas seulement la ligne cliquée. */
        if (folderContext.kind === "folder") {
          deleteSelection();
        }
      }
      return;
    }
    var item = target.closest("[data-file-action]");
    if (item) {
      if (!fileMenuContext) {
        return;
      }
      event.preventDefault();
      var action = item.getAttribute("data-file-action");
      var context = fileMenuContext;
      if (action === "move") {
        enterMoveMode();
        return;
      }
      if (action === "move-here") {
        closeFileMenu();
        moveSelection(context.folder || "");
        return;
      }
      if (action === "move-cancel") {
        cancelMoveMode();
        clearFileSelection();
        return;
      }
      if (context.kind !== "file") {
        return;
      }
      closeFileMenu();
      if (action === "rename") {
        startFileRename(context);
      } else if (action === "delete") {
        deleteFiles(context.ids);
      }
      return;
    }
    var menu = fileContextMenu();
    if (menu && !menu.hidden && !menu.contains(target)) {
      closeFileMenu();
    }
    if (moveMode && !target.closest("#file-context-menu")) {
      cancelMoveMode();
      clearFileSelection();
    }
  });

  /* Ligne dossier ou ligne `..`, hors champs de renommage : cible de la sélection. */
  function folderNavRow(target) {
    if (!(target instanceof Element)) {
      return null;
    }
    var row = target.closest("[data-folder-row], [data-folder-up]");
    if (!row || row.classList.contains("is-renaming")) {
      return null;
    }
    if (
      target.closest(
        "input, button, textarea, select, label, [data-folder-action], [data-folder-rename], [data-remove-tag]"
      )
    ) {
      return null;
    }
    return row;
  }

  /* Double-clic : navigation htmx, sélection purgée hors mode déplacement. */
  function navigateFolder(link) {
    clearFileSelection();
    if (!link) {
      return;
    }
    if (!window.htmx) {
      window.location.assign(link.href);
      return;
    }
    window.htmx.ajax("GET", link.getAttribute("hx-get") || link.href, {
      target: link.getAttribute("hx-target") || "#files-page",
      swap: link.getAttribute("hx-swap") || "outerHTML",
      select: link.getAttribute("hx-select") || "#files-page",
      push:
        link.getAttribute("hx-push-url") === "true"
          ? link.getAttribute("hx-get") || link.pathname
          : false
    });
  }

  /* Ctrl/clic ou shift/clic sur une ligne dossier ne doit pas ouvrir le lien dans un onglet. */
  document.addEventListener("mousedown", function (event) {
    var target = event.target;
    if (
      (event.ctrlKey || event.metaKey || event.shiftKey) &&
      target instanceof Element &&
      target.closest("[data-folder-row], [data-folder-up]") &&
      !target.closest("button, input, textarea, select, [data-folder-action]")
    ) {
      event.preventDefault();
    }
  });

  /* Clic souris : sélection seule. Capture pour couper htmx et le href avant le lien. */
  document.addEventListener(
    "click",
    function (event) {
      var row = folderNavRow(event.target);
      if (!row) {
        return;
      }
      /* detail 0 : clic clavier (Entrée) → le lien reste maître de la navigation. */
      if (event.detail === 0) {
        return;
      }
      event.preventDefault();
      event.stopPropagation();
      closeFileMenu();
      if (moveMode) {
        /* Mode déplacement : simple clic inerte, le double-clic navigue sans annuler. */
        return;
      }
      if (row.hasAttribute("data-folder-up")) {
        return;
      }
      if (event.shiftKey && app.selectRange && app.selectRange(row)) {
        return;
      }
      if (event.ctrlKey || event.metaKey) {
        setFolderRowSelected(row, !folderRowSelected(row));
        setAnchor(row);
        return;
      }
      selectOnlyFolderRow(row);
    },
    true
  );

  /* Double-clic : ouvre le dossier (ou remonte pour `..`) après les deux clics de sélection. */
  document.addEventListener("dblclick", function (event) {
    var row = folderNavRow(event.target);
    if (!row) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    navigateFolder(row.querySelector("a[data-folder-nav]"));
  });

  /* Navigation htmx au clavier vers un dossier : hors mode déplacement la sélection ne survit pas. */
  document.addEventListener("click", function (event) {
    var target = event.target;
    if (!(target instanceof Element) || !target.closest("[data-folder-nav]")) {
      return;
    }
    clearFileSelection();
  });

  document.addEventListener("keydown", function (event) {
    if (event.key !== "Escape") {
      return;
    }
    if (moveMode) {
      cancelMoveMode();
      clearFileSelection();
      return;
    }
    if (closeFileMenu()) {
      return;
    }
    closeFileRename(null);
  });

  window.addEventListener("scroll", closeFileMenu, true);
  window.addEventListener("resize", closeFileMenu);

  document.body.addEventListener("htmx:beforeSwap", function (event) {
    var target = event.target;
    if (target instanceof Element && target.id === "files-page") {
      closeFileMenu();
      closeFileRename(null);
    }
  });

  /* Retour/avant navigateur : nettoyage hors mode déplacement, la sélection reste en mode déplacement. */
  document.addEventListener("htmx:historyRestore", function () {
    clearFileSelection();
  });

  /* Réapplique la sélection dossier sur les lignes rendues, purge seulement sur page complète. */
  function restoreFolderSelection(prune) {
    var present = Object.create(null);
    each(qsa(".table__row[data-folder-row]"), function (row) {
      var id = folderRowId(row);
      var selected = folderRowSelected(row);
      if (id) {
        present[id] = true;
      }
      if (id && selected && selectAllFolders) {
        selectionRemember(selectedFolders, id);
      }
      row.classList.toggle("is-selected", selected);
    });
    if (!prune || moveMode) {
      return;
    }
    var info = folderRowsInfo();
    /* Scroll virtuel : une absence du DOM ne prouve pas la disparition du dossier. */
    if (info && info.count < info.total) {
      return;
    }
    each(selectedFolders.ids.slice(), function (id) {
      if (!present[id]) {
        selectionForget(selectedFolders, id);
      }
    });
  }

  function restoreSelection(prune) {
    var present = Object.create(null);
    each(rows(), function (row) {
      var id = rowId(row);
      var selected = rowSelected(row);
      if (id) {
        present[id] = true;
      }
      row.classList.toggle("is-selected", selected);
      if (selected) {
        selectionRemember(selectedFiles, id, rowKind(row));
      }
    });
    restoreFolderSelection(prune);
    /* Mode déplacement : une absence du DOM ne prouve pas la disparition, l'id reste sélectionnable. */
    if (!prune || moveMode) {
      syncAnchorRow();
      return;
    }
    var info = rowsInfo();
    /* Scroll virtuel : le DOM ne porte qu'une plage, une absence ne prouve pas la disparition. */
    if (info && info.count < info.total) {
      syncAnchorRow();
      return;
    }
    /* Rechargement complet : les ids absents du DOM ne sont plus sélectionnables. */
    each(selectedFiles.ids.slice(), function (id) {
      if (!present[id]) {
        selectionForget(selectedFiles, id);
      }
    });
    if (anchorId && !present[anchorId]) {
      clearAnchor();
      return;
    }
    syncAnchorRow();
  }

  app.lastSelectRow = null;
  app.moveRow = moveRow;
  app.toggleRowSelection = toggleRowSelection;
  app.selectAllFiles = selectAllFiles;
  app.selectionScope = selectionScope;
  app.addSelectionScope = addSelectionScope;
  app.deleteSelection = deleteSelection;
  app.openActiveRow = openActiveRow;
  app.renameActiveRow = renameActiveRow;
  app.rowSelected = rowSelected;
  app.setRowSelected = setRowSelected;
  app.setSelectionAnchor = setAnchor;
  app.selectRange = selectRange;
  app.selectOnlyFileRow = selectOnlyFileRow;
  app.selectedFileIds = selectedFileIds;
  app.selectedFolderIds = selectedFolderIds;
  app.folderSelectionReady = folderSelectionReady;
  app.outOfBandIds = outOfBandIds;
  app.clearFileSelection = clearFileSelection;
  app.restoreSelection = restoreSelection;
  window.fsEz.app = app;
})();
