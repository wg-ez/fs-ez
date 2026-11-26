use super::*;

#[test]
fn virtual_folder_actions_live_in_the_context_menu_and_file_table() {
    let files_page = static_source("templates/partials/files_page.html");
    assert!(files_page.contains("data-folder-action=\"create\""));
    assert!(!files_page.contains("data-folder-create"));
    assert!(files_page.contains("data-file-action=\"move\""));
    assert!(files_page.contains("data-file-action=\"move-here\""));
    assert!(files_page.contains("data-file-action=\"move-cancel\""));
    assert!(!files_page.contains("data-folder-rename-open"));
    let move_index = files_page
        .find("data-file-action=\"move\"")
        .expect("bouton déplacer");
    assert!(
        move_index
            < files_page
                .find("data-file-action=\"delete\"")
                .expect("supprimer fichier"),
        "Déplacer précède Supprimer pour un fichier"
    );
    assert!(
        move_index
            < files_page
                .find("data-folder-action=\"delete\"")
                .expect("supprimer dossier"),
        "Déplacer précède Supprimer pour un dossier"
    );

    let files = static_source("templates/partials/files.html");
    assert!(files.contains("partials/folder_rows.html"));
    assert!(files.contains("partials/folder_static_rows.html"));
    assert!(files.contains("data-folder-rows"));
    assert!(files.contains("data-folder-total"));
    assert!(files.contains("data-folder-rows-url"));
    assert!(files.contains("spacer(\"folder\""));
    assert!(!files.contains("data-folder-menu"));
    assert!(!files.contains("class=\"breadcrumb\""));

    let ui = static_source("templates/partials/ui.html");
    assert!(ui.contains("data-folder-nav"));
    assert!(ui.contains("hx-select=\"#files-page\""));
    assert!(ui.contains("table__name-link--folder"));

    let rows = static_source("templates/partials/folder_rows.html");
    assert!(rows.contains("data-folder-row="));
    assert!(!rows.contains("data-folder-delete-url"));
    assert!(!rows.contains("data-folder-delete-confirm"));
    assert!(rows.contains("ui::folder_link("));
    assert!(rows.contains("ui::folder()"));
    assert!(!rows.contains("▸"));
    assert!(!rows.contains("data-remove-tag"));

    let static_rows = static_source("templates/partials/folder_static_rows.html");
    assert!(static_rows.contains("data-folder-create-row"));
    assert!(static_rows.contains("data-folder-create-name"));
    assert!(static_rows.contains("data-folder-up"));
    assert!(static_rows.contains("data-folder-drop=\"{{ parent.id }}\""));
    assert!(static_rows.contains("ui::folder()"));

    let menu = static_source("static/js/app/file-menu.js");
    assert!(menu.contains("folderCreateForm"));
    assert!(menu.contains("[data-folder-action]"));
    assert!(menu.contains("[data-folder-row]"));
    assert!(menu.contains("selectedFolderIds"));
    assert!(menu.contains("/partials/virtual-folders/move-selection"));
    assert!(menu.contains("/partials/virtual-folders/delete-selection"));
    assert!(menu.contains("deleteEntries"));
    assert!(menu.contains("moveSelectionBody"));
    assert!(menu.contains("new window.FormData()"));
    assert!(menu.contains("navigateFolder"));
    assert!(menu.contains("addEventListener(\"dblclick\""));
    assert!(menu.contains("event.detail === 0"));
    assert!(menu.contains("function navRows"));
    assert!(menu.contains("openActiveRow"));
    assert!(menu.contains("renameActiveRow"));
    assert!(menu.contains("folderAction === \"delete\""));
    assert!(menu.contains("deleteSelection()"));

    let folders = static_source("static/js/app/folders.js");
    assert!(folders.contains("data-folder-create-name"));
    assert!(folders.contains("htmx:beforeHistorySave"));
    assert!(!folders.contains("folderDeleteId"));
    assert!(!folders.contains("app.folderDelete ="));
    assert!(folders.contains("/partials/virtual-folders/move-selection"));
    assert!(!folders.contains("data-folder-rename-open"));
    assert!(!folders.contains("window.confirm"));
}

#[test]
fn move_mode_survives_folder_navigation_and_history_restore() {
    let js = static_source("static/js/app/file-menu.js");

    let clear = between(&js, "function clearFileSelection", "function fileMenuIds");
    assert!(
        clear.contains("if (moveMode)"),
        "la sélection reste en mode déplacement"
    );

    let nav = between(
        &js,
        "function navigateFolder",
        "document.addEventListener(\"mousedown\"",
    );
    assert!(
        !nav.contains("cancelMoveMode"),
        "naviguer ne quitte pas le mode déplacement"
    );

    let restore = between(&js, "function restoreSelection", "app.lastSelectRow = null");
    assert!(
        restore.contains("!prune || moveMode"),
        "pas de purge pendant le déplacement"
    );

    let folder_restore = between(
        &js,
        "function restoreFolderSelection",
        "function restoreSelection",
    );
    assert!(
        folder_restore.contains("if (!prune || moveMode)"),
        "les ids dossier absents du DOM restent sélectionnés au scroll"
    );
    assert!(
        folder_restore.contains("folderRowsInfo"),
        "la fenêtre virtuelle empêche une purge à tort"
    );

    let history = between(
        &js,
        "htmx:historyRestore",
        "function restoreFolderSelection",
    );
    assert!(
        !history.contains("cancelMoveMode"),
        "le retour navigateur conserve le mode déplacement"
    );

    assert!(js.contains("Mode déplacement : simple clic inerte"));
    assert!(js.contains("cancelMoveMode();\n        clearFileSelection();"));
}

