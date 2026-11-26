/* Largeurs de colonnes : poignées, clavier, remise à zéro et état localStorage. */
(function () {
  "use strict";

  var app = window.fsEz.app || {};
  var qsa = window.fsEz.qsa;
  var each = window.fsEz.each;
  var readJson = window.fsEz.readJson;
  var writeJson = window.fsEz.writeJson;
  var removeKey = window.fsEz.removeKey;

  var COLUMN_STORAGE_PREFIX = "fs-ez:column-widths:v1:";
  var COLUMN_MIN_WIDTH = 48;

  function columnHeaders(table) {
    if (!table || !table.tHead || table.tHead.rows.length === 0) {
      return [];
    }
    return qsa("th[data-col]", table.tHead.rows[0]);
  }

  function columnStorageKey(table) {
    return (
      COLUMN_STORAGE_PREFIX + (table.getAttribute("data-col-key") || "files")
    );
  }

  function measureCells(cells) {
    var widths = [];
    for (var index = 0; index < cells.length; index += 1) {
      widths.push(cells[index].getBoundingClientRect().width);
    }
    return widths;
  }

  /* Pourcentages de largeur, `null` si le total est nul. */
  function percentageWidths(widths) {
    var total = 0;
    for (var index = 0; index < widths.length; index += 1) {
      total += widths[index];
    }
    if (total <= 0) {
      return null;
    }
    var percentages = [];
    for (var width = 0; width < widths.length; width += 1) {
      percentages.push(((widths[width] / total) * 100).toFixed(4));
    }
    return percentages;
  }

  function applyStoredColumnWidths(table) {
    var stored = readJson(columnStorageKey(table), null);
    if (!stored) {
      return;
    }
    var cells = columnHeaders(table);
    if (cells.length < 2) {
      return;
    }
    var widths = [];
    for (var index = 0; index < cells.length; index += 1) {
      var value = stored[cells[index].getAttribute("data-col")];
      if (typeof value !== "number" || !isFinite(value) || value <= 0) {
        return;
      }
      widths.push(value);
    }
    table.classList.add("table--columns");
    var percentages = percentageWidths(widths);
    if (!percentages) {
      return;
    }
    for (var cell = 0; cell < cells.length; cell += 1) {
      cells[cell].style.width = percentages[cell] + "%";
    }
  }

  function initColumnTables(root) {
    var tables = [];
    if (root instanceof Element && root.matches("table[data-col-resizable]")) {
      tables.push(root);
    }
    each(qsa("table[data-col-resizable]", root), function (table) {
      tables.push(table);
    });
    each(tables, function (table) {
      applyStoredColumnWidths(table);
    });
  }

  function persistColumnWidths(table) {
    var cells = columnHeaders(table);
    var percentages = percentageWidths(measureCells(cells));
    if (!percentages) {
      return;
    }
    var stored = {};
    for (var cell = 0; cell < cells.length; cell += 1) {
      stored[cells[cell].getAttribute("data-col")] = Number(percentages[cell]);
    }
    writeJson(columnStorageKey(table), stored);
  }

  function prepareColumnResize(table) {
    var cells = columnHeaders(table);
    if (cells.length < 2) {
      return null;
    }
    var widths = measureCells(cells);
    for (var index = 0; index < cells.length; index += 1) {
      cells[index].style.width = widths[index] + "px";
    }
    table.classList.add("table--columns");
    return cells;
  }

  function columnPartner(cells, index) {
    return index === cells.length - 1 ? index - 1 : index + 1;
  }

  function resizeColumnPair(cells, index, current, pairTotal) {
    var partner = columnPartner(cells, index);
    var max = pairTotal - COLUMN_MIN_WIDTH;
    if (current < COLUMN_MIN_WIDTH) {
      current = COLUMN_MIN_WIDTH;
    }
    if (current > max) {
      current = max;
    }
    cells[index].style.width = current + "px";
    cells[partner].style.width = pairTotal - current + "px";
  }

  function resizeHandle(event) {
    if (!(event.target instanceof Element)) {
      return null;
    }
    var handle = event.target.closest("[data-col-resizer]");
    if (!handle) {
      return null;
    }
    var table = handle.closest("table[data-col-resizable]");
    if (!table) {
      return null;
    }
    return { handle: handle, table: table };
  }

  function resizeGeometry(handle, table) {
    var cells = prepareColumnResize(table);
    if (!cells) {
      return null;
    }
    var index = cells.indexOf(handle.parentElement);
    if (index < 0) {
      return null;
    }
    var widths = measureCells(cells);
    return {
      cells: cells,
      index: index,
      widths: widths,
      pairTotal: widths[index] + widths[columnPartner(cells, index)]
    };
  }

  function startColumnResize(event) {
    /* `resizeHandle` filtre déjà une cible non Element. */
    if (event.button !== 0) {
      return;
    }
    var target = resizeHandle(event);
    if (!target) {
      return;
    }
    event.preventDefault();
    var geometry = resizeGeometry(target.handle, target.table);
    if (!geometry) {
      return;
    }
    var startX = event.clientX;
    target.table.classList.add("is-resizing");

    function move(moveEvent) {
      resizeColumnPair(
        geometry.cells,
        geometry.index,
        geometry.widths[geometry.index] + moveEvent.clientX - startX,
        geometry.pairTotal
      );
    }

    function finish() {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", finish);
      window.removeEventListener("pointercancel", finish);
      target.table.classList.remove("is-resizing");
      persistColumnWidths(target.table);
    }

    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", finish);
    window.addEventListener("pointercancel", finish);
  }

  function resetColumnWidths(table) {
    removeKey(columnStorageKey(table));
    var cells = columnHeaders(table);
    for (var index = 0; index < cells.length; index += 1) {
      cells[index].style.width = "";
    }
    table.classList.remove("table--columns");
  }

  function columnResizeKey(event) {
    /* `resizeHandle` filtre déjà une cible non Element. */
    var target = resizeHandle(event);
    if (!target) {
      return;
    }
    if (
      event.key !== "ArrowLeft" &&
      event.key !== "ArrowRight" &&
      event.key !== "Home"
    ) {
      return;
    }
    event.preventDefault();
    if (event.key === "Home") {
      resetColumnWidths(target.table);
      return;
    }
    var geometry = resizeGeometry(target.handle, target.table);
    if (!geometry) {
      return;
    }
    var step = event.shiftKey ? 48 : 16;
    var delta = event.key === "ArrowRight" ? step : -step;
    resizeColumnPair(
      geometry.cells,
      geometry.index,
      geometry.widths[geometry.index] + delta,
      geometry.pairTotal
    );
    persistColumnWidths(target.table);
  }

  document.addEventListener("pointerdown", startColumnResize);
  document.addEventListener("keydown", columnResizeKey);

  app.initColumnTables = initColumnTables;
  app.resetColumnWidths = resetColumnWidths;
  window.fsEz.app = app;
})();
