/* Coquille et rail, état visuel du repli de la barre latérale et sa persistance serveur. */
(function () {
  "use strict";

  document.documentElement.classList.remove("no-js");

  var app = window.fsEz.app || {};
  var body = document.body;
  var qs = window.fsEz.qs;

  function sidebarToggle() {
    return qs(".js-rail-toggle");
  }

  function shellElement() {
    return qs(".shell");
  }

  function setToggleState(expanded) {
    var toggle = sidebarToggle();
    if (!toggle) {
      return;
    }
    toggle.setAttribute("aria-expanded", expanded ? "true" : "false");
    var label = toggle.getAttribute(
      expanded ? "data-label-collapse" : "data-label-expand"
    );
    if (label) {
      toggle.setAttribute("aria-label", label);
      toggle.setAttribute("title", label);
    }
  }

  function railIsOpen() {
    return body.classList.contains("is-rail-open");
  }

  function sidebarHidden() {
    var shell = shellElement();
    return Boolean(shell && shell.classList.contains("shell--sidebar-hidden"));
  }

  function desktopViewport() {
    return window.matchMedia("(min-width: 48rem)").matches;
  }

  function syncSidebarToggle() {
    if (desktopViewport()) {
      setToggleState(!sidebarHidden());
    } else {
      setToggleState(railIsOpen());
    }
  }

  function setSidebarHidden(hidden) {
    var shell = shellElement();
    if (shell) {
      shell.classList.toggle("shell--sidebar-hidden", hidden);
    }
    setToggleState(!hidden);
  }

  function persistSidebarToggle() {
    if (!window.fetch) {
      return;
    }
    window.fsEz
      .post("/partials/sidebar/toggle", {
        headers: { "HX-Request": "true" },
      })
      .catch(function () {});
  }

  function closeRail() {
    body.classList.remove("is-rail-open");
    syncSidebarToggle();
  }

  function openRail() {
    body.classList.add("is-rail-open");
    syncSidebarToggle();
  }

  app.railIsOpen = railIsOpen;
  app.sidebarHidden = sidebarHidden;
  app.desktopViewport = desktopViewport;
  app.syncSidebarToggle = syncSidebarToggle;
  app.setSidebarHidden = setSidebarHidden;
  app.persistSidebarToggle = persistSidebarToggle;
  app.closeRail = closeRail;
  app.openRail = openRail;
  window.fsEz.app = app;
})();
