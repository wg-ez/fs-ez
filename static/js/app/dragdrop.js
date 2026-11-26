/* Glisser-déposer : réordonnancement des tags/groupes et affectation de tags aux fichiers. */
(function () {
  "use strict";

  var app = window.fsEz.app || {};
  var qsa = window.fsEz.qsa;
  var each = window.fsEz.each;

  function fileDropTarget(target) {
    if (!(target instanceof Element)) {
      return null;
    }
    var row = target.closest("[data-file-drop]");
    if (row) {
      return row;
    }
    /* Dossier virtuel : la cible traite le drop elle-même, jamais la zone de sélection. */
    if (target.closest("[data-folder-drop]")) {
      return null;
    }
    return target.closest("[data-file-drop-zone]");
  }

  /* Cible de drop : une ligne hors sélection reste locale, une zone couvre la sélection. */
  function fileDropSelection(element) {
    var selected = app.selectedFileIds();
    if (!element.hasAttribute("data-file-drop")) {
      return { ids: selected, whole: true };
    }
    var id = element.getAttribute("data-file-id");
    if (!id) {
      return { ids: [], whole: false };
    }
    return selected.indexOf(id) === -1
      ? { ids: [id], whole: false }
      : { ids: selected, whole: true };
  }

  /* Erreur de drop de tag, message local `data-drop-error` sinon repli `Error`. */
  function tagDropFailure(scope) {
    return function () {
      var zone = scope.closest("[data-file-drop-zone]");
      app.showToast((zone ? zone.getAttribute("data-drop-error") : "") || "Error", true);
    };
  }

  /* POST groupé de tags sur `/partials/files/tags`, erreur affichée dans `#toast`. */
  function postTagValues(values, scope) {
    window.htmx
      .ajax("POST", "/partials/files/tags", {
        target: "#toast",
        swap: "innerHTML",
        values: values
      })
      .catch(tagDropFailure(scope));
  }

  function tagFiles(selection, tag, target) {
    if (selection.ids.length === 0 || !tag || !window.htmx) {
      return;
    }
    var ready = app.folderSelectionReady ? app.folderSelectionReady() : window.Promise.resolve();
    ready.then(function () {
      var values = {
        ids: selection.ids,
        oob_ids: app.outOfBandIds ? app.outOfBandIds(selection.ids) : selection.ids,
        tags: tag,
        action: "add"
      };
      if (selection.whole && app.addSelectionScope) {
        app.addSelectionScope(values);
      }
      /* Sélection mixte : les dossiers sélectionnés sont tagués récursivement avec les fichiers. */
      if (selection.whole && app.selectedFolderIds) {
        var folders = app.selectedFolderIds();
        if (folders.length > 0) {
          values.folder_ids = folders;
        }
      }
      postTagValues(values, target);
    });
  }

  /* Double-clic sur une pastille : le premier clic ne fait rien, le second retire le tag. */
  function removeFileTag(pill) {
    var ready = app.folderSelectionReady ? app.folderSelectionReady() : window.Promise.resolve();
    ready.then(function () {
      removeFileTagNow(pill);
    });
  }

  function removeFileTagNow(pill) {
    var id = pill.getAttribute("data-file-id");
    var tag = pill.getAttribute("data-tag-label");
    var cell = pill.closest("[id^='file-tags-']");
    if (!window.htmx || !id || !tag || !cell) {
      return;
    }
    var selected = app.selectedFileIds();
    var folders = app.selectedFolderIds ? app.selectedFolderIds() : [];
    var inSelection = selected.indexOf(id) !== -1;
    /* Ligne exclue de Ctrl+A ou hors sélection : retrait limité à cette ligne. */
    if (inSelection && (selected.length > 1 || app.selectionScope() || folders.length > 0)) {
      var values = {
        ids: selected,
        oob_ids: app.outOfBandIds ? app.outOfBandIds(selected) : selected,
        tags: tag,
        action: "remove"
      };
      if (app.addSelectionScope) {
        app.addSelectionScope(values);
      }
      if (folders.length > 0) {
        values.folder_ids = folders;
      }
      postTagValues(values, pill);
      return;
    }
    window.htmx
      .ajax("POST", "/partials/file/" + encodeURIComponent(id) + "/tags/remove", {
        target: "#" + cell.id,
        swap: "innerHTML",
        values: { tags: tag }
      })
      .catch(tagDropFailure(pill));
  }

  var dragState = null;

  /* Reorder de groupe : sélecteur, attribut d'id, URL, selon le type glissé. */
  var GROUP_DRAG = {
    group: ["[data-group-drag]", "data-group-drag", "/partials/groups/reorder"],
    "view-group": ["[data-view-group-drag]", "data-view-group-drag", "/partials/view-groups/reorder"]
  };

  function closest(target, selector) {
    return target instanceof Element ? target.closest(selector) : null;
  }

  /* Démarre un glisser : `field` reçoit `value`, `data` alimente `text/plain`. */
  function startDrag(event, kind, field, value, row, effect, data) {
    if (!event.dataTransfer) {
      return false;
    }
    dragState = { kind: kind, row: row };
    dragState[field] = value;
    if (row) {
      row.classList.add("is-dragging");
    }
    event.dataTransfer.setData("text/plain", data);
    event.dataTransfer.effectAllowed = effect;
    return true;
  }

  function endDrag() {
    dragState = null;
    clearDragMarks();
  }

  /* Curseur au-dessus de la moitié haute : insertion avant, sinon après. */
  function isBeforeDrop(event, element) {
    var rect = element.getBoundingClientRect();
    return event.clientY < rect.top + rect.height / 2;
  }

  function dragHead(target) {
    var group =
      closest(target, "[data-group-drag]") ||
      closest(target, "[data-view-group-drag]");
    if (!group) {
      return null;
    }
    return group.querySelector("summary") || group;
  }

  function clearReorderMarks() {
    each(qsa(".is-drop-before, .is-drop-after"), function (mark) {
      mark.classList.remove("is-drop-before", "is-drop-after");
    });
  }

  function clearDragMarks() {
    clearReorderMarks();
    each(qsa(".is-dragging"), function (row) {
      row.classList.remove("is-dragging");
    });
    each(qsa(".drop-target.is-drop, [data-file-drop].is-drop"), function (target) {
      target.classList.remove("is-drop");
    });
  }

  function allowDrop(event, effect) {
    event.preventDefault();
    if (event.dataTransfer) {
      event.dataTransfer.dropEffect = effect;
    }
  }

  /* Une seule cible marquée `is-drop` à la fois, le sélecteur sépare fichiers et dossiers. */
  function markDropTarget(target, selector) {
    each(qsa(selector + ".is-drop"), function (marked) {
      if (marked !== target) {
        marked.classList.remove("is-drop");
      }
    });
    target.classList.add("is-drop");
  }

  function refreshSidebar() {
    if (window.htmx) {
      app.localFilesRefresh = true;
      window.htmx.trigger(document.body, "data-changed");
    }
  }

  function persistOrder(url, body) {
    window.fsEz
      .post(url, {
        headers: { "Content-Type": "application/x-www-form-urlencoded" },
        body: body
      })
      .then(function (response) {
        if (!response.ok) {
          throw new Error("reorder failed");
        }
        refreshSidebar();
      })
      .catch(function () {
        refreshSidebar();
      });
  }

  /* Réordonnancement tag ou vue : même corps, seul le couple URL/clé change. */
  function place(url, key, group, value, before) {
    if (!group || !value) {
      return;
    }
    var body = "group=" + encodeURIComponent(group) + "&" + key + "=" + encodeURIComponent(value);
    if (before) {
      body += "&before=" + encodeURIComponent(before);
    }
    persistOrder(url, body);
  }

  function listIds(list, selector, attribute) {
    var ids = [];
    each(qsa(selector, list), function (row) {
      var value = row.getAttribute(attribute);
      if (value) {
        ids.push(value);
      }
    });
    return ids;
  }

  function nextRow(row, attribute) {
    var next = row.nextElementSibling;
    while (next && !next.hasAttribute(attribute)) {
      next = next.nextElementSibling;
    }
    return next;
  }

  document.addEventListener("dragstart", function (event) {
    var tag = closest(event.target, "[data-tag][draggable='true']");
    if (tag) {
      var name = tag.getAttribute("data-tag") || "";
      // Cible fichier = copie, cible groupe = déplacement → copyMove obligatoire.
      if (startDrag(event, "tag", "name", name, tag.closest("[data-tag-row]"), "copyMove", name)) {
        return;
      }
    }
    var view = closest(event.target, "[data-view][draggable='true']");
    if (view) {
      var viewId = view.getAttribute("data-view") || "";
      if (startDrag(event, "view", "id", viewId, view.closest("[data-view-row]"), "move", "")) {
        return;
      }
    }
    var viewGroup = closest(event.target, "[data-view-group-drag]");
    if (viewGroup) {
      if (
        startDrag(
          event,
          "view-group",
          "id",
          viewGroup.getAttribute("data-view-group-drag"),
          viewGroup,
          "move",
          ""
        )
      ) {
        return;
      }
    }
    var group = closest(event.target, "[data-group-drag]");
    if (group) {
      startDrag(
        event,
        "group",
        "id",
        group.getAttribute("data-group-drag"),
        group,
        "move",
        ""
      );
    }
  });

  document.addEventListener("dragover", function (event) {
    if (!dragState) {
      return;
    }
    var groupSpec = GROUP_DRAG[dragState.kind];
    if (groupSpec) {
      var group = closest(event.target, groupSpec[0]);
      if (!group) {
        return;
      }
      allowDrop(event, "move");
      clearReorderMarks();
      var head = dragHead(event.target);
      group.classList.add(
        isBeforeDrop(event, head) ? "is-drop-before" : "is-drop-after"
      );
      return;
    }
    if (dragState.kind === "view") {
      var viewRow = event.target.closest("[data-view-row]");
      if (viewRow) {
        allowDrop(event, "move");
        clearReorderMarks();
        viewRow.classList.add(
          isBeforeDrop(event, viewRow) ? "is-drop-before" : "is-drop-after"
        );
        return;
      }
      var viewHead = closest(event.target, "[data-view-group].drop-target");
      if (!viewHead) {
        return;
      }
      allowDrop(event, "move");
      clearReorderMarks();
      viewHead.classList.add("is-drop");
      return;
    }
    var row = event.target.closest("[data-tag-row]");
    if (row) {
      allowDrop(event, "move");
      clearReorderMarks();
      row.classList.add(
        isBeforeDrop(event, row) ? "is-drop-before" : "is-drop-after"
      );
      return;
    }
    var headTarget = closest(event.target, "[data-group].drop-target");
    if (!headTarget) {
      return;
    }
    allowDrop(event, "move");
    clearReorderMarks();
    headTarget.classList.add("is-drop");
  });

  document.addEventListener("dragleave", function (event) {
    var target = closest(event.target, "[data-group].drop-target");
    if (target && !target.contains(event.relatedTarget)) {
      target.classList.remove("is-drop");
    }
    var viewTarget = closest(event.target, "[data-view-group].drop-target");
    if (viewTarget && !viewTarget.contains(event.relatedTarget)) {
      viewTarget.classList.remove("is-drop");
    }
  });

  document.addEventListener("drop", function (event) {
    if (!dragState) {
      return;
    }
    var groupSpec = GROUP_DRAG[dragState.kind];
    if (groupSpec) {
      var group = closest(event.target, groupSpec[0]);
      if (!group) {
        return;
      }
      event.preventDefault();
      var source = dragState.row;
      var before = group.classList.contains("is-drop-before");
      if (source && source !== group) {
        if (before) {
          group.parentNode.insertBefore(source, group);
        } else {
          group.parentNode.insertBefore(source, group.nextSibling);
        }
        var ids = listIds(document, groupSpec[0], groupSpec[1]);
        persistOrder(groupSpec[2], "ids=" + encodeURIComponent(ids.join(",")));
      }
      endDrag();
      return;
    }
    if (dragState.kind === "view") {
      event.preventDefault();
      var viewRow = event.target.closest("[data-view-row]");
      if (viewRow) {
        var viewList = closest(viewRow, "[data-view-group-list]");
        if (!viewList) {
          return;
        }
        var viewGroupId = viewList.getAttribute("data-view-group-list");
        var beforeView = viewRow.classList.contains("is-drop-before");
        var beforeViewId = "";
        if (beforeView) {
          beforeViewId = viewRow.getAttribute("data-view-id") || "";
        } else {
          var nextView = nextRow(viewRow, "data-view-row");
          beforeViewId = nextView ? nextView.getAttribute("data-view-id") || "" : "";
        }
        place("/partials/views/place", "view", viewGroupId, dragState.id, beforeViewId);
        endDrag();
        return;
      }
      var viewHead = closest(event.target, "[data-view-group].drop-target");
      if (viewHead) {
        place(
          "/partials/views/place",
          "view",
          viewHead.getAttribute("data-view-group"),
          dragState.id,
          ""
        );
        endDrag();
        return;
      }
      return;
    }
    var row = event.target.closest("[data-tag-row]");
    if (row) {
      event.preventDefault();
      var list = closest(row, "[data-group-list]");
      if (!list) {
        return;
      }
      var groupId = list.getAttribute("data-group-list");
      var beforeTag = row.classList.contains("is-drop-before");
      var beforeId = "";
      if (beforeTag) {
        beforeId = row.getAttribute("data-tag-id") || "";
      } else {
        var next = nextRow(row, "data-tag-row");
        beforeId = next ? next.getAttribute("data-tag-id") || "" : "";
      }
      place("/partials/tags/place", "tag", groupId, dragState.name, beforeId);
      endDrag();
      return;
    }
    var headTarget = closest(event.target, "[data-group].drop-target");
    if (headTarget) {
      event.preventDefault();
      place(
        "/partials/tags/place",
        "tag",
        headTarget.getAttribute("data-group"),
        dragState.name,
        ""
      );
      endDrag();
      return;
    }
    // Hors sidebar : le drop de fichier lit `text/plain`, ne pas annuler ici.
  });

  document.addEventListener("dragover", function (event) {
    var target = fileDropTarget(event.target);
    if (!target) {
      return;
    }
    allowDrop(event, "copy");
    if (!target.hasAttribute("data-file-drop")) {
      return;
    }
    markDropTarget(target, "[data-file-drop]");
  });

  document.addEventListener("dragleave", function (event) {
    var target = fileDropTarget(event.target);
    if (target && !target.contains(event.relatedTarget)) {
      target.classList.remove("is-drop");
    }
  });

  document.addEventListener("drop", function (event) {
    var target = fileDropTarget(event.target);
    if (!target) {
      return;
    }
    event.preventDefault();
    target.classList.remove("is-drop");
    var tag = event.dataTransfer ? event.dataTransfer.getData("text/plain") : "";
    if (!tag) {
      return;
    }
    tagFiles(fileDropSelection(target), tag, target);
  });

  document.addEventListener("dragend", endDrag);

  app.removeFileTag = removeFileTag;
  app.allowDrop = allowDrop;
  app.markDropTarget = markDropTarget;
  window.fsEz.app = app;
})();
