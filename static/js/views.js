/* fs-ez — constructeur de filtres des vues sauvegardées. */
(function () {
  "use strict";

  var MAX_DEPTH = 4;
  var CONDITION_OPS = ["tag", "only_tag", "tag_set"];
  var GROUP_OPS = ["and", "or", "not"];
  var qs = window.fsEz.qs;
  var qsa = window.fsEz.qsa;
  var each = window.fsEz.each;

  function isConditionOp(op) {
    return CONDITION_OPS.indexOf(op) !== -1;
  }

  function isGroupOp(op) {
    return GROUP_OPS.indexOf(op) !== -1;
  }

  function inputOf(root) {
    var selector = root.getAttribute("data-filter-input");
    return selector ? qs(selector) : null;
  }

  function rootGroup(root) { return qs(":scope > .builder__group", root); }

  function itemsOf(group) { return qs(":scope > .builder__items", group); }

  function cloneWithDropdowns(root, selector) {
    var template = qs(selector, root);
    var node = template ? template.content.firstElementChild.cloneNode(true) : null;
    if (node && window.fsEz.app && window.fsEz.app.initDropdowns) {
      window.fsEz.app.initDropdowns(node);
    }
    return node;
  }

  function conditionNode(root) {
    return cloneWithDropdowns(root, ".builder-condition-template");
  }

  function groupNode(root) {
    return cloneWithDropdowns(root, ".builder-group-template");
  }

  function logicSelect(condition) { return qs(".builder__logic", condition); }

  function comparator(condition) { return qs(".builder__cmp", condition); }

  function tagsetChips(condition) { return qsa(".builder__tagset-chips > .builder__chip", condition); }

  function tagsetOptions(condition) { return qsa(".builder__tagset-panel input[data-tag-option]", condition); }

  function groupOpSelect(group) { return qs(":scope > .builder__group-head > .builder__group-op", group); }

  function groupOp(group) {
    return group.dataset.op === "or" ? "or" : "and";
  }

  /* Applique l'opérateur au dataset et au select du groupe. */
  function setGroupOp(group, value) {
    var op = value === "or" ? "or" : "and";
    group.dataset.op = op;
    var select = groupOpSelect(group);
    if (select) {
      select.value = op;
    }
    return op;
  }

  function findLast(items, test) {
    var found = null;
    each(items, function (item) {
      if (test(item)) {
        found = item;
      }
    });
    return found;
  }

  function selectedTags(condition) {
    var tags = [];
    each(tagsetChips(condition), function (chip) {
      var tag = chip.getAttribute("data-tag") || "";
      if (tag && tags.indexOf(tag) === -1) {
        tags.push(tag);
      }
    });
    return tags;
  }

  function findTagsetChip(condition, tag) {
    return findLast(tagsetChips(condition), function (chip) {
      return chip.getAttribute("data-tag") === tag;
    });
  }

  function tagsetOption(condition, tag) {
    return findLast(tagsetOptions(condition), function (input) {
      return input.value === tag;
    });
  }

  function addTagsetChip(condition, tag, root) {
    if (findTagsetChip(condition, tag)) {
      return;
    }
    var chips = qs(".builder__tagset-chips", condition);
    if (!chips) {
      return;
    }
    root = root || condition.closest("[data-builder]");
    var removeLabel = root ? root.getAttribute("data-remove-label") || "Remove" : "Remove";
    var option = tagsetOption(condition, tag);
    var color = option ? option.getAttribute("data-color") || "neutral" : "neutral";
    var chip = document.createElement("span");
    chip.className = "builder__chip tag-pill tag-pill--" + color;
    chip.setAttribute("data-tag", tag);
    var label = document.createElement("span");
    label.textContent = tag;
    var remove = document.createElement("button");
    remove.type = "button";
    remove.className = "builder__chip-remove";
    remove.setAttribute("data-remove-chip", "");
    remove.setAttribute("title", removeLabel);
    remove.setAttribute("aria-label", removeLabel);
    remove.textContent = "\u00d7";
    chip.appendChild(label);
    chip.appendChild(remove);
    chips.appendChild(chip);
    if (option) {
      option.checked = true;
    }
  }

  function removeTagsetChip(condition, chip) {
    if (!chip) {
      return;
    }
    var option = tagsetOption(condition, chip.getAttribute("data-tag") || "");
    if (option) {
      option.checked = false;
    }
    if (chip.parentElement) {
      chip.parentElement.removeChild(chip);
    }
  }

  function fillTagset(condition, tags, root) {
    each(tagsetChips(condition), function (chip) {
      chip.parentElement.removeChild(chip);
    });
    each(tagsetOptions(condition), function (option) {
      option.checked = false;
    });
    each(tags || [], function (tag) {
      tag = String(tag).trim();
      if (tag) {
        addTagsetChip(condition, tag, root);
      }
    });
  }

  function syncTagsetOptions(condition) {
    each(tagsetOptions(condition), function (option) {
      option.checked = Boolean(findTagsetChip(condition, option.value));
    });
  }

  function groupDepth(group) {
    var depth = 1;
    var parent = group.parentElement;
    while (parent) {
      if (parent.classList && parent.classList.contains("builder__group")) {
        depth += 1;
      }
      parent = parent.parentElement;
    }
    return depth;
  }

  function syncCondition(condition) {
    var cmp = comparator(condition);
    if (!cmp) {
      return;
    }
    syncTagsetOptions(condition);
    cmp.dispatchEvent(new Event("change"));
  }

  function syncLogic(group) {
    var op = groupOp(group);
    var select = groupOpSelect(group);
    if (select) {
      select.value = op;
      select.dispatchEvent(new Event("change"));
    }
    each(qsa(":scope > .builder__items > .builder__condition", group), function (condition, index) {
      var logic = logicSelect(condition);
      if (!logic) {
        return;
      }
      var where = qs('option[value="where"]', logic);
      if (where) {
        where.disabled = index !== 0;
      }
      logic.disabled = index === 0;
      logic.value = index === 0 ? "where" : op;
      logic.dispatchEvent(new Event("change"));
    });
  }

  function syncEmpty(root) {
    var empty = qs(".builder__empty", root);
    var group = rootGroup(root);
    var items = group ? itemsOf(group) : null;
    if (!empty || !items) {
      return;
    }
    empty.hidden = items.children.length > 0;
  }

  function syncGroup(group) {
    each(qsa(":scope > .builder__items > .builder__condition", group), syncCondition);
    syncLogic(group);
    each(qsa(":scope > .builder__items > .builder__group", group), syncGroup);
  }

  function serializeCondition(condition) {
    var cmpSelect = comparator(condition);
    if (!cmpSelect) {
      return null;
    }
    var tags = selectedTags(condition);
    if (tags.length === 0) {
      return null;
    }
    if (cmpSelect.value === "only" || cmpSelect.value === "not_only") {
      return { op: "tag_set", tags: tags, negated: cmpSelect.value === "not_only" };
    }
    var negated = cmpSelect.value === "has_not";
    if (tags.length === 1) {
      return { op: "tag", tag: tags[0], negated: negated };
    }
    var items = [];
    each(tags, function (tag) {
      items.push({ op: "tag", tag: tag, negated: negated });
    });
    return { op: "and", items: items };
  }

  function serializeGroup(group) {
    var items = itemsOf(group);
    if (!items) {
      return null;
    }
    var children = [];
    each(items.children, function (child) {
      if (child.classList.contains("builder__condition")) {
        var condition = serializeCondition(child);
        if (condition) {
          children.push(condition);
        }
      } else if (child.classList.contains("builder__group")) {
        var nested = serializeGroup(child);
        if (nested) {
          children.push(nested);
        }
      }
    });
    if (children.length === 0) {
      return null;
    }
    return { op: groupOp(group), items: children };
  }

  function serialize(root) {
    var field = inputOf(root);
    if (!field) {
      return;
    }
    var group = rootGroup(root);
    var node = group ? serializeGroup(group) : null;
    field.value = node ? JSON.stringify(node) : "";
    syncEmpty(root);
  }

  function andTags(node) {
    if (!node || node.op !== "and") {
      return null;
    }
    var items = node.items || [];
    if (items.length < 2) {
      return null;
    }
    var tags = [];
    var negated = null;
    for (var index = 0; index < items.length; index += 1) {
      var item = items[index];
      if (!item || item.op !== "tag" || !item.tag) {
        return null;
      }
      var itemNegated = Boolean(item.negated);
      if (negated === null) {
        negated = itemNegated;
      } else if (negated !== itemNegated) {
        return null;
      }
      tags.push(item.tag);
    }
    return { tags: tags, negated: negated };
  }

  function fillTags(condition, tags, negated, root) {
    var cmp = comparator(condition);
    if (!cmp) {
      return;
    }
    cmp.value = negated ? "has_not" : "has";
    fillTagset(condition, tags, root);
    syncCondition(condition);
  }

  function fillCondition(condition, node, root) {
    var cmp = comparator(condition);
    if (!cmp) {
      return;
    }
    if (node.op === "tag_set" || node.op === "only_tag") {
      cmp.value = node.negated ? "not_only" : "only";
      fillTagset(condition, node.tags || (node.tag ? [node.tag] : []), root);
      syncCondition(condition);
      return;
    }
    if (node.op === "tag" && node.tag) {
      fillTags(condition, [node.tag], Boolean(node.negated), root);
    }
  }

  function fillGroup(root, group, node) {
    var items = itemsOf(group);
    if (!items) {
      return;
    }
    var body = node;
    if (node.op === "not") {
      body = node.item || {};
    }
    var multi = andTags(body);
    if (multi) {
      var combined = conditionNode(root);
      if (combined) {
        fillTags(combined, multi.tags, multi.negated, root);
        items.appendChild(combined);
      }
      return;
    }
    if (isConditionOp(body.op)) {
      var single = conditionNode(root);
      if (single) {
        fillCondition(single, body, root);
        items.appendChild(single);
      }
      return;
    }
    if (body.op !== "and" && body.op !== "or") {
      return;
    }
    setGroupOp(group, body.op);
    each(body.items || [], function (child) {
      if (isConditionOp(child.op)) {
        var condition = conditionNode(root);
        if (condition) {
          fillCondition(condition, child, root);
          items.appendChild(condition);
        }
      } else if (isGroupOp(child.op)) {
        var nested = groupNode(root);
        if (nested && groupDepth(group) < MAX_DEPTH) {
          fillGroup(root, nested, child);
          /* Groupe ET réduit à une seule condition superflu, remonter la condition. */
          var only = qs(':scope[data-op="and"] > .builder__items > .builder__condition:only-child', nested);
          items.appendChild(only || nested);
        }
      }
    });
  }

  function hydrate(root) {
    var field = inputOf(root);
    var group = rootGroup(root);
    if (!field || !group || !field.value.trim()) {
      return;
    }
    var parsed;
    try {
      parsed = JSON.parse(field.value);
    } catch (error) {
      return;
    }
    var items = itemsOf(group);
    if (items) {
      while (items.firstChild) {
        items.removeChild(items.firstChild);
      }
    }
    fillGroup(root, group, parsed);
    syncGroup(group);
  }

  function appendNode(node, group) {
    var items = itemsOf(group);
    if (!node || !items) {
      return false;
    }
    items.appendChild(node);
    return true;
  }

  function addCondition(root, group) {
    var condition = conditionNode(root);
    if (!appendNode(condition, group)) {
      return;
    }
    /* `syncGroup` synchronise déjà la condition ajoutée. */
    syncGroup(group);
    serialize(root);
    var trigger = qs(".builder__tagset-summary", condition);
    if (trigger) {
      trigger.focus();
    }
  }

  function addGroup(root, group) {
    if (groupDepth(group) >= MAX_DEPTH) {
      return;
    }
    var nested = groupNode(root);
    if (!appendNode(nested, group)) {
      return;
    }
    var condition = conditionNode(root);
    if (condition) {
      appendNode(condition, nested);
    }
    /* `syncGroup` descend dans le groupe imbriqué et synchronise sa condition. */
    syncGroup(group);
    serialize(root);
  }

  function removeNode(root, element, selector) {
    var node = element.closest(selector);
    if (!node || !node.parentElement) {
      return;
    }
    var group = node.parentElement.closest(".builder__group");
    node.parentElement.removeChild(node);
    if (group) {
      syncGroup(group);
    }
    serialize(root);
  }

  function init(root) {
    if (root.dataset.builderInit) {
      return;
    }
    root.dataset.builderInit = "1";
    hydrate(root);
    var group = rootGroup(root);
    if (group) {
      syncGroup(group);
    }
    syncEmpty(root);
  }

  function initAll(scope) {
    var list = qsa("[data-builder]", scope);
    if (scope instanceof Element && scope.matches("[data-builder]")) {
      list.unshift(scope);
    }
    each(list, init);
  }

  document.addEventListener("click", function (event) {
    var target = event.target;
    if (!(target instanceof Element)) {
      return;
    }
    var root = target.closest("[data-builder]");
    if (!root) {
      return;
    }
    var addConditionButton = target.closest("[data-add-condition]");
    if (addConditionButton) {
      event.preventDefault();
      var owner = addConditionButton.closest(".builder__group");
      if (owner) {
        addCondition(root, owner);
      }
      return;
    }
    var addGroupButton = target.closest("[data-add-group]");
    if (addGroupButton) {
      event.preventDefault();
      var parent = addGroupButton.closest(".builder__group");
      if (parent) {
        addGroup(root, parent);
      }
      return;
    }
    var removeChip = target.closest("[data-remove-chip]");
    if (removeChip) {
      event.preventDefault();
      var chip = removeChip.closest(".builder__chip");
      var chipCondition = chip ? chip.closest(".builder__condition") : null;
      if (chipCondition) {
        removeTagsetChip(chipCondition, chip);
        serialize(root);
      }
      return;
    }
    var removeCondition = target.closest("[data-remove-condition]");
    if (removeCondition) {
      event.preventDefault();
      removeNode(root, removeCondition, ".builder__condition");
      return;
    }
    var removeGroup = target.closest("[data-remove-group]");
    if (removeGroup) {
      event.preventDefault();
      removeNode(root, removeGroup, ".builder__group");
    }
  });

  document.addEventListener("change", function (event) {
    var target = event.target;
    if (!(target instanceof Element)) {
      return;
    }
    var root = target.closest("[data-builder]");
    if (!root) {
      return;
    }
    if (target.matches(".builder__group-op")) {
      var opGroup = target.closest(".builder__group");
      if (opGroup) {
        opGroup.dataset.op = target.value === "or" ? "or" : "and";
      }
    } else if (target.matches(".builder__logic")) {
      var rule = target.closest(".builder__condition");
      var owner = rule && rule.parentElement ? rule.parentElement.closest(".builder__group") : null;
      if (owner) {
        owner.dataset.op = target.value === "or" ? "or" : "and";
      }
    }
    if (target.matches("input[data-tag-option]")) {
      var tagCondition = target.closest(".builder__condition");
      if (tagCondition) {
        if (target.checked) {
          addTagsetChip(tagCondition, target.value, root);
        } else {
          removeTagsetChip(tagCondition, findTagsetChip(tagCondition, target.value));
        }
      }
    }
    var group = target.closest(".builder__group");
    if (group) {
      syncGroup(group);
    }
    serialize(root);
  });

  document.addEventListener("input", function (event) {
    var target = event.target;
    if (!(target instanceof Element)) {
      return;
    }
    var root = target.closest("[data-builder]");
    if (root) {
      serialize(root);
    }
  });

  document.addEventListener("htmx:afterSwap", function (event) {
    initAll(event.target instanceof Element ? event.target : document);
  });

  initAll(document);
})();
