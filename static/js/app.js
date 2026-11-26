/* Entrée fs-ez : démarrage des événements serveur et hauteur du bandeau, après les modules métier. */
(function () {
  "use strict";

  var app = window.fsEz.app || {};
  var qs = window.fsEz.qs;

  /* Un import WebDAV émet un événement par fichier : un seul rafraîchissement final évite les listes partielles. */
  var REFRESH_DELAY = 200;
  /* htmx remplace `#files-page` pendant le swap, une requête en file sur l'ancien élément serait perdue. */
  var REFRESH_BUSY = "#files-page.htmx-request, #sidebar.htmx-request";

  function startEvents() {
    if (typeof window.EventSource !== "function") {
      return;
    }
    var names = [
      "data-changed",
      "inbox-changed",
      "import-finished",
      "resync"
    ];
    var pending = {};
    var timer = null;

    function refresh() {
      timer = null;
      var triggered = Object.keys(pending);
      if (triggered.length === 0) {
        return;
      }
      if (document.querySelector(REFRESH_BUSY)) {
        timer = window.setTimeout(refresh, 100);
        return;
      }
      pending = {};
      if (!window.htmx) {
        return;
      }
      for (var index = 0; index < triggered.length; index += 1) {
        window.htmx.trigger(document.body, triggered[index]);
      }
    }

    function schedule(name) {
      pending[name] = true;
      if (timer !== null) {
        window.clearTimeout(timer);
      }
      timer = window.setTimeout(refresh, REFRESH_DELAY);
    }

    var source = new EventSource("/api/events");
    for (var index = 0; index < names.length; index += 1) {
      source.addEventListener(names[index], function (event) {
        if (event.type === "resync") {
          window.location.reload();
          return;
        }
        schedule(event.type);
      });
    }
  }


  var railMedia = window.matchMedia("(min-width: 48rem)");
  if (railMedia.addEventListener) {
    railMedia.addEventListener("change", app.syncSidebarToggle);
  } else if (railMedia.addListener) {
    railMedia.addListener(app.syncSidebarToggle);
  }
  app.syncSidebarToggle();
  app.restoreGroupState(document);
  app.initColumnTables(document);
  app.initDropdowns(document);
  var createForm = qs("[data-view-create-form]");
  if (createForm) {
    app.placeViewCreatePanel(createForm);
  }

  startEvents();
})();