#[test]
fn file_menu_shift_range_spans_virtual_windows() {
    let js = static_source("static/js/app/file-menu.js");
    assert!(js.contains("function selectRange"));
    assert!(js.contains("filesRowsInfo"));
    assert!(js.contains("fetchRangeEntries"));
    assert!(js.contains("function restoreSelection"));
    let scroll = static_source("static/js/app/scroll.js");
    assert!(scroll.contains("function fileRowIndex"));
    assert!(scroll.contains("filesRowsInfo"));
}

#[test]
fn file_menu_shift_range_spans_folder_windows() {
    let js = static_source("static/js/app/file-menu.js");
    assert!(js.contains("function fetchFolderRangeIds"));
    assert!(js.contains("function requestFolderIndexRange"));
    assert!(js.contains("folderRangeGeneration"));
    assert!(js.contains("folderRowsInfo"));

    let range = between(&js, "function selectRange", "var fileMenuContext");
    assert!(range.contains("folderFrom"));
    assert!(range.contains("requestFolderIndexRange"));
    assert!(range.contains("requestIndexRange"));
    assert!(
        range
            .find("requestFolderIndexRange")
            .expect("plage dossier")
            < range.find("requestIndexRange").expect("plage fichier"),
        "la partie dossier précède la partie fichier"
    );
}

#[test]
fn file_menu_ctrl_a_selects_all_folders_beyond_rendered_rows() {
    let js = static_source("static/js/app/file-menu.js");
    assert!(js.contains("function fetchAllFolderIds"));
    assert!(js.contains("fetchFolderRangeIds(info, 0, info.total - 1"));
    assert!(js.contains("var selectAllFolders = false"));
    assert!(js.contains("folderSelectAllPromise"));
    assert!(js.contains("function folderSelectionReady"));
    assert!(js.contains("app.folderSelectionReady = folderSelectionReady"));
    assert!(js.contains("folderExcludedMap"));

    let all = between(&js, "function selectAllFiles", "function selectedFileIds");
    assert!(all.contains("folderRowsInfo"));
    assert!(all.contains("fetchAllFolderIds(folderInfo)"));
    assert!(
        all.find("setFolderRowSelected").expect("lignes dossier")
            < all.find("fetchAllFolderIds").expect("ids hors DOM"),
        "les lignes rendues sont marquées avant le chargement complet"
    );

    let delete_selection = between(
        &js,
        "function deleteSelection",
        "document.addEventListener(\"contextmenu\"",
    );
    assert!(delete_selection.contains("folderSelectionReady()"));
}

#[test]
fn file_list_keyboard_shortcuts_select_all_and_delete() {
    let events = static_source("static/js/app/events.js");
    assert!(events.contains("event.key === \"Delete\""));
    assert!(events.contains("event.key === \"Enter\""));
    assert!(events.contains("event.key === \"F2\""));
    assert!(events.contains("app.selectAllFiles()"));
    assert!(events.contains("app.deleteSelection()"));
    assert!(events.contains("app.openActiveRow()"));
    assert!(events.contains("app.renameActiveRow()"));
    assert!(events.contains("target.id === \"work\""));

    let js = static_source("static/js/app/file-menu.js");
    let delete_selection = between(
        &js,
        "function deleteSelection",
        "document.addEventListener(\"contextmenu\"",
    );
    assert!(delete_selection.contains("data-file-kind"));
    assert!(delete_selection.contains("selectedFolderIds()"));
    assert!(delete_selection.contains("deleteEntries(ids, folders)"));
    assert!(js.contains("function selectAllFiles"));
    assert!(js.contains("[data-folder-up], .table__row[data-folder-row]"));
    assert!(js.contains("[data-folder-row].is-selected"));
}

/* Scénarios clavier vérifiés : Ctrl+A fichiers+dossiers, Delete mixte, j/k sur `..`/dossiers/fichiers,
Espace sur la ligne active, Entrée ouvre un dossier, F2 renomme fichier ou dossier, mode déplacement inerte. */
#[test]
fn virtual_folder_keyboard_navigation_covers_folders_and_up_row() {
    let js = static_source("static/js/app/file-menu.js");
    assert!(js.contains("function navRows"));
    assert!(js.contains(".table__row[data-folder-up], .table__row[data-folder-row]"));
    assert!(js.contains("isUpRow(row)"));
    assert!(js.contains("isFolderNavRow(row)"));
    assert!(js.contains("moveMode"));
    assert!(js.contains("app.folderOpenRename(folderId)"));
    assert!(js.contains("navigateFolder(link)"));
    assert!(js.contains("each(qsa(\".table__row[data-folder-row]\")"));
}
