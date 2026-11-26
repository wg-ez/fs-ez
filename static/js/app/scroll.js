/* Scroll virtuel des listes : dossiers puis fichiers, chacun une plage contiguë côté serveur. */
(function () {
  "use strict";

  var app = window.fsEz.app || {};
  var qs = window.fsEz.qs;
  var qsa = window.fsEz.qsa;
  var each = window.fsEz.each;
  var DEFAULT_ROW_HEIGHT = 36;
  /* Marge d'une hauteur de viewport en lignes, de chaque côté de la plage visible. */
  var MARGIN_VIEWPORTS = 1;
  /* Déclenche le chargement avant d'atteindre le bord, en fraction de viewport. */
  var PREFETCH_RATIO = 0.5;
  var MAX_LIMIT = 500;

  var state = null;
  /* Ancre de scroll capturée avant un refresh de `#files-page`, rejouée après le swap. */
  var pendingAnchor = null;
  /* Mutations qui rafraîchissent la liste : on recharge la plage au lieu de remplacer la page. */
  var LOCAL_REFRESH_PREFIXES = [
    "/partials/tags",
    "/partials/groups",
    "/partials/files/",
    "/partials/file/"
  ];

  function clamp(value, min, max) {
    return Math.min(max, Math.max(min, value));
  }

  function listByKind(kind) {
    if (!state) {
      return null;
    }
    for (var index = 0; index < state.lists.length; index += 1) {
      if (state.lists[index].kind === kind) {
        return state.lists[index];
      }
    }
    return null;
  }

  function measureRowHeight(rowsBody) {
    var row = rowsBody.querySelector(".table__row");
    if (!row) {
      return DEFAULT_ROW_HEIGHT;
    }
    var height = row.getBoundingClientRect().height;
    return height > 0 ? height : DEFAULT_ROW_HEIGHT;
  }

  function setSpacer(spacer, height) {
    if (!spacer) {
      return;
    }
    spacer.style.setProperty("--files-spacer-h", Math.max(0, height) + "px");
  }

  /* `total` peut rétrécir après suppression : un offset au-delà laisse un vide géant. */
  function applyListSpacers(list, first, count) {
    var start = clamp(first, 0, list.total);
    setSpacer(list.top, start * list.rowHeight);
    setSpacer(list.bottom, Math.max(0, list.total - start - count) * list.rowHeight);
  }

  /* Haut de la zone propre à la liste : son spacer haut, jamais celui du tableau. */
  function listOrigin(list) {
    return list.top.getBoundingClientRect().top + window.scrollY;
  }

  function rowAtViewportTop(list, origin) {
    if (origin == null) {
      origin = listOrigin(list);
    }
    var row = Math.floor((window.scrollY - origin) / list.rowHeight);
    return clamp(row, 0, Math.max(0, list.total - 1));
  }

  /* Liste la plus proche du haut du viewport, pour restaurer l'ancre au bon endroit. */
  function captureAnchor() {
    if (!state) {
      return null;
    }
    var target = null;
    var origin = 0;
    each(state.lists, function (list) {
      var listTop = listOrigin(list);
      if (
        window.scrollY >= listTop &&
        window.scrollY < listTop + list.total * list.rowHeight
      ) {
        target = list;
        origin = listTop;
      }
    });
    if (!target) {
      return null;
    }
    var index = rowAtViewportTop(target, origin);
    return {
      kind: target.kind,
      index: index,
      offset: window.scrollY - (origin + index * target.rowHeight)
    };
  }

  /* Remonte le spacer haut avant le scroll : la hauteur du document doit couvrir l'ancre. */
  function restoreAnchor(anchor) {
    var target = anchor ? listByKind(anchor.kind) : null;
    if (!target) {
      return;
    }
    /* Suppression massive : l'ancre d'avant le swap dépasse la nouvelle liste. */
    var index = clamp(anchor.index, 0, Math.max(0, target.total - 1));
    applyListSpacers(target, index, target.count);
    window.scrollTo(0, listOrigin(target) + index * target.rowHeight + anchor.offset);
  }

  /* Position verticale de la liste calculée une fois par frame pour les deux consommateurs. */
  function viewportPosition(list) {
    var rows = Math.max(1, Math.ceil((window.innerHeight || 0) / list.rowHeight));
    return { row: rowAtViewportTop(list), rows: rows };
  }

  function listNearViewport(list) {
    var margin = (window.innerHeight || 0) * MARGIN_VIEWPORTS;
    return (
      listOrigin(list) + list.total * list.rowHeight >= window.scrollY - margin &&
      listOrigin(list) <= window.scrollY + (window.innerHeight || 0) + margin
    );
  }

  function viewportCovered(list, position) {
    var top = position.row;
    var visible = position.rows;
    var end = Math.min(top + visible, list.total);
    var rangeEnd = list.first + list.count;
    var ahead = Math.floor(visible * PREFETCH_RATIO);
    var needUp = list.first > 0 && top - list.first < ahead;
    var needDown = rangeEnd < list.total && rangeEnd - end < ahead;
    return !needUp && !needDown;
  }

  function targetRange(list, position) {
    var top = position.row;
    var visible = position.rows;
    var margin = visible * MARGIN_VIEWPORTS;
    var first = clamp(top - margin, 0, list.total);
    var end = clamp(top + visible + margin, 0, list.total);
    if (end <= first) {
      end = Math.min(list.total, first + 1);
    }
    return { first: first, count: Math.min(end - first, MAX_LIMIT) };
  }

  function requestUrl(list, first, limit) {
    var separator = list.rowsUrl.indexOf("?") === -1 ? "?" : "&";
    return list.rowsUrl + separator + "offset=" + first + "&limit=" + limit;
  }

  function setBusy() {
    if (!state) {
      return;
    }
    var busy = false;
    each(state.lists, function (list) {
      if (list.controller) {
        busy = true;
      }
    });
    if (busy) {
      state.page.setAttribute("aria-busy", "true");
      if (state.status) {
        state.status.textContent = state.loadingLabel;
      }
      return;
    }
    state.page.removeAttribute("aria-busy");
    if (state.status) {
      state.status.textContent = "";
    }
  }

  function applyRange(list, first, html) {
    var doc = new window.DOMParser().parseFromString(
      "<table><tbody>" + html + "</tbody></table>",
      "text/html"
    );
    var body = doc.querySelector("tbody");
    var rows = body ? qsa(".table__row", body) : [];
    var count = rows.length;
    /* Spacers avant remplacement : la position document des lignes reste invariante. */
    applyListSpacers(list, first, count);
    while (list.rowsBody.firstChild) {
      list.rowsBody.removeChild(list.rowsBody.firstChild);
    }
    each(rows, function (row) {
      list.rowsBody.appendChild(document.importNode(row, true));
    });
    list.first = first;
    list.count = count;
    /* htmx doit connaître les liens et formulaires insérés hors swap. */
    if (window.htmx) {
      window.htmx.process(list.rowsBody);
    }
    if (app.restoreSelection) {
      app.restoreSelection();
    }
    if (app.lastSelectRow && !document.contains(app.lastSelectRow)) {
      app.lastSelectRow = null;
    }
  }

  function finishRequest(list, generation) {
    if (!state || generation !== list.generation) {
      return;
    }
    list.controller = null;
    setBusy();
    /* Reévalue après la réponse, notamment si le scroll a continué pendant le chargement. */
    schedule();
  }

  function requestRange(list, target) {
    list.generation += 1;
    var generation = list.generation;
    if (list.controller) {
      list.controller.abort();
    }
    var controller = new window.AbortController();
    list.controller = controller;
    setBusy();
    window.fsEz
      .fetchHtml(requestUrl(list, target.first, target.count), {
        signal: controller.signal
      })
      .then(function (html) {
        if (!state || generation !== list.generation) {
          return;
        }
        applyRange(list, target.first, html);
        finishRequest(list, generation);
      })
      .catch(function (error) {
        if (error && error.name === "AbortError") {
          return;
        }
        finishRequest(list, generation);
      });
  }

  function update() {
    if (!state) {
      return;
    }
    each(state.lists, function (list) {
      if (list.controller || !listNearViewport(list)) {
        return;
      }
      var position = viewportPosition(list);
      if (!viewportCovered(list, position)) {
        requestRange(list, targetRange(list, position));
      }
    });
  }

  function schedule() {
    if (!state || state.ticking) {
      return;
    }
    state.ticking = true;
    window.requestAnimationFrame(function () {
      if (!state) {
        return;
      }
      state.ticking = false;
      update();
    });
  }

  function onResize() {
    if (!state) {
      return;
    }
    each(state.lists, function (list) {
      var height = measureRowHeight(list.rowsBody);
      if (height !== list.rowHeight) {
        list.rowHeight = height;
        applyListSpacers(list, list.first, list.count);
      }
    });
    schedule();
  }

  function destroy() {
    if (!state) {
      return;
    }
    each(state.lists, function (list) {
      list.generation += 1;
      if (list.controller) {
        list.controller.abort();
      }
    });
    window.removeEventListener("scroll", schedule);
    window.removeEventListener("resize", onResize);
    if (state.table) {
      state.table.classList.remove("is-virtual");
    }
    state = null;
  }

  function makeList(kind, rowsBody, top, bottom, rowsUrl, total) {
    if (!rowsBody || !top || !bottom || !(total > 0) || !rowsUrl) {
      return null;
    }
    var rows = qsa(".table__row", rowsBody);
    if (rows.length === 0) {
      return null;
    }
    return {
      kind: kind,
      rowsBody: rowsBody,
      top: top,
      bottom: bottom,
      rowsUrl: rowsUrl,
      total: total,
      rowHeight: DEFAULT_ROW_HEIGHT,
      first: 0,
      count: rows.length,
      generation: 0,
      controller: null
    };
  }

  function init() {
    destroy();
    var anchor = pendingAnchor;
    pendingAnchor = null;
    var page = qs("#files-page");
    if (!page) {
      return;
    }
    var table = qs("[data-files-table]", page);
    if (!table) {
      return;
    }
    var lists = [];
    var folderBody = qs("[data-folder-rows]", table);
    if (folderBody) {
      var folderList = makeList(
        "folders",
        folderBody,
        qs("[data-folder-spacer='top']", table),
        qs("[data-folder-spacer='bottom']", table),
        folderBody.getAttribute("data-folder-rows-url") || "",
        parseInt(folderBody.getAttribute("data-folder-total") || "0", 10)
      );
      if (folderList) {
        lists.push(folderList);
      }
    }
    var filesBody = qs("[data-files-rows]", table);
    if (filesBody) {
      var filesList = makeList(
        "files",
        filesBody,
        qs("[data-files-spacer='top']", table),
        qs("[data-files-spacer='bottom']", table),
        page.getAttribute("data-rows-url") || "",
        parseInt(page.getAttribute("data-total") || "0", 10)
      );
      if (filesList) {
        lists.push(filesList);
      }
    }
    if (lists.length === 0) {
      return;
    }
    table.classList.add("is-virtual");
    each(lists, function (list) {
      list.rowHeight = measureRowHeight(list.rowsBody);
    });
    state = {
      page: page,
      table: table,
      status: qs("[data-files-status]", page),
      lists: lists,
      loadingLabel: page.getAttribute("data-loading") || "",
      ticking: false
    };
    each(lists, function (list) { applyListSpacers(list, list.first, list.count); });
    restoreAnchor(anchor);
    window.addEventListener("scroll", schedule, { passive: true });
    window.addEventListener("resize", onResize);
    /* Première frame : relit `scrollY` et charge la plage, y compris après un refresh htmx. */
    schedule();
  }

  function isFilesTarget(event) {
    var target = event.target;
    return (
      target instanceof Element &&
      (target.id === "files-page" || target.id === "work")
    );
  }

  /* Index absolu et URL de plage exposés à la sélection pour les sauts hors DOM. */
  function listRowIndex(list, row) {
    if (!list || !row || !list.rowsBody.contains(row)) {
      return null;
    }
    var index = qsa(".table__row", list.rowsBody).indexOf(row);
    return index === -1 ? null : list.first + index;
  }

  function fileRowIndex(row) {
    return listRowIndex(listByKind("files"), row);
  }

  function folderRowIndex(row) {
    return listRowIndex(listByKind("folders"), row);
  }

  function rowsInfo(kind) {
    var list = listByKind(kind);
    return list
      ? {
          first: list.first,
          count: list.count,
          total: list.total,
          url: list.rowsUrl
        }
      : null;
  }

  function filesRowsInfo() {
    return rowsInfo("files");
  }

  function folderRowsInfo() {
    return rowsInfo("folders");
  }

  /* Fragments `#files-page` d'une réponse retenue sans swap. */
  function freshPage(html) {
    if (!html) {
      return null;
    }
    var doc = new window.DOMParser().parseFromString(html, "text/html");
    return doc.querySelector("#files-page");
  }

  /* Mutation locale : recharge la plage visible, garde scroll et sélection. */
  function refreshVisibleRange(page) {
    var list = listByKind("files");
    if (!list) {
      return false;
    }
    var total = parseInt(page.getAttribute("data-total") || "", 10);
    if (!(total > 0)) {
      return false;
    }
    list.total = total;
    state.page.setAttribute("data-total", String(total));
    var url = page.getAttribute("data-rows-url");
    if (url) {
      list.rowsUrl = url;
    }
    applyListSpacers(list, list.first, list.count);
    requestRange(list, targetRange(list, viewportPosition(list)));
    return true;
  }

  /* Mutation POST ciblant la liste : la réponse déclenche un refresh `#files-page`. */
  function isLocalMutation(detail) {
    if (!detail || !detail.requestConfig) {
      return false;
    }
    if ((detail.requestConfig.verb || "").toLowerCase() === "get") {
      return false;
    }
    var path = detail.pathInfo
      ? detail.pathInfo.requestPath
      : detail.requestConfig.path || "";
    /* Suppression : la purge de sélection passe par un swap complet. */
    if (path === "/partials/files/delete") {
      return false;
    }
    for (var index = 0; index < LOCAL_REFRESH_PREFIXES.length; index += 1) {
      if (path.indexOf(LOCAL_REFRESH_PREFIXES[index]) === 0) {
        return true;
      }
    }
    return false;
  }

  app.fileRowIndex = fileRowIndex;
  app.folderRowIndex = folderRowIndex;
  app.filesRowsInfo = filesRowsInfo;
  app.folderRowsInfo = folderRowsInfo;

  document.addEventListener("DOMContentLoaded", init);
  document.body.addEventListener("htmx:beforeRequest", function (event) {
    if (isLocalMutation(event.detail)) {
      app.localFilesRefresh = true;
    }
  });
  document.body.addEventListener("htmx:beforeSwap", function (event) {
    if (!isFilesTarget(event)) {
      return;
    }
    /* `isFilesTarget` garantit déjà un Element : seul l'id distingue `#work`. */
    if (event.target.id !== "files-page") {
      destroy();
      return;
    }
    if (app.localFilesRefresh) {
      app.localFilesRefresh = false;
      var page = event.detail ? freshPage(event.detail.serverResponse) : null;
      if (state && page && refreshVisibleRange(page)) {
        event.detail.shouldSwap = false;
        return;
      }
    }
    pendingAnchor = captureAnchor();
    destroy();
  });
  document.body.addEventListener("htmx:afterSwap", function (event) {
    if (isFilesTarget(event)) {
      init();
    }
  });
  document.addEventListener("htmx:historyRestore", init);
})();
