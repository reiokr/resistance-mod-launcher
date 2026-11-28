<script>
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { getVersion } from "@tauri-apps/api/app";
  import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
  import { translate } from "./i18n.js";
  import SimpleMode from "./SimpleMode.svelte";

  let snapshot = $state(null);
  let loading = $state(true);
  let busy = $state(false);
  let busyText = $state("");
  let error = $state("");
  let message = $state("");
  let launchIssue = $state(null);
  let query = $state("");
  let stateFilter = $state("all");
  let categoryFilter = $state("all");
  let tagFilter = $state("all");
  let tab = $state("mods");
  const DETAILS_DEFAULT_WIDTH = 400;
  function initialDetailsWidth() {
    const saved = Number(localStorage.getItem("resistance-details-width"));
    return Number.isFinite(saved) && saved >= 260
      ? saved
      : DETAILS_DEFAULT_WIDTH;
  }
  let detailsWidth = $state(initialDetailsWidth());
  let detailsResizing = $state(false);
  function initialLanguage() {
    const saved = localStorage.getItem("resistance-lang");
    if (saved === "et" || saved === "en") return saved;
    const locale =
      typeof navigator !== "undefined" ? (navigator.language || "").toLowerCase() : "";
    return locale.startsWith("et") ? "et" : "en";
  }
  function initialUiMode() {
    const saved = localStorage.getItem("resistance-ui-mode");
    return saved === "simple" || saved === "advanced" ? saved : "advanced";
  }
  const initialMode = initialUiMode();
  let uiMode = $state(initialMode);
  let simpleUsed = $state(
    localStorage.getItem("resistance-simple-used") === "1" ||
      initialMode === "simple"
  );
  let simpleHintDismissed = $state(
    localStorage.getItem("resistance-simple-hint-dismissed") === "1"
  );
  function setUiMode(mode) {
    if (mode !== "simple" && mode !== "advanced") return;
    uiMode = mode;
    localStorage.setItem("resistance-ui-mode", mode);
    if (mode === "simple") {
      simpleUsed = true;
      localStorage.setItem("resistance-simple-used", "1");
    }
  }
  function dismissSimpleHint() {
    simpleHintDismissed = true;
    localStorage.setItem("resistance-simple-hint-dismissed", "1");
  }
  let lang = $state(initialLanguage());
  let diagnosticsSignature = $state(null);
  let diagnosticsScanToken = 0;
  function activeSignature(ids) {
    return ids.join("\u0000");
  }
  function diagnosticsFresh() {
    return (
      !!snapshot?.diagnostics &&
      diagnosticsSignature === activeSignature(activeOrder)
    );
  }
  const t = (text, vars) => translate(lang, text, vars);
  function setLanguage(value) {
    if (value !== "et" && value !== "en") return;
    lang = value;
    localStorage.setItem("resistance-lang", value);
    document.documentElement.lang = value;
  }
  let selectedId = $state(null);
  let details = $state(null);
  let activeOrder = $state([]);
  let savedActiveIds = $state([]);
  let profileCovers = $state({});
  let dragId = $state(null);
  let newCategory = $state("");
  let editingCategory = $state(null);
  let editingName = $state("");
  let profileName = $state("");
  let currentProfile = $state(null);
  let profileBaseline = $state(null);
  const profileDirty = $derived(
    !!currentProfile &&
      !!profileBaseline &&
      (activeOrder.length !== profileBaseline.length ||
        activeOrder.some((id, index) => id !== profileBaseline[index]))
  );
  let profileModalOpen = $state(false);
  let profileMode = $state("collection");
  let profileDraftName = $state("");
  let profileDraftDescription = $state("");
  let collectionInput = $state("");
  let collectionPreview = $state(null);
  let collectionLoading = $state(false);
  let collectionError = $state("");
  let collectionSelected = $state(new Set());
  let collectionIncludePending = $state(true);
  let editingProfile = $state(null);
  let historyOpen = $state(false);
  let historyProfileName = $state(null);
  let historyEntries = $state([]);
  let historyLoading = $state(false);
  let historyError = $state("");
  let historySelected = $state(null);
  let historyRevision = $state(null);
  let historyRevisionLoading = $state(false);
  let historyRestoring = $state(false);
  let profileCollectionOpen = $state(false);
  let profileCollectionName = $state("");
  let profileCollectionPreview = $state(null);
  let profileCollectionActive = $state(new Set());
  let profileCollectionIds = $state([]);
  let profileCollectionDeactivatedIds = $state(new Set());
  let profileCollectionSelected = $state(new Set());
  let profileCollectionLoading = $state(false);
  let profileCollectionSaving = $state(false);
  let profileCollectionError = $state("");
  let importOpen = $state(false);
  let importPath = $state("");
  let importPreview = $state(null);
  let importName = $state("");
  let importReplace = $state(false);
  let importLoading = $state(false);
  let importApplying = $state(false);
  let importError = $state("");
  let deleteOpen = $state(false);
  let deleteTarget = $state(null);
  let deletePreview = $state(null);
  let deleteLoading = $state(false);
  let deleteApplying = $state(false);
  let deleteError = $state("");
  let cleanupReport = $state(null);
  let cleanupLoading = $state(false);
  let cleanupError = $state("");
  let trashEntries = $state(null);
  let trashLoading = $state(false);
  let trashError = $state("");
  let configOpen = $state(false);
  let configMod = $state(null);
  let configFiles = $state([]);
  let configPath = $state("");
  let configContent = $state("");
  let configOriginal = $state("");
  let configMeta = $state(null);
  let configLoading = $state(false);
  let configSaving = $state(false);
  let configError = $state("");
  let configMessage = $state("");
  let configTextarea = $state(null);
  const configDirty = $derived(configOpen && configContent !== configOriginal);
  let autoOverwrite = $state(false);
  let autoPreview = $state(null);
  let readmeOpen = $state(false);
  let readmeMod = $state(null);
  let readme = $state(null);
  let readmeLoading = $state(false);
  let readmeError = $state("");
  let aboutOpen = $state(false);
  let appVersion = $state("0.3.3");

  onMount(() => {
    document.documentElement.lang = lang;
    detailsWidth = clampDetailsWidth(detailsWidth);
    const onWindowResize = () => {
      detailsWidth = clampDetailsWidth(detailsWidth);
    };
    window.addEventListener("resize", onWindowResize);
    getVersion().then((version) => (appVersion = version)).catch(() => {});
    load();
    return () => window.removeEventListener("resize", onWindowResize);
  });

  async function load() {
    diagnosticsScanToken += 1;
    diagnosticsSignature = null;
    const firstLoad = !snapshot;
    if (firstLoad) {
      loading = true;
    } else {
      busy = true;
    }
    busyText = firstLoad ? t("Laen modi nimekirja…") : t("Laen uuesti…");
    error = "";
    message = "";
    try {
      snapshot = await invoke("get_state");
      activeOrder = [...snapshot.active_ids];
      savedActiveIds = [...snapshot.active_ids];
      await refreshProfileCovers();
      const remembered = localStorage.getItem("resistance-current-profile");
      const rememberedIds = localStorage.getItem("resistance-current-profile-ids");
      const rememberedValid =
        !!remembered &&
        rememberedIds === JSON.stringify(activeOrder) &&
        snapshot.profiles.some((profile) => profile.name === remembered);
      if (rememberedValid) {
        currentProfile = remembered;
      } else {
        currentProfile = null;
        profileBaseline = null;
        if (remembered || rememberedIds) rememberSavedProfile(null, []);
      }
      selectedId = null;
      details = null;
      autoPreview = null;
    } catch (err) {
      error = String(err);
    } finally {
      loading = false;
      busy = false;
      busyText = "";
    }
  }

  const gameConfigDirty = $derived(
    activeOrder.length !== savedActiveIds.length ||
      activeOrder.some((id, index) => id !== savedActiveIds[index])
  );
  const showSimpleHint = $derived(
    uiMode === "advanced" &&
      tab === "profiles" &&
      !!snapshot &&
      !simpleHintDismissed &&
      !simpleUsed &&
      !gameConfigDirty &&
      snapshot.profiles.some((profile) => profile.count > 0)
  );
  const modsById = $derived(new Map((snapshot?.mods ?? []).map((mod) => [mod.id, mod])));
  const historyProfile = $derived(
    snapshot?.profiles.find((profile) => profile.name === historyProfileName) ?? null
  );
  const historyCurrentIds = $derived(
    historyProfile
      ? effectiveIds(historyProfile.active_ids, historyProfile.deactivated_mod_ids)
      : []
  );
  const historyRevisionIds = $derived(
    historyRevision
      ? effectiveIds(
          historyRevision.active_ids,
          historyRevision.deactivated_mod_ids
        )
      : []
  );
  const historyDiff = $derived(
    diffLists(historyRevisionIds, historyCurrentIds)
  );
  const editingDiff = $derived(
    profileBaseline
      ? diffLists(profileBaseline, activeOrder)
      : { added: [], removed: [], reordered: [] }
  );
  const activeMods = $derived(
    activeOrder.map((id) => modsById.get(id)).filter(Boolean)
  );
  const selected = $derived(selectedId ? modsById.get(selectedId) : null);
  const profileCollectionNew = $derived.by(() => {
    if (!profileCollectionPreview) return [];
    return profileCollectionPreview.items.filter(
      (item) =>
        item.installed &&
        item.mod_id &&
        !profileCollectionActive.has(item.mod_id) &&
        !profileCollectionDeactivatedIds.has(item.mod_id)
    );
  });
  const profileCollectionDeactivated = $derived.by(() => {
    if (!profileCollectionPreview) return [];
    return profileCollectionPreview.items.filter(
      (item) =>
        item.installed &&
        item.mod_id &&
        profileCollectionDeactivatedIds.has(item.mod_id)
    );
  });
  const profileCollectionActiveCount = $derived(
    profileCollectionIds.filter((id) => !profileCollectionDeactivatedIds.has(id)).length
  );
  const importNameConflict = $derived(
    !!importName.trim() &&
      (snapshot?.profiles ?? []).some((profile) => profile.name === importName.trim())
  );
  const categories = $derived(
    (snapshot?.categories ?? []).map((category) => category.name)
  );

  const categoryCounts = $derived.by(() => {
    const counts = new Map();
    for (const mod of snapshot?.mods ?? []) {
      counts.set(mod.category, (counts.get(mod.category) || 0) + 1);
    }
    return counts;
  });

  const workshopTags = $derived.by(() => {
    const groups = new Map();
    for (const mod of snapshot?.mods ?? []) {
      const seen = new Set();
      for (const raw of mod.workshop_tags ?? []) {
        const tag = raw.trim();
        if (!tag) continue;
        const key = tag.toLowerCase();
        if (seen.has(key)) continue;
        seen.add(key);
        let group = groups.get(key);
        if (!group) {
          group = { variants: new Map(), count: 0 };
          groups.set(key, group);
        }
        group.count += 1;
        group.variants.set(tag, (group.variants.get(tag) || 0) + 1);
      }
    }
    return [...groups.values()]
      .filter((group) => group.count >= 2)
      .map((group) => {
        const [name] = [...group.variants.entries()].sort(
          (a, b) => b[1] - a[1] || a[0].localeCompare(b[0])
        )[0];
        return { name, count: group.count };
      })
      .sort((a, b) => b.count - a.count || a.name.localeCompare(b.name));
  });

  const conflictModIds = $derived(
    new Set(
      (snapshot?.diagnostics?.conflicts ?? []).flatMap((conflict) =>
        conflict.overrides.map((override) => override.mod_id)
      )
    )
  );

  const dependencyModIds = $derived(
    new Set(
      (snapshot?.diagnostics?.missing_dependencies ?? []).map((issue) => issue.mod_id)
    )
  );

  const diagnosticsIssueCount = $derived(
    (snapshot?.summary?.conflicts ?? 0) + (snapshot?.summary?.missing_dependencies ?? 0)
  );

  const manualCategoryCount = $derived(
    (snapshot?.mods ?? []).filter((mod) => mod.manual_category).length
  );

  const filteredMods = $derived.by(() => {
    const needle = query.trim().toLowerCase();
    return (snapshot?.mods ?? []).filter((mod) => {
      if (stateFilter === "active" && !mod.active) return false;
      if (stateFilter === "inactive" && mod.active) return false;
      if (stateFilter === "missing" && (!mod.missing || !mod.active)) return false;
      if (stateFilter === "updates" && !mod.update_available) return false;
      if (categoryFilter !== "all" && mod.category !== categoryFilter) return false;
      if (tagFilter !== "all" && !(mod.workshop_tags ?? []).some(
        (tag) => tag.toLowerCase() === tagFilter.toLowerCase()
      )) return false;
      if (!needle) return true;
      return [mod.name, mod.author, mod.id, mod.category, mod.workshop_id ?? ""]
        .concat(mod.workshop_tags ?? [])
        .join(" ")
        .toLowerCase()
        .includes(needle);
    });
  });

  function syncOrderFields() {
    if (!snapshot) return;
    diagnosticsSignature = null;
    for (const mod of snapshot.mods) {
      const index = activeOrder.indexOf(mod.id);
      mod.active = index >= 0;
      mod.order = index >= 0 ? index : null;
    }
    snapshot.summary.active = activeOrder.length;
    snapshot.summary.missing = snapshot.mods.filter(
      (mod) => mod.active && mod.missing
    ).length;
  }

  let deactivationQueue = Promise.resolve();

  function recordProfileDeactivation(modId, deactivated) {
    if (!currentProfile || !modId) return deactivationQueue;
    const current = snapshot?.profiles.find(
      (entry) => entry.name === currentProfile
    );
    if (!current?.source_id) return deactivationQueue;
    const profileName = currentProfile;
    deactivationQueue = deactivationQueue.then(async () => {
      try {
        const ids = await invoke("set_profile_mod_deactivated", {
          name: profileName,
          mod_id: modId,
          deactivated,
        });
        if (snapshot) {
          const profile = snapshot.profiles.find(
            (entry) => entry.name === profileName
          );
          if (profile) profile.deactivated_mod_ids = ids;
        }
        if (deactivated && profileBaseline) {
          profileBaseline = profileBaseline.filter((id) => id !== modId);
        }
      } catch (err) {
        error = String(err);
      }
    });
    return deactivationQueue;
  }

  async function toggle(mod) {
    const wasActive = mod.active;
    if (mod.active) {
      mod.active = false;
      activeOrder = activeOrder.filter((id) => id !== mod.id);
    } else {
      activeOrder = [...activeOrder, mod.id];
    }
    syncOrderFields();
    await recordProfileDeactivation(mod.id, wasActive);
  }

  function moveBy(id, delta) {
    const from = activeOrder.indexOf(id);
    const to = from + delta;
    if (from < 0 || to < 0 || to >= activeOrder.length) return;
    const next = [...activeOrder];
    const [item] = next.splice(from, 1);
    next.splice(to, 0, item);
    activeOrder = next;
    syncOrderFields();
  }

  function moveTo(fromId, toId) {
    const from = activeOrder.indexOf(fromId);
    const to = activeOrder.indexOf(toId);
    if (from < 0 || to < 0 || from === to) return;
    const next = [...activeOrder];
    const [item] = next.splice(from, 1);
    next.splice(to, 0, item);
    activeOrder = next;
    syncOrderFields();
  }

  function dragStart(event, id) {
    dragId = id;
    event.dataTransfer.effectAllowed = "move";
  }

  function dragOver(event) {
    event.preventDefault();
    event.dataTransfer.dropEffect = "move";
  }

  function drop(event, id) {
    event.preventDefault();
    if (dragId && dragId !== id) moveTo(dragId, id);
    dragId = null;
  }

  async function save() {
    busy = true;
    busyText = t("Salvestan XComModOptions.ini…");
    error = "";
    message = "";
    try {
      const result = await invoke("save_state", { active_ids: activeOrder });
      savedActiveIds = [...activeOrder];
      rememberSavedProfile(currentProfile, [...activeOrder]);
      if (snapshot) {
        snapshot.summary.active = result.active_count;
        for (const mod of snapshot.mods) mod.active = activeOrder.includes(mod.id);
      }
      message = result.unchanged
        ? t("Muudatusi ei olnud — XComModOptions.ini on juba sama.")
        : t("Salvestatud {count} aktiivset modi. Backup: {backup}", { count: result.active_count, backup: result.backup ?? "—" });
      if (result.warnings?.length) {
        message += " " + t("Hoiatus: {warnings}", { warnings: result.warnings.join(" · ") });
      }
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
      busyText = "";
    }
  }

  async function launch() {
    launchIssue = null;
    try {
      const status = await invoke("runtime_status");
      if (status.workshop_path_warning) {
        setWorkshopWarning(status.workshop_path_warning);
        return;
      }
    } catch {
      // If the check itself fails, let the normal launch path report the error.
    }
    if (!(await ensureSteam())) return;
    if (gameConfigDirty) {
      const ok = window.confirm(
        t("Aktiivne nimekiri on veel mängu configusse salvestamata. Kas salvestada enne käivitamist?\n\nOK = salvesta ja käivita\nCancel = katkesta")
      );
      if (!ok) return;
      await save();
      if (error) return;
    }
    busy = true;
    busyText = t("Käivitan XCOM 2…");
    error = "";
    message = "";
    try {
      const result = await invoke("launch_game");
      let suffix = "";
      if (result.prelaunch) suffix += ` · prelaunch: ${result.prelaunch}`;
      if (result.prelaunch_ok === false) suffix += " " + t("(prelaunch hoiatas)");
      message = t("XCOM 2 käivitatud (PID {pid}): {exe}{suffix}", { pid: result.pid, exe: result.exe, suffix });
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
      busyText = "";
    }
  }

  async function select(mod) {
    if (selectedId === mod.id) {
      selectedId = null;
      details = null;
      return;
    }
    selectedId = mod.id;
    details = null;
    try {
      const result = await invoke("get_mod_details", { id: mod.id });
      if (selectedId === mod.id) details = result;
    } catch (err) {
      if (selectedId === mod.id) error = String(err);
    }
  }

  async function refreshWorkshop() {
    busy = true;
    const workshopCount = (snapshot?.mods ?? []).filter((mod) => mod.workshop_id).length;
    busyText = t("Värskendan Workshopi andmeid ({count} modi)…", {
      count: workshopCount,
    });
    error = "";
    message = "";
    try {
      const result = await invoke("refresh_workshop");
      await load();
      message = t("Workshop: {fetched}/{total} uuendatud", { fetched: result.fetched, total: result.total });
      if (result.failed > 0) {
        message += t(" · {failed} ebaõnnestus", { failed: result.failed });
        if (result.errors?.length) message += ` (${result.errors[0]})`;
      }
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
      busyText = "";
    }
  }

  async function setCategory(mod, category) {
    busy = true;
    busyText = t("Muudan kategooriat…");
    try {
      await invoke("set_mod_category", { mod_id: mod.id, category });
      mod.category = category;
      mod.manual_category = true;
      autoPreview = null;
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
      busyText = "";
    }
  }

  async function createCategory() {
    const name = newCategory.trim();
    if (!name) return;
    busy = true;
    busyText = t("Lisan kategooria…");
    try {
      await invoke("create_category", { name });
      newCategory = "";
      await load();
      tab = "categories";
      message = t("Kategooria lisatud: {name}", { name });
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
      busyText = "";
    }
  }

  async function renameCategory(oldName) {
    const name = editingName.trim();
    if (!name) return;
    busy = true;
    busyText = t("Nimetan kategooriat ümber…");
    try {
      await invoke("rename_category", { old: oldName, new: name });
      editingCategory = null;
      editingName = "";
      await load();
      tab = "categories";
      message = t("Kategooria ümber nimetatud: {name}", { name });
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
      busyText = "";
    }
  }

  async function deleteCategory(name) {
    if (!window.confirm(t("Kustuta kategooria \"{name}\"? Modid liiguvad \"Unsorted\" alla.", { name }))) return;
    busy = true;
    busyText = t("Kustutan kategooriat…");
    try {
      await invoke("delete_category", { name });
      await load();
      tab = "categories";
      message = t("Kategooria kustutatud: {name}", { name });
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
      busyText = "";
    }
  }

  async function previewAutoCategories() {
    busy = true;
    busyText = t("Arvutan automaatseid kategooriaid…");
    error = "";
    try {
      autoPreview = await invoke("preview_auto_categories", {
        overwrite: autoOverwrite,
      });
    } catch (err) {
      error = String(err);
      autoPreview = null;
    } finally {
      busy = false;
      busyText = "";
    }
  }

  async function applyAutoCategories() {
    if (!autoPreview) return;
    busy = true;
    busyText = t("Rakendan automaatseid kategooriaid…");
    error = "";
    try {
      const result = await invoke("apply_auto_categories", {
        overwrite: autoOverwrite,
      });
      autoPreview = null;
      await load();
      tab = "categories";
      message = t("Kategooriad rakendatud: {assigned} modi, {created} uut kategooriat.{manual}{backup}", {
        assigned: result.assigned,
        created: result.categories_created,
        manual: result.manual_pinned
          ? t(" {count} käsitsi määratud modi jäi paigale.", { count: result.manual_pinned })
          : "",
        backup: result.backup ? t(" Varundus: {path}", { path: result.backup }) : "",
      });
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
      busyText = "";
    }
  }

  async function clearCategoryLearning() {
    if (
      !window.confirm(
t("Unustada {count} käsitsi määratud kategoorianäidet? Kategooriad ise jäävad alles.", { count: manualCategoryCount })
      )
    ) {
      return;
    }
    busy = true;
    busyText = t("Kustutan õpitud näiteid…");
    error = "";
    try {
      const count = await invoke("clear_category_feedback");
      autoPreview = null;
      await load();
      tab = "categories";
      message = t("Õpitud näited kustutatud ({count}). Kategooriad ise jäid alles.", { count });
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
      busyText = "";
    }
  }

  async function clearManualCategory(mod) {
    busy = true;
    busyText = t("Eemaldan käsitsi määratud kategooriat õppimisest…");
    error = "";
    try {
      const removed = await invoke("clear_mod_category_feedback", {
        mod_id: mod.id,
      });
      mod.manual_category = false;
      autoPreview = null;
      message = removed
        ? t("“{name}” kategooria ei ole enam õppimiseks kinnitatud.", { name: mod.name })
        : t("“{name}” ei olnudki käsitsi kinnitatud.", { name: mod.name });
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
      busyText = "";
    }
  }

  function openProfileModal() {
    profileModalOpen = true;
    profileMode = "collection";
    profileDraftName = "";
    profileDraftDescription = "";
    collectionInput = "";
    collectionPreview = null;
    collectionLoading = false;
    collectionError = "";
    collectionSelected = new Set();
    collectionIncludePending = true;
  }

  function closeProfileModal() {
    if (collectionLoading || busy) return;
    profileModalOpen = false;
  }

  function parseCollectionId(value) {
    const text = value.trim();
    const match = text.match(/(?:id=|\/details\/|\/)(\d{5,})/);
    return match ? match[1] : text;
  }

  function stripBbcode(value) {
    return value
      .replace(/\[\/?[^\]]+\]/g, "")
      .replace(/\r/g, "")
      .trim();
  }

  async function loadCollectionPreview() {
    const id = parseCollectionId(collectionInput);
    if (!/^\d+$/.test(id)) {
      collectionError = t("Sisesta korrektne Steam Workshopi kollektsiooni URL või ID.");
      return;
    }
    collectionLoading = true;
    collectionError = "";
    collectionPreview = null;
    try {
      const preview = await invoke("preview_steam_collection", {
        collection_id: Number(id),
      });
      collectionPreview = preview;
      collectionSelected = new Set(
        preview.items
          .filter((item) => item.installed)
          .map((item) => item.workshop_id)
      );
      if (!profileDraftName.trim()) profileDraftName = preview.title;
      if (!profileDraftDescription.trim()) {
        profileDraftDescription = stripBbcode(preview.description);
      }
    } catch (err) {
      collectionError = String(err);
    } finally {
      collectionLoading = false;
    }
  }

  function toggleCollectionItem(workshopId) {
    const next = new Set(collectionSelected);
    if (next.has(workshopId)) next.delete(workshopId);
    else next.add(workshopId);
    collectionSelected = next;
  }

  function selectAllInstalled() {
    if (!collectionPreview) return;
    collectionSelected = new Set(
      collectionPreview.items
        .filter((item) => item.installed)
        .map((item) => item.workshop_id)
    );
  }

  function clearCollectionSelection() {
    collectionSelected = new Set();
  }

  async function createProfileFromModal() {
    if (busy) return;
    if (!profileDraftName.trim()) {
      collectionError = t("Profiili nimi on kohustuslik.");
      return;
    }
    error = "";
    collectionError = "";
    if (profileMode === "current") {
      const name = profileDraftName.trim();
      if (snapshot?.profiles.some((profile) => profile.name === name)) {
        collectionError = t("Selle nimega profiil on juba olemas. Vali teine nimi.");
        return;
      }
      busy = true;
      busyText = t("Salvestan praegust nimekirja profiilina…");
      const currentIds = [...activeOrder];
      try {
        await invoke("create_empty_profile", {
          name,
          description: profileDraftDescription.trim() || null,
        });
        await invoke("save_profile", {
          name,
          active_ids: currentIds,
        });
        profileModalOpen = false;
        await load();
        activeOrder = currentIds;
        currentProfile = name;
        profileBaseline = [...currentIds];
        syncOrderFields();
        tab = "profiles";
        message = t("Profiil loodud praegusest nimekirjast: {name}", { name });
      } catch (err) {
        collectionError = String(err);
      } finally {
        busy = false;
        busyText = "";
      }
      return;
    }
    if (profileMode === "collection") {
      if (!collectionPreview) {
        collectionError = t("Laadi enne kollektsioon.");
        return;
      }
      const selected = collectionPreview.items.filter(
        (item) => item.installed && collectionSelected.has(item.workshop_id)
      );
      const rank = new Map(activeOrder.map((id, index) => [id, index]));
      selected.sort((a, b) => {
        const aRank = a.mod_id != null ? rank.get(a.mod_id) : undefined;
        const bRank = b.mod_id != null ? rank.get(b.mod_id) : undefined;
        if (aRank != null && bRank != null) return aRank - bRank;
        if (aRank != null) return -1;
        if (bRank != null) return 1;
        return (a.title || "").localeCompare(b.title || "");
      });
      busy = true;
      busyText = t("Loon profiili kollektsioonist…");
      const currentIds = [...activeOrder];
      const previousProfile = currentProfile;
      const previousBaseline = profileBaseline ? [...profileBaseline] : null;
      try {
        const name = await invoke("create_profile_from_collection", {
          name: profileDraftName.trim(),
          description: profileDraftDescription.trim() || null,
          collection_id: collectionPreview.id,
          selected_mod_ids: selected.map((item) => item.mod_id).filter(Boolean),
          include_pending: collectionIncludePending,
        });
        profileModalOpen = false;
        await load();
        activeOrder = currentIds;
        currentProfile = previousProfile;
        profileBaseline = previousBaseline;
        syncOrderFields();
        tab = "profiles";
        message = t("Profiil loodud: {name} ({count} modi)", {
          name,
          count: selected.length,
        });
      } catch (err) {
        collectionError = String(err);
      } finally {
        busy = false;
        busyText = "";
      }
      return;
    }

    busy = true;
    busyText = t("Loon profiili…");
    const currentIds = [...activeOrder];
    const previousProfile = currentProfile;
    const previousBaseline = profileBaseline ? [...profileBaseline] : null;
    try {
      const name = await invoke("create_empty_profile", {
        name: profileDraftName.trim(),
        description: profileDraftDescription.trim() || null,
      });
      profileModalOpen = false;
      await load();
      activeOrder = currentIds;
      currentProfile = previousProfile;
      profileBaseline = previousBaseline;
      syncOrderFields();
      tab = "profiles";
      message = t("Profiil loodud: {name}", { name });
    } catch (err) {
      collectionError = String(err);
    } finally {
      busy = false;
      busyText = "";
    }
  }

  async function openProfileCollection(profile) {
    profileCollectionOpen = true;
    profileCollectionName = profile.name;
    profileCollectionPreview = null;
    profileCollectionActive = new Set();
    profileCollectionSelected = new Set();
    profileCollectionLoading = true;
    profileCollectionError = "";
    try {
      const [ids, preview] = await Promise.all([
        invoke("get_profile", { name: profile.name }),
        invoke("profile_collection_preview", { name: profile.name }),
      ]);
      const profileSet = new Set(ids);
      const profileView = snapshot?.profiles.find((entry) => entry.name === profile.name);
      profileCollectionIds = ids;
      profileCollectionActive = profileSet;
      profileCollectionDeactivatedIds = new Set(profileView?.deactivated_mod_ids ?? []);
      profileCollectionPreview = preview;
      profileCollectionSelected = new Set(
        preview.items
          .filter(
            (item) => item.installed && item.mod_id && !profileSet.has(item.mod_id)
          )
          .map((item) => item.workshop_id)
      );
    } catch (err) {
      profileCollectionError = String(err);
    } finally {
      profileCollectionLoading = false;
    }
  }

  function closeProfileCollection() {
    if (profileCollectionSaving) return;
    profileCollectionOpen = false;
    profileCollectionDeactivatedIds = new Set();
  }

  function rememberSavedProfile(name, ids) {
    if (name) {
      localStorage.setItem("resistance-current-profile", name);
      localStorage.setItem("resistance-current-profile-ids", JSON.stringify(ids));
    } else {
      localStorage.removeItem("resistance-current-profile");
      localStorage.removeItem("resistance-current-profile-ids");
    }
  }

  function safeFileName(name) {
    return (
      String(name || "profile")
        .replace(/[\\/:*?"<>|]/g, "_")
        .replace(/\s+/g, " ")
        .trim()
        .slice(0, 120) || "profile"
    );
  }

  function fileBaseName(path) {
    return String(path).split(/[\\/]/).pop() || String(path);
  }

  async function exportProfile(profile) {
    error = "";
    message = "";
    try {
      const path = await saveDialog({
        defaultPath: `${safeFileName(profile.name)}.x2profile.json`,
        filters: [
          {
            name: t("Resistance Mod Launcheri profiil"),
            extensions: ["json"],
          },
        ],
      });
      if (!path) return;
      busy = true;
      busyText = t("Ekspordin profiili…");
      await invoke("export_profile", { name: profile.name, path });
      message = t("Profiil eksporditud: {path}", { path });
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
      busyText = "";
    }
  }

  async function exportAllProfiles() {
    error = "";
    message = "";
    try {
      const directory = await openDialog({
        directory: true,
        multiple: false,
        title: t("Vali kaust profiilide eksportimiseks"),
      });
      if (!directory || Array.isArray(directory)) return;
      busy = true;
      busyText = t("Ekspordin kõiki profiile…");
      const result = await invoke("export_all_profiles", { directory });
      message = t("Eksporditud {count} profiili: {path}", {
        count: result.count,
        path: result.directory,
      });
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
      busyText = "";
    }
  }

  async function chooseProfileImport() {
    error = "";
    message = "";
    try {
      const selection = await openDialog({
        multiple: false,
        directory: false,
        filters: [
          {
            name: t("Resistance Mod Launcheri profiil"),
            extensions: ["json"],
          },
        ],
      });
      const path = Array.isArray(selection) ? selection[0] : selection;
      if (!path) return;
      await previewProfileImport(path);
    } catch (err) {
      error = String(err);
    }
  }

  async function previewProfileImport(path) {
    importOpen = true;
    importPath = path;
    importPreview = null;
    importName = "";
    importReplace = false;
    importLoading = true;
    importApplying = false;
    importError = "";
    try {
      const preview = await invoke("preview_profile_import", { path });
      importPreview = preview;
      importName = preview.profile_name;
      importReplace = false;
    } catch (err) {
      importError = String(err);
    } finally {
      importLoading = false;
    }
  }

  function closeProfileImport() {
    if (importApplying) return;
    importOpen = false;
    importPath = "";
    importPreview = null;
    importName = "";
    importReplace = false;
    importLoading = false;
    importError = "";
  }

  async function applyImportedProfile() {
    if (!importPreview || !importName.trim() || importApplying) return;
    if (importNameConflict && !importReplace) {
      importError = t(
        "Selle nimega profiil on juba olemas. Vali teine nimi või märgi „Asenda olemasolev”."
      );
      return;
    }
    importApplying = true;
    importError = "";
    try {
      const result = await invoke("apply_profile_import", {
        path: importPath,
        name_override: importName.trim(),
        replace_existing: importReplace,
      });
      importOpen = false;
      await load();
      tab = "profiles";
      message = result.replaced
        ? t("Profiil asendatud: {name} ({count} modi)", {
            name: result.name,
            count: result.active_count,
          })
        : t("Profiil imporditud: {name} ({count} modi)", {
            name: result.name,
            count: result.active_count,
          });
      if (result.missing_count > 0) {
        message +=
          " " +
          t("{count} modi puudub selles masinas — vaata profiili „Ootab” märki.", {
            count: result.missing_count,
          });
      }
    } catch (err) {
      importError = String(err);
    } finally {
      importApplying = false;
    }
  }

  function toggleProfileCollectionItem(workshopId) {
    const next = new Set(profileCollectionSelected);
    if (next.has(workshopId)) next.delete(workshopId);
    else next.add(workshopId);
    profileCollectionSelected = next;
  }

  function selectAllProfileCollection() {
    if (!profileCollectionPreview) return;
    profileCollectionSelected = new Set(
      profileCollectionNew.map((item) => item.workshop_id)
    );
  }

  async function restoreCollectionMod(id) {
    if (!id || busy) return;
    busy = true;
    busyText = t("Taastan modi…");
    try {
      const ids = await invoke("restore_profile_mod", {
        name: profileCollectionName,
        mod_id: id,
      });
      profileCollectionIds = ids;
      profileCollectionActive = new Set(ids);
      const remaining = new Set(profileCollectionDeactivatedIds);
      remaining.delete(id);
      profileCollectionDeactivatedIds = remaining;
      if (snapshot) {
        const profile = snapshot.profiles.find(
          (entry) => entry.name === profileCollectionName
        );
        if (profile) {
          profile.count = ids.length;
          profile.deactivated_mod_ids = profile.deactivated_mod_ids.filter(
            (value) => value !== id
          );
        }
      }
      if (profileCollectionName === currentProfile) {
        if (!activeOrder.includes(id)) {
          const profileIndex = ids.indexOf(id);
          let insertAt = activeOrder.length;
          for (let index = profileIndex + 1; index < ids.length; index += 1) {
            const activeIndex = activeOrder.indexOf(ids[index]);
            if (activeIndex >= 0) {
              insertAt = activeIndex;
              break;
            }
          }
          const next = [...activeOrder];
          next.splice(insertAt, 0, id);
          activeOrder = next;
          syncOrderFields();
        }
        profileBaseline = ids.filter((value) => !remaining.has(value));
        message = t(
          "Mod „{name}” lülitati sisse. Vajuta Salvesta, et muudatus mängu configusse läheks.",
          { name: modsById.get(id)?.name || id }
        );
      } else {
        message = t(
          "Mod „{name}” taastati profiilis. Rakenda profiil, et see aktiivseks teha.",
          { name: modsById.get(id)?.name || id }
        );
      }
    } catch (err) {
      profileCollectionError = String(err);
    } finally {
      busy = false;
      busyText = "";
    }
  }

  async function addProfileCollectionMods() {
    if (!profileCollectionPreview || profileCollectionSaving) return;
    const selected = profileCollectionNew.filter((item) =>
      profileCollectionSelected.has(item.workshop_id)
    );
    if (selected.length === 0) return;
    profileCollectionSaving = true;
    profileCollectionError = "";
    try {
      const added = await invoke("add_collection_mods_to_profile", {
        name: profileCollectionName,
        selected_mod_ids: selected.map((item) => item.mod_id).filter(Boolean),
      });
      profileCollectionOpen = false;
      const currentIds = [...activeOrder];
      const previousProfile = currentProfile;
      const previousBaseline = profileBaseline ? [...profileBaseline] : null;
      await load();
      activeOrder = currentIds;
      currentProfile = previousProfile;
      profileBaseline = previousBaseline;
      syncOrderFields();
      tab = "profiles";
      message = t("Lisatud {count} modi profiili „{name}”.", {
        count: added,
        name: profileCollectionName,
      });
    } catch (err) {
      profileCollectionError = String(err);
    } finally {
      profileCollectionSaving = false;
    }
  }

  async function saveCurrentProfile() {
    if (!currentProfile || busy) return;
    const name = currentProfile;
    const savedIds = [...activeOrder];
    busy = true;
    busyText = t("Salvestan profiili…");
    try {
      await invoke("save_profile", { name, active_ids: savedIds });
      profileBaseline = [...savedIds];
      if (snapshot) {
        const profile = snapshot.profiles.find((entry) => entry.name === name);
        if (profile) {
          profile.count = savedIds.length;
          profile.active_ids = [...savedIds];
          profile.updated = Math.floor(Date.now() / 1000);
          try {
            const history = await invoke("profile_history", { name });
            profile.revision_count = history.length;
          } catch {
            // The profile itself was saved; the history badge can refresh later.
          }
        }
      }
      editingProfile = null;
      message = t("Profiil salvestatud: {name}", { name });
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
      busyText = "";
    }
  }

  async function saveProfile() {
    const name = profileName.trim();
    if (!name) return;
    busy = true;
    busyText = t("Salvestan profiili…");
    try {
      const savedIds = [...activeOrder];
      await invoke("save_profile", { name, active_ids: savedIds });
      profileName = "";
      await load();
      activeOrder = savedIds;
      syncOrderFields();
      currentProfile = name;
      profileBaseline = [...savedIds];
      tab = "profiles";
      message = t("Profiil salvestatud: {name}", { name });
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
      busyText = "";
    }
  }

  async function selectProfile(event) {
    const name = event.target.value;
    if (!name) {
      currentProfile = null;
      profileBaseline = null;
      rememberSavedProfile(null, []);
      return;
    }
    if (name === currentProfile) return;
    if (profileDirty) {
      const ok = window.confirm(
        t("Profiilis on salvestamata muudatusi. Vaheta profiili ja kaota need?")
      );
      if (!ok) {
        event.target.value = currentProfile ?? "";
        return;
      }
    }
    await applyProfile(name, true);
  }

  async function applyProfile(name, skipConfirm = false) {
    editingProfile = null;
    launchIssue = null;
    if (
      !skipConfirm &&
      currentProfile &&
      profileDirty &&
      name !== currentProfile &&
      !window.confirm(
        t("Profiilis on salvestamata muudatusi. Vaheta profiili ja kaota need?")
      )
    ) {
      return;
    }
    busy = true;
    busyText = t("Rakendan profiili…");
    try {
      const profileView = snapshot?.profiles.find((entry) => entry.name === name);
      const deactivated = new Set(profileView?.deactivated_mod_ids ?? []);
      const ids = (
        await invoke("get_profile", { name })
      ).filter((id) => !deactivated.has(id));
      activeOrder = [...ids];
      currentProfile = name;
      profileBaseline = [...ids];
      syncOrderFields();
      tab = "order";
      let scanNote = "";
      const scanIds = [...activeOrder];
      const signature = activeSignature(scanIds);
      const token = ++diagnosticsScanToken;
      try {
        busyText = t("Kontrollin profiili konflikte…");
        const report = await invoke("scan_diagnostics", {
          active_ids: scanIds,
        });
        if (
          token === diagnosticsScanToken &&
          signature === activeSignature(activeOrder)
        ) {
          snapshot.diagnostics = report;
          diagnosticsSignature = signature;
          snapshot.summary.conflicts = report.conflicts.length;
          snapshot.summary.missing_dependencies = report.missing_dependency_links;
          scanNote = " " + t(
            "Kontroll: {conflicts} konflikti, {missing} puudulikku sõltuvust.",
            {
              conflicts: report.conflicts.length,
              missing: report.missing_dependency_count,
            }
          );
        }
      } catch {
        if (token === diagnosticsScanToken) {
          scanNote = " " + t("Konfliktikontroll ebaõnnestus.");
        }
      }
      message = scanNote.trim();
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
      busyText = "";
    }
  }

  async function applyProfileForSimple(name) {
    if (busy) return;
    busy = true;
    busyText = t("Rakendan profiili…");
    error = "";
    message = "";
    launchIssue = null;
    try {
      const profileView = snapshot?.profiles.find(
        (entry) => entry.name === name
      );
      const deactivated = new Set(profileView?.deactivated_mod_ids ?? []);
      const ids = (
        await invoke("get_profile", { name })
      ).filter((id) => !deactivated.has(id));
      activeOrder = [...ids];
      currentProfile = name;
      profileBaseline = [...ids];
      syncOrderFields();
      tab = "order";
      diagnosticsScanToken += 1;
      diagnosticsSignature = null;
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
      busyText = "";
    }
  }

  async function openProfileHistory(profile) {
    historyOpen = true;
    historyProfileName = profile.name;
    historyEntries = [];
    historySelected = null;
    historyRevision = null;
    historyError = "";
    historyLoading = true;
    try {
      historyEntries = await invoke("profile_history", { name: profile.name });
      if (historyEntries.length > 0) {
        await selectHistoryRevision(historyEntries[0].id);
      }
    } catch (err) {
      historyError = String(err);
    } finally {
      historyLoading = false;
    }
  }

  async function selectHistoryRevision(revisionId) {
    historySelected = revisionId;
    historyRevision = null;
    historyError = "";
    historyRevisionLoading = true;
    try {
      historyRevision = await invoke("get_profile_revision", {
        name: historyProfileName,
        revision_id: revisionId,
      });
    } catch (err) {
      historyError = String(err);
    } finally {
      historyRevisionLoading = false;
    }
  }

  function closeProfileHistory() {
    if (historyRestoring) return;
    historyOpen = false;
    historyProfileName = null;
    historyEntries = [];
    historySelected = null;
    historyRevision = null;
    historyError = "";
    historyLoading = false;
    historyRevisionLoading = false;
  }

  async function restoreHistoryRevision() {
    if (!historyProfile || !historyRevision || historyRestoring) return;
    const name = historyProfile.name;
    const unsaved =
      currentProfile === name && profileDirty
        ? "\n\n" + t("Sul on salvestamata muudatusi. Need kaotatakse.")
        : "";
    if (
      !window.confirm(
        t("Taastada see versioon? Praegune seis salvestatakse ajalukku.") +
          unsaved
      )
    ) {
      return;
    }
    const revisionId = historyRevision.id;
    historyRestoring = true;
    busy = true;
    busyText = t("Taastan versiooni…");
    error = "";
    try {
      await invoke("restore_profile_revision", {
        name,
        revision_id: revisionId,
      });
      historyOpen = false;
      await load();
      if (error) return;
      if (currentProfile === name) {
        await applyProfileForSimple(name);
      }
      message = t("Versioon taastatud: {name}", { name });
    } catch (err) {
      historyError = String(err);
    } finally {
      historyRestoring = false;
      busy = false;
      busyText = "";
    }
  }

  async function duplicateProfile(profile) {
    if (busy) return;
    if (
      currentProfile === profile.name &&
      profileDirty &&
      !window.confirm(
        t("Dubleeritakse viimati salvestatud seis. Profiilis on salvestamata muudatusi. Jätkata?")
      )
    ) {
      return;
    }
    const name = window.prompt(
      t("Uue profiili nimi:"),
      `${profile.name} (test)`
    );
    if (!name || !name.trim()) return;

    busy = true;
    busyText = t("Dubleerin profiili…");
    error = "";
    message = "";
    try {
      const created = await invoke("duplicate_profile", {
        source: profile.name,
        name: name.trim(),
      });
      await load();
      message = t("Profiil loodud: {name}", { name: created });
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
      busyText = "";
    }
  }

  async function syncProfileToParent(profile) {
    if (!profile.parent || busy) return;
    const parent = snapshot?.profiles.find(
      (entry) => entry.name === profile.parent
    );
    if (!parent) {
      error = t("Originaalprofiili ei leitud: {name}", {
        name: profile.parent,
      });
      return;
    }

    busy = true;
    busyText = t("Kontrollin muudatusi…");
    error = "";
    try {
      const [childIds, parentIds] = await Promise.all([
        invoke("get_profile", { name: profile.name }),
        invoke("get_profile", { name: profile.parent }),
      ]);
      const childEffective = effectiveIds(
        childIds,
        profile.deactivated_mod_ids
      );
      const parentEffective = effectiveIds(
        parentIds,
        parent.deactivated_mod_ids
      );
      const diff = diffLists(parentEffective, childEffective);
      const changed =
        profile.parent_updated != null &&
        parent.updated !== profile.parent_updated;
      let text = t(
        "Uuendada profiili „{parent}” selle testkoopiaga?\n\n+{added} lisatud, −{removed} eemaldatud.",
        {
          parent: profile.parent,
          added: diff.added.length,
          removed: diff.removed.length,
        }
      );
      if (changed) {
        text +=
          "\n\n" +
          t(
            "Originaali on vahepeal muudetud. Sünkroonimine asendab selle nimekirja; vana seis jääb ajalukku."
          );
      }
      if (
        currentProfile === profile.parent &&
        profileDirty
      ) {
        text +=
          "\n\n" +
          t("Originaalprofiilis on salvestamata muudatusi. Need kaotatakse.");
      }
      if (!window.confirm(text)) return;

      busyText = t("Uuendan originaali…");
      const result = await invoke("sync_profile_to_parent", {
        name: profile.name,
      });
      await load();
      message = t("Profiil „{name}” uuendatud: {count} modi.", {
        name: result.parent,
        count: result.active_count,
      });
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
      busyText = "";
    }
  }

  function askDeleteProfile(profile) {
    deleteOpen = true;
    deleteTarget = profile;
    deletePreview = null;
    deleteLoading = true;
    deleteApplying = false;
    deleteError = "";
    invoke("preview_profile_delete", { name: profile.name })
      .then((preview) => {
        deletePreview = preview;
      })
      .catch((err) => {
        deleteError = String(err);
      })
      .finally(() => {
        deleteLoading = false;
      });
  }

  function closeDeleteProfile() {
    if (deleteApplying) return;
    deleteOpen = false;
    deleteTarget = null;
    deletePreview = null;
    deleteLoading = false;
    deleteError = "";
  }

  async function confirmDeleteProfile() {
    if (!deleteTarget || deleteApplying) return;
    const name = deleteTarget.name;
    deleteApplying = true;
    deleteError = "";
    try {
      await invoke("delete_profile", { name });
      deleteOpen = false;
      deleteTarget = null;
      deletePreview = null;
      await load();
      tab = "profiles";
      message = t(
        "Profiil kustutatud: {name}. Modisid ei kustutatud.",
        { name }
      );
      await scanCleanup(true);
    } catch (err) {
      deleteError = String(err);
    } finally {
      deleteApplying = false;
    }
  }

  async function removeUnusedMod(mod) {
    if (
      !window.confirm(
        t("Eemalda mod „{name}” kettalt? See viiakse prügikasti. Kui Steamis on tellimus alles, võib Steam selle uuesti alla laadida.", {
          name: mod.name,
        })
      )
    ) {
      return;
    }
    cleanupLoading = true;
    cleanupError = "";
    try {
      const result = await invoke("remove_unused_mod", {
        workshop_id: mod.workshop_id,
        active_ids: activeOrder,
      });
      message = t("Mod „{name}” viidi prügikasti ({size}).", {
        name: result.name,
        size: formatSize(result.size),
      });
      cleanupLoading = false;
      await scanCleanup();
      await loadTrash();
    } catch (err) {
      cleanupError = String(err);
      cleanupLoading = false;
    }
  }

  async function scanCleanup(scrollIntoView = false) {
    if (cleanupLoading) return;
    cleanupLoading = true;
    cleanupError = "";
    try {
      cleanupReport = await invoke("scan_unused_mods", {
        active_ids: activeOrder,
      });
      if (scrollIntoView) {
        await tick();
        document
          .getElementById("cleanup-panel")
          ?.scrollIntoView({ behavior: "smooth", block: "start" });
      }
    } catch (err) {
      cleanupError = String(err);
    } finally {
      cleanupLoading = false;
    }
  }

  async function loadTrash() {
    trashLoading = true;
    trashError = "";
    try {
      trashEntries = await invoke("list_mod_trash");
    } catch (err) {
      trashError = String(err);
    } finally {
      trashLoading = false;
    }
  }

  async function restoreTrashEntry(entry) {
    if (
      !window.confirm(
        t("Taastada mod „{name}” algupärasesse kausta?", { name: entry.name })
      )
    ) {
      return;
    }
    trashLoading = true;
    trashError = "";
    try {
      const result = await invoke("restore_mod_trash", { id: entry.id });
      message = t("Mod „{name}” taastati: {path}", {
        name: result.name,
        path: result.original_path,
      });
      trashEntries = await invoke("list_mod_trash");
      await scanCleanup();
    } catch (err) {
      const text = String(err);
      trashError = text.includes("original folder already exists")
        ? t("Algne kaust on juba olemas. Eemalda see enne taastamist.")
        : text;
    } finally {
      trashLoading = false;
    }
  }

  async function refreshProfileCovers() {
    try {
      const entries = await invoke("list_profile_covers");
      profileCovers = Object.fromEntries(
        entries.map((entry) => [entry.name, entry.data_url])
      );
    } catch {
      profileCovers = {};
    }
  }

  async function pickProfileCover(profile) {
    try {
      const path = await openDialog({
        multiple: false,
        directory: false,
        filters: [
          {
            name: t("Pilt"),
            extensions: ["png", "jpg", "jpeg", "webp"],
          },
        ],
      });
      if (!path || Array.isArray(path)) return;
      busy = true;
      busyText = t("Lisan profiilipilti…");
      error = "";
      message = "";
      const cover = await invoke("set_profile_cover", {
        name: profile.name,
        path,
      });
      profileCovers = { ...profileCovers, [profile.name]: cover.data_url };
      message = t("Profiilipilt lisatud: {name}", { name: profile.name });
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
      busyText = "";
    }
  }

  async function clearProfileCover(profile) {
    if (!window.confirm(t("Eemalda profiili „{name}” pilt?", { name: profile.name }))) {
      return;
    }
    busy = true;
    busyText = t("Eemaldan profiilipilti…");
    error = "";
    message = "";
    try {
      await invoke("clear_profile_cover", { name: profile.name });
      const next = { ...profileCovers };
      delete next[profile.name];
      profileCovers = next;
      message = t("Profiilipilt eemaldatud: {name}", { name: profile.name });
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
      busyText = "";
    }
  }

  async function editProfile(profile) {
    if (busy) return;
    if (
      currentProfile &&
      currentProfile !== profile.name &&
      profileDirty &&
      !window.confirm(
        t("Profiilis „{name}” on salvestamata muudatused. Kas jätkata ja kaotada need?", {
          name: currentProfile,
        })
      )
    ) {
      return;
    }
    if (currentProfile !== profile.name) {
      await applyProfileForSimple(profile.name);
    }
    editingProfile = profile.name;
    tab = "mods";
    stateFilter = "all";
    categoryFilter = "all";
    tagFilter = "all";
    query = "";
    selectedId = null;
    details = null;
  }

  async function cancelEditingProfile() {
    if (!editingProfile || busy) return;
    const name = editingProfile;
    if (
      profileDirty &&
      !window.confirm(
        t("Tühista profiili „{name}” salvestamata muudatused?", { name })
      )
    ) {
      return;
    }
    editingProfile = null;
    if (currentProfile === name && profileDirty) {
      await applyProfileForSimple(name);
    }
    message = t("Muutmine lõpetatud.");
  }

  function setWorkshopWarning(warning) {
    launchIssue = warning;
    let text = t(
      "Mängu ei käivitatud: XComEngine.ini Workshopi tee ei klapi launcheriga."
    );
    if (warning.fix_command) {
      text += " " + t("Parandus:") + " " + warning.fix_command;
    }
    error = text;
    message = "";
  }

  async function ensureSteam(status = null) {
    if (!status) {
      try {
        status = await invoke("runtime_status");
      } catch {
        return true;
      }
    }
    if (status.steam_running) return true;

    busy = true;
    busyText = t("Käivitan Steami…");
    error = "";
    try {
      const started = await invoke("start_steam");
      if (!started) {
        error = t("Steami käivitamine ebaõnnestus. Käivita Steam käsitsi.");
        return false;
      }
      if (snapshot) snapshot.summary.steam_running = true;
      return true;
    } catch (err) {
      error = t("Steami käivitamine ebaõnnestus: {error}", {
        error: String(err),
      });
      return false;
    } finally {
      busy = false;
      busyText = "";
    }
  }

  async function startCurrentProfile() {
    if (!currentProfile || busy) return;
    error = "";
    message = "";
    launchIssue = null;
    busy = true;
    busyText = t("Kontrollin olekut…");
    let status = null;
    try {
      status = await invoke("runtime_status");
      snapshot.summary.steam_running = status.steam_running;
      snapshot.summary.aml_running = status.aml_running;
      snapshot.summary.game_running = status.game_running;
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
      busyText = "";
    }
    if (!status) return;
    if (status.aml_running) {
      error = t("AML on lahti. Sulge AML enne käivitamist.");
      return;
    }
    if (status.game_running) {
      error = t("XCOM 2 töötab juba.");
      return;
    }
    if (status.workshop_path_warning) {
      setWorkshopWarning(status.workshop_path_warning);
      return;
    }
    if (!(await ensureSteam(status))) return;
    if (snapshot.summary.missing > 0) {
      const ok = window.confirm(
        t("Selles profiilis on {count} puuduvat aktiivset modi. Kas käivitada ikkagi?", {
          count: snapshot.summary.missing,
        })
      );
      if (!ok) return;
    }
    await save();
    if (error) return;
    await launch();
  }

  function openManageProfiles() {
    setUiMode("advanced");
    tab = "profiles";
  }

  async function openChecksFromSimple() {
    setUiMode("advanced");
    await tick();
    await showStat("dependencies");
  }

  async function runDiagnostics() {
    if (busy) return;
    const scanIds = [...activeOrder];
    const signature = activeSignature(scanIds);
    const token = ++diagnosticsScanToken;
    busy = true;
    busyText = t("Kontrollin konflikte ja sõltuvusi ({count} aktiivset modi)…", { count: scanIds.length });
    error = "";
    message = "";
    try {
      const report = await invoke("scan_diagnostics", { active_ids: scanIds });
      if (
        token !== diagnosticsScanToken ||
        signature !== activeSignature(activeOrder)
      ) {
        return;
      }
      snapshot.diagnostics = report;
      diagnosticsSignature = signature;
      snapshot.summary.conflicts = report.conflicts.length;
      snapshot.summary.missing_dependencies = report.missing_dependency_links;
      tab = "diagnostics";
      message = t("Kontroll valmis: {missing} modi puudulike sõltuvustega, {conflicts} konflikti, {duplicates} duplikaatrühma.", { missing: report.missing_dependency_count, conflicts: report.conflicts.length, duplicates: report.duplicate_groups.length });
    } catch (err) {
      if (token === diagnosticsScanToken) {
        error = String(err);
      }
    } finally {
      if (token === diagnosticsScanToken) {
        busy = false;
        busyText = "";
      }
    }
  }

  async function enableDependency(issue, dependency) {
    if (!dependency?.mod_id) return;
    const mod = modsById.get(dependency.mod_id);
    if (!mod) return;
    if (!mod.active) {
      const current = [...activeOrder];
      const dependentIndex = current.indexOf(issue.mod_id);
      if (dependentIndex >= 0) current.splice(dependentIndex, 0, mod.id);
      else current.push(mod.id);
      activeOrder = current;
      syncOrderFields();
    }
    await recordProfileDeactivation(mod.id, false);
    await runDiagnostics();
    message =
      t("Sõltuvus \"{name}\" lülitati sisse. Vajuta Salvesta, et muudatus mängu configusse läheks.", {
        name: mod.name,
      }) + (message ? " " + message : "");
  }

  async function deactivateConflictMod(override) {
    const mod = modsById.get(override.mod_id);
    if (!mod || !mod.active) return;
    if (
      !window.confirm(
        t("Lülitada mod \"{name}\" välja ja kontrollida konfliktid uuesti?", {
          name: mod.name,
        })
      )
    ) {
      return;
    }
    activeOrder = activeOrder.filter((id) => id !== mod.id);
    mod.active = false;
    mod.order = null;
    syncOrderFields();
    await recordProfileDeactivation(mod.id, true);
    await runDiagnostics();
    message =
      t("Mod \"{name}\" lülitati välja. Vajuta Salvesta, et muudatus mängu configusse läheks.", {
        name: mod.name,
      }) + (message ? " " + message : "");
  }

  async function showStat(view) {
    error = "";
    message = "";
    if (view === "active") {
      tab = "mods";
      stateFilter = "active";
    } else if (view === "all") {
      tab = "mods";
      stateFilter = "all";
      categoryFilter = "all";
      tagFilter = "all";
      query = "";
    } else if (view === "missing") {
      tab = "mods";
      stateFilter = "missing";
    } else if (view === "updates") {
      tab = "mods";
      stateFilter = "updates";
    } else if (view === "categories") {
      tab = "categories";
    } else if (view === "unknown") {
      const count = snapshot.summary.unknown_active;
      if (count > 0) {
        const knownCount = snapshot.diagnostics?.unknown_active_ids?.length ?? 0;
        if (!snapshot.diagnostics || knownCount !== count) {
          await runDiagnostics();
        } else {
          tab = "diagnostics";
        }
        await tick();
        document
          .getElementById("diag-unknown")
          ?.scrollIntoView({ behavior: "smooth", block: "start" });
      } else {
        tab = "mods";
        stateFilter = "all";
        message = t("Tundmatuid aktiivseid modisid ei ole.");
      }
    } else if (view === "conflicts" || view === "dependencies") {
      if (busy) {
        message = t("Kontroll käib juba, oota…");
        return;
      }
      if (!diagnosticsFresh()) await runDiagnostics();
      tab = "diagnostics";
      await tick();
      document
        .getElementById(view === "conflicts" ? "diag-conflicts" : "diag-missing")
        ?.scrollIntoView({ behavior: "smooth", block: "start" });
    }
  }

  function modDependencyIssue(id) {
    return (snapshot?.diagnostics?.missing_dependencies ?? []).find(
      (issue) => issue.mod_id === id
    );
  }

  function modConflicts(id) {
    return (snapshot?.diagnostics?.conflicts ?? []).filter((conflict) =>
      conflict.overrides.some((override) => override.mod_id === id)
    );
  }

  async function openConfigEditor(mod) {
    configMod = mod;
    configOpen = true;
    configFiles = [];
    configPath = "";
    configContent = "";
    configOriginal = "";
    configMeta = null;
    configError = "";
    configMessage = "";
    configLoading = true;
    try {
      configFiles = await invoke("list_mod_config_files", { mod_id: mod.id });
      if (configFiles.length) {
        await loadConfigFile(configFiles[0].path, true);
      }
    } catch (err) {
      configError = String(err);
    } finally {
      configLoading = false;
    }
  }

  async function loadConfigFile(path, force = false) {
    if (!configMod) return;
    if (!force && configDirty) {
      if (!window.confirm(t("Sulge salvestamata muudatused?"))) return;
    }
    configLoading = true;
    configError = "";
    configMessage = "";
    try {
      const file = await invoke("read_mod_config", {
        mod_id: configMod.id,
        path,
      });
      configPath = file.path;
      configContent = file.content;
      configOriginal = file.content;
      configMeta = file;
    } catch (err) {
      configError = String(err);
    } finally {
      configLoading = false;
    }
  }

  async function saveConfigFile() {
    if (!configMod || !configPath) return;
    configSaving = true;
    configError = "";
    configMessage = "";
    try {
      const result = await invoke("save_mod_config", {
        mod_id: configMod.id,
        path: configPath,
        content: configContent,
      });
      configOriginal = configContent;
      if (result.changed) {
        configMessage = result.backup
          ? t("Salvestatud. Varundus: {path}", { path: result.backup })
          : t("Salvestatud.");
        configFiles = await invoke("list_mod_config_files", {
          mod_id: configMod.id,
        });
      } else {
        configMessage = t("Muudatusi ei olnud; fail jäi puutumata.");
      }
    } catch (err) {
      configError = String(err);
    } finally {
      configSaving = false;
    }
  }

  function closeConfigEditor() {
    if (configDirty && !window.confirm(t("Sulge salvestamata muudatustega?"))) return;
    configOpen = false;
    configMod = null;
    configFiles = [];
    configPath = "";
    configContent = "";
    configOriginal = "";
    configMeta = null;
    configError = "";
    configMessage = "";
  }

  async function openReadme(mod) {
    readmeMod = mod;
    readmeOpen = true;
    readme = null;
    readmeError = "";
    readmeLoading = true;
    try {
      readme = await invoke("get_mod_readme", { mod_id: mod.id });
    } catch (err) {
      readmeError = String(err);
    } finally {
      readmeLoading = false;
    }
  }

  function closeReadme() {
    readmeOpen = false;
    readmeMod = null;
    readme = null;
    readmeError = "";
  }

  function closeProfileMenus(event) {
    for (const details of document.querySelectorAll("details.profile-menu[open]")) {
      if (!details.contains(event.target)) details.removeAttribute("open");
    }
  }

  function handleWindowKeydown(event) {
    if (configOpen) {
      if (event.key === "Escape") {
        event.preventDefault();
        closeConfigEditor();
      } else if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "s") {
        event.preventDefault();
        if (configDirty && !configSaving && !configLoading) saveConfigFile();
      }
      return;
    }
    if (readmeOpen && event.key === "Escape") {
      event.preventDefault();
      closeReadme();
    } else if (aboutOpen && event.key === "Escape") {
      event.preventDefault();
      aboutOpen = false;
    }
  }

  async function handleEditorKeydown(event) {
    if (event.key !== "Tab") return;
    event.preventDefault();
    const element = event.currentTarget;
    const start = element.selectionStart;
    const end = element.selectionEnd;
    configContent =
      configContent.slice(0, start) + "  " + configContent.slice(end);
    await tick();
    element.selectionStart = element.selectionEnd = start + 2;
  }

  function formatUnix(seconds) {
    if (!seconds) return "—";
    return new Date(seconds * 1000).toLocaleString();
  }

  function effectiveIds(ids, deactivated) {
    const disabled = new Set(deactivated ?? []);
    return (ids ?? []).filter((id) => !disabled.has(id));
  }

  function diffLists(before, after) {
    const beforeSet = new Set(before ?? []);
    const afterSet = new Set(after ?? []);
    const added = (after ?? []).filter((id) => !beforeSet.has(id));
    const removed = (before ?? []).filter((id) => !afterSet.has(id));
    const commonBefore = (before ?? []).filter((id) => afterSet.has(id));
    const commonAfter = (after ?? []).filter((id) => beforeSet.has(id));
    const reordered = commonAfter.filter(
      (id, index) => commonBefore[index] !== id
    );
    return { added, removed, reordered };
  }

  function modLabel(id) {
    return modsById.get(id)?.name || id;
  }

  function clampDetailsWidth(value) {
    const max = Math.max(320, Math.min(1200, window.innerWidth - 480));
    return Math.min(max, Math.max(260, Math.round(value)));
  }

  function persistDetailsWidth() {
    localStorage.setItem("resistance-details-width", String(detailsWidth));
  }

  function startDetailsResize(event) {
    event.preventDefault();
    const startX = event.clientX;
    const startWidth = detailsWidth;
    detailsResizing = true;
    document.body.style.userSelect = "none";
    document.body.style.cursor = "col-resize";

    const onMove = (moveEvent) => {
      detailsWidth = clampDetailsWidth(
        startWidth + (startX - moveEvent.clientX)
      );
    };
    const finish = () => {
      detailsResizing = false;
      document.body.style.userSelect = "";
      document.body.style.cursor = "";
      persistDetailsWidth();
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", finish);
      window.removeEventListener("pointercancel", finish);
      window.removeEventListener("blur", finish);
    };

    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", finish);
    window.addEventListener("pointercancel", finish);
    window.addEventListener("blur", finish);
  }

  function handleDetailsResizeKey(event) {
    const step = event.shiftKey ? 60 : 20;
    if (event.key === "ArrowLeft") {
      detailsWidth = clampDetailsWidth(detailsWidth + step);
    } else if (event.key === "ArrowRight") {
      detailsWidth = clampDetailsWidth(detailsWidth - step);
    } else if (event.key === "Home") {
      detailsWidth = clampDetailsWidth(DETAILS_DEFAULT_WIDTH);
    } else {
      return;
    }
    event.preventDefault();
    persistDetailsWidth();
  }

  function resetDetailsWidth() {
    detailsWidth = clampDetailsWidth(DETAILS_DEFAULT_WIDTH);
    persistDetailsWidth();
  }

  function formatSize(bytes) {
    if (bytes === null || bytes === undefined || bytes <= 0) return "—";
    const units = ["B", "KB", "MB", "GB"];
    let value = bytes;
    let unit = 0;
    while (value >= 1024 && unit < units.length - 1) {
      value /= 1024;
      unit++;
    }
    return `${value.toFixed(unit === 0 ? 0 : 1)} ${units[unit]}`;
  }
</script>

<svelte:window onkeydown={handleWindowKeydown} onclick={closeProfileMenus} />

<div class="shell">
  <header class="topbar">
    <div class="brand">
      <div class="logo">R</div>
      <div>
        <h1>{t("Resistance Mod Launcher")}</h1>
        <p>
          {t("Unofficial XCOM 2: WotC · Linux / Proton")}
          {#if snapshot}
            <span class="dot">·</span>
            {t("{count} modi", { count: snapshot.summary.total })}
            <span class="dot">·</span>
            {t("{count} aktiivset", { count: snapshot.summary.active })}
          {:else}
            <span class="dot">·</span>
            {t("laen…")}
          {/if}
        </p>
      </div>
    </div>

    <div class="top-actions">
      {#if uiMode === "simple"}
        <button
          class="ghost mode-toggle"
          onclick={() => setUiMode("advanced")}
          title={t("Ava täisvaade")}
        >{t("Täisvaade")}</button>
      {/if}
      {#if uiMode === "advanced"}
      <div class="profile-picker">
        <select
          value={currentProfile ?? ""}
          onchange={selectProfile}
          disabled={loading || busy || !snapshot}
          title={t("Profiil / modpack")}
        >
          <option value="">{t("Muu / salvestamata")}</option>
          {#each snapshot?.profiles ?? [] as profile (profile.name)}
            <option value={profile.name}>
              {profile.name}{profile.pending_count > 0 ? ` · ${t("{count} ootab", { count: profile.pending_count })}` : ""}
            </option>
          {/each}
        </select>
        {#if profileDirty}
          <span
            class="profile-dirty"
            title={t("Muudatused ei ole profiili salvestatud")}
          >●</span>
        {/if}
      </div>
      {#if currentProfile}
        <button
          class="ghost"
          onclick={saveCurrentProfile}
          disabled={loading || busy}
          title={t("Salvesta praegune nimekiri selle profiili sisse")}
        >{t("Salvesta profiil")}</button>
      {/if}
      {/if}
      <select
        class="lang-select"
        value={lang}
        onchange={(event) => setLanguage(event.currentTarget.value)}
        title={t("Keel")}
        aria-label={t("Keel")}
      >
        <option value="en">English</option>
        <option value="et">Eesti</option>
      </select>
      {#if uiMode === "advanced"}
        <button class="ghost" onclick={load} disabled={loading || busy}>{t("Laadi uuesti")}</button>
        <button class="ghost" onclick={refreshWorkshop} disabled={loading || busy}>
          {t("Värskenda Workshopi")}
        </button>
        <button
          class="ghost mode-toggle simple-switch"
          onclick={() => setUiMode("simple")}
          disabled={!snapshot}
          title={t("Ava lihtvaade: ainult profiilid ja Start")}
        >{t("Lihtvaade")}</button>
        <button
          class="primary config-save"
          class:dirty={gameConfigDirty}
          onclick={save}
          disabled={loading || busy || !snapshot}
          title={t("Kirjuta aktiivne nimekiri XComModOptions.ini faili")}
        >
          {t("Salvesta config")}
          {#if gameConfigDirty}
            <span class="save-dot"></span>
          {/if}
        </button>
        <button class="launch" onclick={launch} disabled={loading || busy || !snapshot}>{t("Käivita mäng")}</button>
      {:else}
        <button class="ghost" onclick={() => (aboutOpen = true)}>{t("Info")}</button>
      {/if}
    </div>
  </header>

  {#if busy && !loading}
    <div class="activity">
      <div class="activity-track"><div class="activity-bar"></div></div>
      <span>{busyText || t("Töötlen…")}</span>
    </div>
  {/if}

  {#if uiMode === "advanced" && snapshot && gameConfigDirty}
    <div class="config-banner">
      <div class="config-banner-text">
        <strong>
          {#if currentProfile}
            {t("Profiil „{name}” on rakendatud.", { name: currentProfile })}
          {:else}
            {t("Aktiivne nimekiri on muudetud.")}
          {/if}
        </strong>
        <span>
          {t("Mängu config on veel vana. Salvesta see enne käivitamist.")}
          {#if diagnosticsFresh()}
            {" · "}
            {t("Kontroll: {conflicts} konflikti, {missing} puudulikku sõltuvust.", {
              conflicts: snapshot.diagnostics.conflicts.length,
              missing: snapshot.diagnostics.missing_dependency_count,
            })}
          {/if}
        </span>
      </div>
      <div class="config-banner-actions">
        <button class="primary" onclick={save} disabled={busy || loading}>
          {t("Salvesta mängu config")}
        </button>
        <button
          class="ghost"
          onclick={() => showStat("dependencies")}
          disabled={busy || loading}
        >{t("Vaata kontrolli")}</button>
      </div>
    </div>
  {/if}

  {#if uiMode === "advanced" && error}
    <div class="banner error">{error}</div>
  {/if}
  {#if uiMode === "advanced" && message}
    <div class="banner info">{message}</div>
  {/if}

  {#if uiMode === "advanced" && editingProfile && currentProfile === editingProfile}
    <div class="editing-banner">
      <div class="editing-text">
        <strong>{t("Muudad profiili „{name}”.", { name: editingProfile })}</strong>
        <span>{t("Muudatused lähevad profiili alles „Salvesta profiil” vajutamisel.")}</span>
        {#if editingDiff.added.length > 0 || editingDiff.removed.length > 0}
          <span class="editing-diff">
            +{editingDiff.added.length} / −{editingDiff.removed.length}
          </span>
        {/if}
      </div>
      <div class="editing-actions">
        <button class="primary" onclick={saveCurrentProfile} disabled={busy}>
          {t("Salvesta profiil")}
        </button>
        <button class="ghost" onclick={cancelEditingProfile} disabled={busy}>
          {t("Tühista muudatused")}
        </button>
      </div>
    </div>
  {/if}

  {#if loading}
    <div class="loading">
      <div class="spinner"></div>
      <div>
        <strong>{t("Laen modi nimekirja…")}</strong>
        <p>{t("Modi nimekiri, AML settings + Workshopi metadata")}</p>
      </div>
    </div>
  {:else if snapshot && uiMode === "simple"}
    <SimpleMode
      {snapshot}
      {currentProfile}
      {busy}
      {busyText}
      {error}
      {message}
      {launchIssue}
      {gameConfigDirty}
      diagnosticsFresh={diagnosticsFresh()}
      coverUrls={profileCovers}
      {t}
      onSelectProfile={(name) => applyProfileForSimple(name)}
      onStart={startCurrentProfile}
      onManageProfiles={openManageProfiles}
      onChecks={openChecksFromSimple}
      onOpenWorkshop={(id) => invoke("open_workshop", { id })}
    />
  {:else if snapshot}
    <section class="summary">
      <button
        type="button"
        class="stat"
        class:selected={tab === "mods" && stateFilter === "active"}
        onclick={() => showStat("active")}
        title={t("Näita aktiivseid")}
      >
        <strong>{snapshot.summary.active}</strong>
        <span>{t("aktiivset")}</span>
      </button>
      <button
        type="button"
        class="stat"
        class:selected={tab === "mods" && stateFilter === "all" && categoryFilter === "all" && tagFilter === "all" && !query}
        onclick={() => showStat("all")}
        title={t("Näita kõiki")}
      >
        <strong>{snapshot.summary.total}</strong>
        <span>{t("kokku")}</span>
      </button>
      <button
        type="button"
        class="stat"
        class:warn={snapshot.summary.missing > 0}
        class:selected={tab === "mods" && stateFilter === "missing"}
        onclick={() => showStat("missing")}
        title={t("Näita puuduvaid aktiivseid modisid")}
      >
        <strong>{snapshot.summary.missing}</strong>
        <span>{t("puudub")}</span>
      </button>
      <button
        type="button"
        class="stat"
        class:warn={snapshot.summary.unknown_active > 0}
        onclick={() => showStat("unknown")}
        title={t("Tundmatud aktiivsed modid")}
      >
        <strong>{snapshot.summary.unknown_active}</strong>
        <span>{t("tundmatut")}</span>
      </button>
      <button
        type="button"
        class="stat"
        class:warn={snapshot.summary.updates > 0}
        class:selected={tab === "mods" && stateFilter === "updates"}
        onclick={() => showStat("updates")}
        title={t("Näita uuendusi")}
      >
        <strong>{snapshot.summary.updates}</strong>
        <span>{t("uuendust")}</span>
      </button>
      <button
        type="button"
        class="stat"
        class:warn={snapshot.summary.conflicts > 0}
        onclick={() => showStat("conflicts")}
        title={t("Ava konfliktid")}
      >
        <strong>{snapshot.summary.conflicts}</strong>
        <span>{t("konflikti")}</span>
      </button>
      <button
        type="button"
        class="stat"
        class:warn={snapshot.summary.missing_dependencies > 0}
        onclick={() => showStat("dependencies")}
        title={t("Ava puuduvad sõltuvused")}
      >
        <strong>{snapshot.summary.missing_dependencies}</strong>
        <span>{t("puuduvat sõltuvust")}</span>
      </button>
      <button
        type="button"
        class="stat"
        onclick={() => showStat("categories")}
        title={t("Ava kategooriad")}
      >
        <strong>{snapshot.summary.categories}</strong>
        <span>{t("kategooriat")}</span>
      </button>
      <div class="stat args" title={snapshot.summary.args.join(" ")}>
        <strong>{snapshot.summary.args.join(" ") || "—"}</strong>
        <span>{t("käivituse argumendid")}</span>
      </div>
      <div class="chips">
        <span class:ok={snapshot.summary.steam_running} class="chip">
          {snapshot.summary.steam_running ? t("Steam töötab") : t("Steam ei tööta")}
        </span>
        <span class:ok={!snapshot.summary.aml_running} class="chip">
          {snapshot.summary.aml_running ? t("AML on lahti") : t("AML suletud")}
        </span>
        <span class:ok={!snapshot.summary.game_running} class="chip">
          {snapshot.summary.game_running ? t("XCOM 2 töötab") : t("XCOM 2 suletud")}
        </span>
      </div>
    </section>

    {#if snapshot.summary.aml_running}
      <div class="banner warn">
        {t("AML on veel lahti. Salvestamiseks ja mängu käivitamiseks sulge AML (see hoiab settings.json-i ja võib configu üle kirjutada).")}
      </div>
    {/if}

    <section class="toolbar">
      <div class="tabs" role="tablist">
        <button class:active={tab === "mods"} onclick={() => (tab = "mods")} role="tab">
          {t("Modid")}
        </button>
        <button class:active={tab === "order"} onclick={() => (tab = "order")} role="tab">
          {t("Laadimisjärjekord")}
          <span class="tab-count">{activeOrder.length}</span>
        </button>
        <button class:active={tab === "categories"} onclick={() => (tab = "categories")} role="tab">
          {t("Kategooriad")}
          <span class="tab-count">{snapshot.categories.length}</span>
        </button>
        <button class:active={tab === "profiles"} onclick={() => (tab = "profiles")} role="tab">
          {t("Profiilid")}
          <span class="tab-count">{snapshot.profiles.length}</span>
        </button>
        <button class:active={tab === "diagnostics"} onclick={() => (tab = "diagnostics")} role="tab">
          {t("Kontroll")}
          <span class:has-issues={diagnosticsIssueCount > 0} class="tab-count">
            {diagnosticsIssueCount}
          </span>
        </button>
      </div>

      <div class="filters">
        <input
          class="search"
          type="search"
          placeholder={t("Otsi nime, autori, ID või kategooria järgi…")}
          bind:value={query}
        />
        <select bind:value={stateFilter} title={t("Staatus")}>
          <option value="all">{t("Kõik")}</option>
          <option value="active">{t("Aktiivsed")}</option>
          <option value="inactive">{t("Väljas")}</option>
          <option value="missing">{t("Puuduvad")}</option>
          <option value="updates">{t("Uuendused")}</option>
        </select>
        <select bind:value={categoryFilter} title={t("Kategooria")}>
          <option value="all">{t("Kõik kategooriad")}</option>
          {#each categories as category}
            <option value={category}>{category}</option>
          {/each}
        </select>
        <select bind:value={tagFilter} title={t("Workshopi tag (vähemalt 2 modi)")}>
          <option value="all">{t("Kõik tagid")}</option>
          {#each workshopTags as tag}
            <option value={tag.name}>{tag.name} ({tag.count})</option>
          {/each}
        </select>
        <span class="result-count">{filteredMods.length} / {snapshot.summary.total}</span>
      </div>
    </section>

    <main class="content">
      {#if tab === "mods"}
        <div class="table">
          <div class="thead">
            <div class="cell check"></div>
            <div class="cell">{t("Nimi")}</div>
            <div class="cell">{t("Autor")}</div>
            <div class="cell">{t("Kategooria")}</div>
            <div class="cell">{t("Workshop")}</div>
            <div class="cell">{t("Suurus")}</div>
            <div class="cell">{t("Staatus")}</div>
          </div>
          <div class="tbody">
            {#each filteredMods as mod (mod.id)}
              <div
                class="tr"
                class:selected={selectedId === mod.id}
                role="button"
                tabindex="0"
                onclick={() => select(mod)}
                onkeydown={(event) => event.key === "Enter" && select(mod)}
              >
                <div class="cell check">
                  <input
                    type="checkbox"
                    checked={mod.active}
                    onclick={(event) => event.stopPropagation()}
                    onchange={() => toggle(mod)}
                    aria-label={`${mod.active ? t("Deaktiveeri") : t("Aktiveeri")} ${mod.name}`}
                  />
                </div>
                <div class="cell name" title={mod.name}>
                  <span class="name-text">{mod.name}</span>
                  {#if mod.missing}<span class="badge danger">{t("puudub")}</span>{/if}
                  {#if conflictModIds.has(mod.id)}<span class="badge warn">{t("konflikt")}</span>{/if}
                  {#if dependencyModIds.has(mod.id)}<span class="badge warn">dep</span>{/if}
                </div>
                <div class="cell muted">{mod.author || "—"}</div>
                <div class="cell muted" title={mod.category}>{mod.category}</div>
                <div class="cell mono">{mod.workshop_id ?? "—"}</div>
                <div class="cell mono">{formatSize(mod.size)}</div>
                <div class="cell">
                  {#if mod.active}
                    <span class="badge active">#{((mod.order ?? 0) + 1)}</span>
                  {:else}
                    <span class="badge">{t("väljas")}</span>
                  {/if}
                  {#if mod.update_available}<span class="badge update">{t("uuendus")}</span>{/if}
                </div>
              </div>
            {:else}
              <div class="empty">{t("Ühtegi modi ei leitud.")}</div>
            {/each}
          </div>
        </div>
      {:else if tab === "order"}
        <div class="order">
          <div class="order-head">
            <div>
              <strong>{t("Laadimisjärjekord")}</strong>
              <span class="muted">{t("— ülalpool laetakse enne, allpool hiljem")}</span>
            </div>
            <span class="result-count">{t("{count} aktiivset", { count: activeMods.length })}</span>
          </div>
          <div class="order-list" role="list">
            {#each activeMods as mod, index (mod.id)}
              <div
                class="order-row"
                role="listitem"
                class:dragging={dragId === mod.id}
                draggable="true"
                ondragstart={(event) => dragStart(event, mod.id)}
                ondragover={dragOver}
                ondrop={(event) => drop(event, mod.id)}
              >
                <span class="handle" title={t("Lohista")}>⠿</span>
                <span class="pos">{index + 1}</span>
                <span class="name">{mod.name}</span>
                <span class="author muted">{mod.author || "—"}</span>
                <span class="cat muted" title={mod.category}>{mod.category}</span>
                <div class="order-actions">
                  <button
                    class="mini"
                    onclick={() => moveBy(mod.id, -1)}
                    disabled={index === 0}
                    title={t("Liiguta üles")}
                  >↑</button>
                  <button
                    class="mini"
                    onclick={() => moveBy(mod.id, 1)}
                    disabled={index === activeMods.length - 1}
                    title={t("Liiguta alla")}
                  >↓</button>
                  <button class="mini danger" onclick={() => toggle(mod)} title={t("Eemalda aktiivsetest")}>×</button>
                </div>
              </div>
            {:else}
              <div class="empty">{t("Kõik modid on väljas.")}</div>
            {/each}
          </div>
        </div>
      {:else if tab === "categories"}
        <div class="manager">
          <div class="manager-title">
            <h2>{t("Kategooriad")}</h2>
            <span class="muted">{t("{count} kategooriat", { count: snapshot.categories.length })}</span>
          </div>

          <div class="auto-card">
            <div class="auto-head">
              <div>
                <strong>{t("Automaatne kategoriseerimine")}</strong>
                <p>
                  {t("Kasutab Workshopi tage, modi nime ja kirjeldust. Nimi normaliseeritakse (CamelCase/underscore), WOTC / War of the Chosen on versioonimarker, mitte kategooria — nime ülejäänud osa otsustab. Õpib sinu käsitsi määratud kategooriatest ({count} näidet); käsitsi määratud modid jäävad paigale. Vaikimisi liigutatakse ainult “Unsorted” modid.", { count: manualCategoryCount })}
                </p>
              </div>
              <label class="auto-check">
                <input
                  type="checkbox"
                  bind:checked={autoOverwrite}
                  onchange={() => (autoPreview = null)}
                />
                {t("Kirjuta üle ka olemasolevad")}
              </label>
              {#if manualCategoryCount > 0}
                <button
                  class="mini danger"
                  onclick={clearCategoryLearning}
                  disabled={busy}
                >{t("Unusta õpitud ({count})", { count: manualCategoryCount })}</button>
              {/if}
              <button class="primary" onclick={previewAutoCategories} disabled={busy}>
                {t("Tee eelvaade")}
              </button>
            </div>

            {#if autoPreview}
              <div class="auto-summary">
                <span>{t("{count} modi liigub", { count: autoPreview.assignable })}</span>
                <span>{t("{count} kategooriasse", { count: autoPreview.groups.length })}</span>
                <span>{t("{count} jääb puutumata", { count: autoPreview.total - autoPreview.assignable })}</span>
                {#if autoPreview.training_examples > 0}
                  <span>{t("{count} õpitud", { count: autoPreview.learned })}</span>
                  <span>{t("{count} reeglitest", { count: autoPreview.rule_based })}</span>
                  <span>{t("{count} käsitsi", { count: autoPreview.manual_pinned })}</span>
                {:else}
                  <span>
                    {t("Õppimiseks vaja vähemalt 5 käsitsi määratud modi ({count} olemas)", { count: manualCategoryCount })}
                  </span>
                {/if}
              </div>
              {#if autoPreview.assignable === 0}
                <p class="auto-empty">
                  {t("Midagi liigutada ei ole. Kui soovid kõiki uuesti liigitada, märgi “Kirjuta üle ka olemasolevad”.")}
                </p>
              {:else}
                <div class="auto-groups">
                  {#each autoPreview.groups as group (group.name)}
                    <div
                      class="auto-group"
                      title={group.mods.slice(0, 10).map((mod) => mod.name).join("\n")}
                    >
                      <span class="badge warn">{group.count}</span>
                      <strong>{group.name}</strong>
                      <span class="muted">
                        {group.mods.slice(0, 3).map((mod) => mod.name).join(" · ")}{group.mods.length > 3 ? " …" : ""}
                      </span>
                    </div>
                  {/each}
                </div>
                <div class="auto-actions">
                  <button
                    class="primary"
                    onclick={applyAutoCategories}
                    disabled={busy || autoPreview.assignable === 0}
                  >{t("Rakenda {count} modi", { count: autoPreview.assignable })}</button>
                  <button class="ghost" onclick={() => (autoPreview = null)}>{t("Tühista")}</button>
                </div>
              {/if}
            {/if}
          </div>

          <div class="manager-add">
            <input
              type="text"
              placeholder={t("Uus kategooria…")}
              bind:value={newCategory}
              onkeydown={(event) => event.key === "Enter" && createCategory()}
            />
            <button class="primary" onclick={createCategory} disabled={busy}>{t("Lisa")}</button>
          </div>
          <div class="manager-list">
            {#each snapshot.categories as category (category.name)}
              <div class="manager-row">
                {#if editingCategory === category.name}
                  <input
                    type="text"
                    bind:value={editingName}
                    onkeydown={(event) => {
                      if (event.key === "Enter") renameCategory(category.name);
                      if (event.key === "Escape") editingCategory = null;
                    }}
                  />
                  <button class="primary" onclick={() => renameCategory(category.name)}>{t("Salvesta")}</button>
                  <button class="ghost" onclick={() => (editingCategory = null)}>{t("Tühista")}</button>
                {:else}
                  <strong>{category.name}</strong>
                  <span class="muted">{t("{count} modi", { count: categoryCounts.get(category.name) || 0 })}</span>
                  <button class="mini" onclick={() => { editingCategory = category.name; editingName = category.name; }}>{t("Nimeta")}</button>
                  {#if category.name !== "Unsorted"}
                    <button class="mini danger" onclick={() => deleteCategory(category.name)}>{t("Kustuta")}</button>
                  {/if}
                {/if}
              </div>
            {/each}
          </div>
          <p class="manager-note">
            {t("Modid määratakse kategooriasse paremal detailpaneelis. Kategooriafilter on modinimekirja kohal. Kategooriad on native appi omad; mängu config ei muutu.")}
          </p>
        </div>
      {:else if tab === "profiles"}
        <div class="manager">
          <div class="manager-title">
            <h2>{t("Profiilid")}</h2>
            <span class="muted">{t("{count} profiili", { count: snapshot.profiles.length })}</span>
            <div class="manager-actions">
              <button
                class="ghost"
                onclick={exportAllProfiles}
                disabled={busy || snapshot.profiles.length === 0}
              >{t("Ekspordi kõik")}</button>
              <button class="ghost" onclick={chooseProfileImport} disabled={busy}>
                {t("Impordi")}
              </button>
              <button class="primary" onclick={openProfileModal} disabled={busy}>
                {t("Uus profiil")}
              </button>
            </div>
          </div>

          {#if showSimpleHint}
            <div class="simple-hint">
              <div class="simple-hint-text">
                <strong>{t("Modpack on valmis.")}</strong>
                <span>{t("Lihtvaates näed ainult profiile ja Start-nuppu — profiili vahetamine käib ühe klikiga.")}</span>
              </div>
              <div class="simple-hint-actions">
                <button class="primary" onclick={() => setUiMode("simple")}>
                  {t("Ava lihtvaade")}
                </button>
                <button class="ghost" onclick={dismissSimpleHint}>
                  {t("Ära näita uuesti")}
                </button>
              </div>
            </div>
          {/if}

          <div class="manager-add">
            <input
              type="text"
              placeholder={t("Profiili nimi…")}
              bind:value={profileName}
              onkeydown={(event) => event.key === "Enter" && saveProfile()}
            />
            <button class="primary" onclick={saveProfile} disabled={busy}>
              {t("Salvesta praegune profiilina ({count})", { count: activeOrder.length })}
            </button>
          </div>
          <div class="profile-list">
            {#each snapshot.profiles as profile (profile.name)}
              <div class="profile-card" class:with-cover={profileCovers[profile.name]}>
                {#if profileCovers[profile.name]}
                  <img
                    class="profile-thumb"
                    src={profileCovers[profile.name]}
                    alt=""
                  />
                {/if}
                <div class="profile-card-main">
                  <div class="profile-card-title">
                    <strong>{profile.name}</strong>
                    {#if profile.source_title}
                      <span class="badge">{t("Steam collection")}</span>
                    {/if}
                    {#if profile.pending_count > 0}
                      <span class="badge warn">
                        {t("{count} ootab", { count: profile.pending_count })}
                      </span>
                    {/if}
                    {#if profile.source_id && profile.deactivated_mod_ids?.length > 0}
                      <span class="badge warn">
                        {t("{count} deaktiveeritud", { count: profile.deactivated_mod_ids.length })}
                      </span>
                    {/if}
                  </div>
                  <div class="profile-card-meta">
                    <span>{t("{count} modi", { count: profile.count })}</span>
                    {#if profile.revision_count > 0}
                      <span class="muted">{t("{count} versiooni", { count: profile.revision_count })}</span>
                    {/if}
                    {#if profile.source_title}
                      <span class="muted">{profile.source_title}</span>
                    {/if}
                  </div>
                  {#if profile.description}
                    <p class="profile-desc">{profile.description}</p>
                  {/if}
                  {#if profile.count === 0}
                    <p class="profile-empty-hint">
                      {t("Profiil on tühi. Vajuta „Lisa modisid”.")}
                    </p>
                  {/if}
                  {#if profile.parent}
                    <div class="profile-parent">
                      <span>{t("Põhineb: {name}", { name: profile.parent })}</span>
                    </div>
                  {/if}
                </div>
                <div class="profile-actions">
                  <button class="mini" onclick={() => editProfile(profile)} disabled={busy}>
                    {profile.count === 0 ? t("Lisa modisid") : t("Muuda modisid")}
                  </button>
                  <button class="mini" onclick={() => applyProfile(profile.name)}>{t("Rakenda")}</button>
                  <details class="profile-menu">
                    <summary class="mini" title={t("Rohkem toiminguid")}>⋯</summary>
                    <div
                      class="profile-menu-items"
                      onclick={(event) =>
                        event.currentTarget.closest("details")?.removeAttribute("open")}
                    >
                      <button class="mini" onclick={() => openProfileHistory(profile)} disabled={busy}>
                        {t("Ajalugu")}{profile.revision_count > 0 ? ` (${profile.revision_count})` : ""}
                      </button>
                      <button class="mini" onclick={() => duplicateProfile(profile)} disabled={busy}>
                        {t("Dubleeri")}
                      </button>
                      {#if profile.parent}
                        <button class="mini" onclick={() => syncProfileToParent(profile)} disabled={busy}>
                          {t("Uuenda originaali")}
                        </button>
                      {/if}
                      {#if profile.source_id}
                        <button class="mini" onclick={() => openProfileCollection(profile)}>
                          {t("Kollektsioon")}
                        </button>
                        <button
                          class="mini"
                          onclick={() => invoke("open_workshop", { id: profile.source_id })}
                        >{t("Ava Steamis")}</button>
                      {/if}
                      <button class="mini" onclick={() => pickProfileCover(profile)}>
                        {profileCovers[profile.name] ? t("Muuda pilti") : t("Lisa pilt")}
                      </button>
                      {#if profileCovers[profile.name]}
                        <button class="mini danger" onclick={() => clearProfileCover(profile)}>
                          {t("Eemalda pilt")}
                        </button>
                      {/if}
                      <button class="mini" onclick={() => exportProfile(profile)}>{t("Ekspordi")}</button>
                      <button class="mini danger" onclick={() => askDeleteProfile(profile)}>{t("Kustuta")}</button>
                    </div>
                  </details>
                </div>
              </div>
            {/each}
            {#if snapshot.profiles.length === 0}
              <div class="empty">{t("Profiile veel pole.")}</div>
            {/if}
          </div>
          <p class="manager-note">
            {t("Modide muutmiseks vajuta profiilikaardil „Muuda modisid”. Lülita Modid-tabis modid sisse või välja ja vajuta „Salvesta profiil”. Seejärel vajuta „Salvesta”, et nimekiri mängu configusse läheks.")}
          </p>

          <div class="cleanup-panel" id="cleanup-panel">
            <div class="manager-title">
              <h2>{t("Kasutamata modid")}</h2>
              {#if cleanupReport}
                <span class="muted">
                  {t("{count} kandidaati", { count: cleanupReport.candidates.length })}
                </span>
              {/if}
              <div class="manager-actions">
                <button
                  class="ghost"
                  onclick={() => scanCleanup()}
                  disabled={cleanupLoading || busy}
                >{cleanupLoading ? t("Skaneerin…") : t("Skaneeri")}</button>
              </div>
            </div>
            <p class="manager-note">
              {t("Kasutamata modid on paigaldatud Workshopi modid, mida ükski profiil ega aktiivne nimekiri ei kasuta. See vaade ei kustuta faile ega muuda Steami tellimusi.")}
            </p>
            {#if cleanupLoading}
              <div class="loading profile-loading">
                <div class="spinner"></div>
                <span>{t("Skaneerin kasutamata modisid…")}</span>
              </div>
            {:else if cleanupError}
              <div class="banner error">{cleanupError}</div>
            {:else if cleanupReport}
              <div class="diag-stats cleanup-stats">
                <div class="diag-stat">
                  <strong>{cleanupReport.installed}</strong>
                  <span>{t("paigaldatud modi")}</span>
                  <em>{t("{count} aktiivset", { count: cleanupReport.active })}</em>
                </div>
                <div class="diag-stat">
                  <strong>{cleanupReport.referenced}</strong>
                  <span>{t("kasutusel")}</span>
                  <em>{t("profiilides või aktiivses nimekirjas")}</em>
                </div>
                <div class="diag-stat" class:warn={cleanupReport.candidates.length > 0}>
                  <strong>{cleanupReport.candidates.length}</strong>
                  <span>{t("kasutamata")}</span>
                  <em>{formatSize(cleanupReport.candidate_size)}</em>
                </div>
              </div>
              {#if cleanupReport.candidates.length === 0}
                <div class="empty">{t("Kasutamata modisid ei leitud.")}</div>
              {:else}
                <div class="cleanup-list">
                  {#each cleanupReport.candidates as mod (mod.workshop_id)}
                    <div class="cleanup-row">
                      <div class="cleanup-name">
                        <strong>{mod.name}</strong>
                        <span class="mono muted">{mod.workshop_id} · {formatSize(mod.size)}</span>
                      </div>
                      <button
                        class="mini"
                        onclick={() => invoke("open_workshop", { id: mod.workshop_id })}
                      >{t("Ava Steamis")}</button>
                      <button
                        class="mini danger"
                        onclick={() => removeUnusedMod(mod)}
                        disabled={cleanupLoading}
                      >{t("Eemalda kettalt")}</button>
                    </div>
                  {/each}
                </div>
                <p class="manager-note">
                  {t("Tellimuse tühistamine toimub Steamis. „Eemalda kettalt” viib kausta prügikasti; kui tellimus on alles, võib Steam selle uuesti alla laadida.")}
                </p>
              {/if}
            {:else}
              <div class="empty">
                {t("Vajuta „Skaneeri”, et leida modid, mida ükski profiil ei kasuta.")}
              </div>
            {/if}
          </div>

          <div class="cleanup-panel" id="trash-panel">
            <div class="manager-title">
              <h2>{t("Prügikast")}</h2>
              {#if trashEntries !== null}
                <span class="muted">{t("{count} modi", { count: trashEntries.length })}</span>
              {/if}
              <div class="manager-actions">
                <button
                  class="ghost"
                  onclick={loadTrash}
                  disabled={trashLoading || busy}
                >{trashLoading ? t("Laen…") : t("Laadi prügikast")}</button>
              </div>
            </div>
            <p class="manager-note">
              {t("Siin on ainult selle rakenduse poolt Workshopist prügikasti viidud modid. Taastamine viib kausta tagasi algsesse asukohta.")}
            </p>
            {#if trashError}
              <div class="banner error">{trashError}</div>
            {:else if trashEntries === null}
              <div class="empty">
                {t("Vajuta „Laadi prügikast”, et näha eemaldatud modisid.")}
              </div>
            {:else if trashEntries.length === 0}
              <div class="empty">{t("Prügikast on tühi.")}</div>
            {:else}
              <div class="cleanup-list">
                {#each trashEntries as entry (entry.id)}
                  <div class="cleanup-row">
                    <div class="cleanup-name">
                      <strong>{entry.name}</strong>
                      <span class="mono muted">
                        {entry.original_path} · {formatUnix(entry.time_deleted)}
                      </span>
                    </div>
                    <button
                      class="mini"
                      onclick={() => restoreTrashEntry(entry)}
                      disabled={trashLoading}
                    >{t("Taasta")}</button>
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        </div>
      {:else}
        <div class="manager diagnostics">
          <div class="manager-title">
            <h2>{t("Kontroll")}</h2>
            {#if currentProfile}
              <span class="badge">{currentProfile}</span>
            {/if}
            <span class="muted">
              {#if snapshot.diagnostics}
                {t("viimane kontroll {date} · {count} aktiivset modi", {
                  date: formatUnix(snapshot.diagnostics.scanned_at),
                  count: snapshot.diagnostics.active_mods,
                })}
                {#if !diagnosticsFresh()}
                  <span class="badge warn">{t("aegunud")}</span>
                {/if}
              {:else}
                {t("veel pole tehtud")}
              {/if}
            </span>
            <button
              class="primary manager-run"
              onclick={runDiagnostics}
              disabled={busy}
            >{t("Käivita kontroll")}</button>
          </div>

          <p class="manager-note">
            {t("Kontroll loeb aktiivsete modide Config/**/XComEngine.ini ja Src/**/*.uc faile. Konflikt = mitu aktiivset modi kirjutab üle sama baasklassi erineva reaga. Sõltuvused loetakse AML settings.json-ist; “paigaldatud, väljas” tähendab, et mod on olemas, kuid pole aktiivne.")}
          </p>

          {#if !snapshot.diagnostics}
            <div class="empty">
              {t("Kontrolli veel tehtud pole. Vajuta “Käivita kontroll”.")}
            </div>
          {:else}
            <div class="diag-stats">
              <div class="diag-stat" class:warn={snapshot.diagnostics.missing_dependency_count > 0}>
                <strong>{snapshot.diagnostics.missing_dependency_count}</strong>
                <span>{t("{count} modi puudulike sõltuvustega", { count: snapshot.diagnostics.missing_dependency_count })}</span>
                <em>{t("{count} puuduvat viidet", { count: snapshot.diagnostics.missing_dependency_links })}</em>
              </div>
              <div class="diag-stat" class:warn={snapshot.diagnostics.conflicts.length > 0}>
                <strong>{snapshot.diagnostics.conflicts.length}</strong>
                <span>{t("{count} klassikonflikti", { count: snapshot.diagnostics.conflicts.length })}</span>
                <em>{t("{count} modi osalevad", { count: snapshot.diagnostics.conflict_mod_count })}</em>
              </div>
              <div class="diag-stat" class:warn={snapshot.diagnostics.duplicate_groups.length > 0}>
                <strong>{snapshot.diagnostics.duplicate_groups.length}</strong>
                <span>{t("{count} duplikaatrühma", { count: snapshot.diagnostics.duplicate_groups.length })}</span>
                <em>{t("{count} modi", { count: snapshot.diagnostics.duplicate_mod_count })}</em>
              </div>
            </div>

            <h3 class="diag-heading" id="diag-missing">{t("Puuduvad sõltuvused")}</h3>
            {#if snapshot.diagnostics.missing_dependencies.length === 0}
              <div class="empty">{t("Kõik sõltuvused on aktiivsed.")}</div>
            {:else}
              <div class="diag-list">
                {#each snapshot.diagnostics.missing_dependencies as issue (issue.mod_id)}
                  <details class="diag-item">
                    <summary>
                      <strong>{issue.mod_name}</strong>
                      <span class="badge warn">{issue.missing.length}</span>
                    </summary>
                    <div class="diag-body">
                      {#each issue.missing as dependency (dependency.workshop_id)}
                        <div class="dep-row">
                          <span class="mono">{dependency.workshop_id}</span>
                          <span>{dependency.title || t("Tundmatu mod")}</span>
                          {#each dependency.tags.slice(0, 4) as tag}
                            <span class="tag">{tag}</span>
                          {/each}
                          {#if dependency.tags.length > 4}
                            <span class="tag">+{dependency.tags.length - 4}</span>
                          {/if}
                          {#if dependency.installed}
                            <span class="badge">{t("paigaldatud, väljas")}</span>
                          {:else}
                            <span class="badge danger">{t("pole paigaldatud")}</span>
                          {/if}
                          {#if dependency.installed && dependency.mod_id}
                            <button
                              class="mini"
                              onclick={() => enableDependency(issue, dependency)}
                              disabled={busy}
                            >{t("Lülita sisse")}</button>
                          {/if}
                          <button
                            class="mini"
                            onclick={() => invoke("open_workshop", { id: dependency.workshop_id })}
                          >{t("Ava Steamis")}</button>
                        </div>
                      {/each}
                    </div>
                  </details>
                {/each}
              </div>
            {/if}

            <h3 class="diag-heading" id="diag-conflicts">{t("Klassikonfliktid")}</h3>
            {#if snapshot.diagnostics.conflicts.length === 0}
              <div class="empty">{t("Konflikte ei leitud.")}</div>
            {:else}
              <div class="diag-list">
                {#each snapshot.diagnostics.conflicts as conflict (conflict.base_class)}
                  <details class="diag-item">
                    <summary>
                      <strong class="mono">{conflict.base_class}</strong>
                      <span class="badge warn">{conflict.overrides.length}</span>
                    </summary>
                    <div class="diag-body">
                      {#each conflict.overrides as override (override.mod_id + override.new_class)}
                        <div class="override-row">
                          <span>{override.mod_name}</span>
                          <span class="mono muted">→ {override.new_class}</span>
                          <span class="badge">{override.kind === "screen" ? "screen listener" : "class override"}</span>
                          <button
                            class="mini danger"
                            onclick={() => deactivateConflictMod(override)}
                            disabled={busy}
                          >{t("Lülita välja")}</button>
                        </div>
                      {/each}
                    </div>
                  </details>
                {/each}
              </div>
            {/if}

            <h3 class="diag-heading">{t("Duplikaat-ID-d")}</h3>
            {#if snapshot.diagnostics.duplicate_groups.length === 0}
              <div class="empty">{t("Duplikaat-ID-sid ei leitud.")}</div>
            {:else}
              <div class="diag-list">
                {#each snapshot.diagnostics.duplicate_groups as group (group.id)}
                  <details class="diag-item">
                    <summary>
                      <strong class="mono">{group.id}</strong>
                      <span class="badge warn">{group.mods.length}</span>
                    </summary>
                    <div class="diag-body">
                      {#each group.mods as mod (mod.mod_id + (mod.workshop_id ?? ""))}
                        <div class="override-row">
                          <span>{mod.name}</span>
                          <span class="mono muted">{mod.mod_id}</span>
                          <span class="mono muted">{mod.workshop_id ?? "—"}</span>
                        </div>
                      {/each}
                    </div>
                  </details>
                {/each}
              </div>
            {/if}
            <h3 class="diag-heading" id="diag-unknown">
              {t("Tundmatud aktiivsed modid")}
              {#if snapshot.diagnostics.unknown_active_ids.length > 0}
                <span class="badge warn">{snapshot.diagnostics.unknown_active_ids.length}</span>
              {/if}
            </h3>
            {#if snapshot.diagnostics.unknown_active_ids.length === 0}
              <div class="empty">{t("Kõik aktiivsed modid on kataloogis olemas.")}</div>
            {:else}
              <div class="diag-list">
                {#each snapshot.diagnostics.unknown_active_ids as id (id)}
                  <div class="unknown-row">
                    <span class="mono">{id}</span>
                    <span class="muted">{t("Seda ID-d ei leitud kataloogist ega Workshopi kaustast.")}</span>
                  </div>
                {/each}
              </div>
              <p class="manager-note">
                {t("Need ID-d on XComModOptions.ini-s aktiivsed, aga launcher ei leia neile modi. Kontrolli modi kausta või eemalda ID configust.")}
              </p>
            {/if}
          {/if}
        </div>
      {/if}

      {#if selected}
        <div
          class="details-resizer"
          class:active={detailsResizing}
          role="separator"
          aria-orientation="vertical"
          aria-label={t("Muuda külgpaneeli laiust")}
          aria-valuenow={detailsWidth}
          aria-valuemin={260}
          aria-valuemax={1200}
          tabindex="0"
          title={t("Lohista paneeli laiuse muutmiseks. Topeltklõps taastab vaikeväärtuse.")}
          onpointerdown={startDetailsResize}
          onkeydown={handleDetailsResizeKey}
          ondblclick={resetDetailsWidth}
        ></div>
        <aside class="details" style:width={`${detailsWidth}px`}>
          <button
            class="close"
            onclick={() => {
              selectedId = null;
              details = null;
            }}
            aria-label={t("Sulge")}
          >×</button>
          <h2>{selected.name}</h2>
          <p class="muted">{selected.author || t("Tundmatu autor")}</p>

          <dl>
            <dt>ID</dt>
            <dd class="mono">{selected.id}</dd>

            <dt>{t("Kategooria")}</dt>
            <dd>
              <select
                value={selected.category}
                onchange={(event) => setCategory(selected, event.target.value)}
              >
                {#each snapshot.categories as category (category.name)}
                  <option value={category.name}>{category.name}</option>
                {/each}
              </select>
              {#if selected.manual_category}
                <span class="badge manual">{t("käsitsi")}</span>
                <button
                  class="mini"
                  onclick={() => clearManualCategory(selected)}
                  disabled={busy}
                  title={t("Eemalda see kategooria õppimisest")}
                >{t("vabasta")}</button>
              {/if}
            </dd>

            <dt>{t("Workshop")}</dt>
            <dd class="mono">{selected.workshop_id ?? "—"}</dd>

            <dt>{t("Suurus")}</dt>
            <dd>{formatSize(selected.size)}</dd>

            <dt>WotC</dt>
            <dd>{selected.built_for_wotc ? t("jah") : t("ei")}</dd>

            <dt>{t("Staatus")}</dt>
            <dd>
              {selected.active
                ? t("aktiivne (#{index})", { index: (selected.order ?? 0) + 1 })
                : t("väljas")}
            </dd>

            <dt>{t("Asukoht")}</dt>
            <dd class="path">{selected.path || "—"}</dd>
          </dl>

          <button class="ghost open-workshop" onclick={() => openConfigEditor(selected)}>
            {t("Muuda configut (INI)")}
          </button>
          <button class="ghost open-workshop" onclick={() => openReadme(selected)}>
            {t("Ava ReadMe")}
          </button>

          {#if modDependencyIssue(selected.id) || modConflicts(selected.id).length}
            <h3>{t("Kontroll")}</h3>
            {#if modDependencyIssue(selected.id)}
              <div class="detail-issue">
                <strong>{t("Puuduvad sõltuvused")}</strong>
                {#each modDependencyIssue(selected.id).missing as dependency (dependency.workshop_id)}
                  <div class="dep-row">
                    <span class="mono">{dependency.workshop_id}</span>
                    <span>{dependency.title || t("Tundmatu mod")}</span>
                    {#each dependency.tags.slice(0, 4) as tag}
                      <span class="tag">{tag}</span>
                    {/each}
                    {#if dependency.tags.length > 4}
                      <span class="tag">+{dependency.tags.length - 4}</span>
                    {/if}
                    {#if dependency.installed}
                      <span class="badge">{t("paigaldatud, väljas")}</span>
                    {:else}
                      <span class="badge danger">{t("pole paigaldatud")}</span>
                    {/if}
                    {#if dependency.installed && dependency.mod_id}
                      <button
                        class="mini"
                        onclick={() => enableDependency(modDependencyIssue(selected.id), dependency)}
                        disabled={busy}
                      >{t("Lülita sisse")}</button>
                    {/if}
                  </div>
                {/each}
              </div>
            {/if}
            {#each modConflicts(selected.id) as conflict (conflict.base_class)}
              <div class="detail-issue">
                <strong>{t("Konflikt: {class}", { class: conflict.base_class })}</strong>
                {#each conflict.overrides as override (override.mod_id + override.new_class)}
                  <div class="override-row">
                    <span>{override.mod_name}</span>
                    <span class="mono muted">→ {override.new_class}</span>
                  </div>
                {/each}
              </div>
            {/each}
          {/if}

          {#if details}
            <h3>{t("Workshop")}</h3>
            <dl>
              <dt>{t("Uuendatud")}</dt>
              <dd>{formatUnix(details.time_updated)}</dd>
              <dt>{t("Kohalik")}</dt>
              <dd>{formatUnix(details.local_timeupdated)}</dd>
              <dt>{t("Uuendus")}</dt>
              <dd>
                {#if details.update_available}
                  <span class="badge update">{t("saadaval")}</span>
                {:else}
                  —
                {/if}
              </dd>
              <dt>{t("Tellijaid")}</dt>
              <dd>{details.subscriptions ?? "—"}</dd>
            </dl>

            {#if details.tags?.length}
              <div class="tag-list">
                {#each details.tags as tag}
                  <span class="tag">{tag}</span>
                {/each}
              </div>
            {/if}

            <h3>{t("Kirjeldus")}</h3>
            <div class="description-text">{details.description || "—"}</div>

            {#if selected.workshop_id}
              <button
                class="primary open-workshop"
                onclick={() => invoke("open_workshop", { id: selected.workshop_id })}
              >{t("Ava Steamis")}</button>
            {/if}
          {:else}
            <p class="muted">{t("Laen Workshopi andmeid…")}</p>
          {/if}

          {#if selected.missing}
            <p class="badge danger">{t("Modi kaust puudub")}</p>
          {/if}
        </aside>
      {/if}
    </main>

    <footer class="status">
      <span title={snapshot.paths.config ?? ""}>Config: {snapshot.paths.config ?? "—"}</span>
      <span title={snapshot.paths.backup_dir}>Backup: {snapshot.paths.backup_dir}</span>
      <span title={snapshot.paths.workshop_dir ?? ""}>Workshop: {snapshot.paths.workshop_dir ?? "—"}</span>
      <button class="status-info" onclick={() => (aboutOpen = true)}>{t("Info")}</button>
    </footer>
  {/if}

  {#if configOpen}
    <div
      class="config-backdrop"
      role="presentation"
      onclick={(event) => {
        if (event.target === event.currentTarget) closeConfigEditor();
      }}
      onkeydown={(event) => {
        if (event.key === "Escape") closeConfigEditor();
      }}
    >
      <div
        class="config-modal"
        role="dialog"
        aria-modal="true"
        aria-label={t("Modi config")}
      >
        <header class="config-head">
          <div>
            <h2>{t("Modi config")}</h2>
            <p class="muted">
              {configMod?.name} · {configPath || t("vali fail")}
            </p>
          </div>
          <div class="config-head-actions">
            {#if configDirty}<span class="badge warn">{t("muudetud")}</span>{/if}
            <button
              class="ghost"
              onclick={() => configPath && loadConfigFile(configPath)}
              disabled={!configPath || configLoading || configSaving}
            >{t("Lae uuesti")}</button>
            <button class="ghost" onclick={closeConfigEditor}>{t("Sulge")}</button>
            <button
              class="primary"
              onclick={saveConfigFile}
              disabled={!configDirty || configSaving || configLoading}
            >
              {configSaving ? t("Salvestan…") : t("Salvesta")}
            </button>
          </div>
        </header>

        <div class="config-layout">
          <aside class="config-files">
            <div class="config-files-title">
              <span>{t("Config failid")}</span>
              <span>{configFiles.length}</span>
            </div>
            {#each configFiles as file (file.path)}
              <button
                class="config-file"
                class:active={file.path === configPath}
                onclick={() => loadConfigFile(file.path)}
              >
                <span>{file.path.replace(/^Config\//, "")}</span>
                <span class="muted">{formatSize(file.size)}</span>
              </button>
            {/each}
            {#if !configFiles.length && !configLoading}
              <div class="empty">{t("Sellel modil pole Config/*.ini faile.")}</div>
            {/if}
          </aside>

          <section class="config-main">
            <div class="config-meta">
              {#if configMeta}
                <span>{configMeta.encoding}</span>
                <span>{configMeta.newline === "crlf" ? "CRLF" : "LF"}</span>
                <span>{formatUnix(configMeta.modified)}</span>
                {#if configMeta.read_only}<span class="badge danger">{t("ainult lugemine")}</span>{/if}
              {/if}
              <span class="config-meta-right">{t("Ctrl+S salvestab · Esc sulgeb")}</span>
            </div>

            {#if configError}
              <div class="banner error config-banner">{configError}</div>
            {/if}
            {#if configMessage}
              <div class="banner info config-banner">{configMessage}</div>
            {/if}

            {#if configLoading}
              <div class="loading config-loading">
                <div class="spinner"></div>
                <span>{t("Laen configut…")}</span>
              </div>
            {:else if configPath}
              <textarea
                class="config-text"
                bind:this={configTextarea}
                bind:value={configContent}
                spellcheck="false"
                readonly={configMeta?.read_only}
                onkeydown={handleEditorKeydown}
              ></textarea>
            {:else}
              <div class="empty">{t("Vali vasakult config fail.")}</div>
            {/if}
          </section>
        </div>
      </div>
    </div>
  {/if}

  {#if readmeOpen}
    <div
      class="config-backdrop"
      role="presentation"
      onclick={(event) => {
        if (event.target === event.currentTarget) closeReadme();
      }}
      onkeydown={(event) => {
        if (event.key === "Escape") closeReadme();
      }}
    >
      <div class="config-modal readme-modal" role="dialog" aria-modal="true" aria-label="ReadMe">
        <header class="config-head">
          <div>
            <h2>ReadMe</h2>
            <p class="muted" title={readme?.path ?? ""}>
              {readmeMod?.name}{readme?.file ? ` · ${readme.file}` : ""}
            </p>
          </div>
          <div class="config-head-actions">
            {#if readme?.format === "markdown"}<span class="badge">Markdown</span>{/if}
            {#if readme?.placeholder}<span class="badge warn">{t("vaikesisu")}</span>{/if}
            <button class="ghost" onclick={closeReadme}>{t("Sulge")}</button>
          </div>
        </header>
        <div class="readme-body">
          {#if readmeLoading}
            <div class="loading readme-loading">
              <div class="spinner"></div>
              <span>{t("Laen ReadMe-d…")}</span>
            </div>
          {:else if readmeError}
            <div class="banner error">{readmeError}</div>
          {:else if readme?.found}
            <pre class="readme-text">{readme.content}</pre>
          {:else}
            <div class="empty">{t("Sellel modil ei ole ReadMe.txt ega ReadMe.md faili.")}</div>
          {/if}
        </div>
      </div>
    </div>
  {/if}

  {#if profileModalOpen}
    <div
      class="config-backdrop"
      role="presentation"
      onclick={(event) => {
        if (event.target === event.currentTarget) closeProfileModal();
      }}
      onkeydown={(event) => {
        if (event.key === "Escape") closeProfileModal();
      }}
    >
      <div class="config-modal profile-modal" role="dialog" aria-modal="true" aria-label={t("Uus profiil")}>
        <header class="config-head">
          <div>
            <h2>{t("Uus profiil")}</h2>
            <p class="muted">{t("Loo profiil kollektsioonist, praegusest nimekirjast või tühjalt.")}</p>
          </div>
          <div class="config-head-actions">
            <button class="ghost" onclick={closeProfileModal} disabled={busy}>{t("Sulge")}</button>
          </div>
        </header>

        <div class="profile-body">
          <div class="profile-fields">
            <label>
              <span>{t("Nimi")}</span>
              <input type="text" bind:value={profileDraftName} placeholder={t("Profiili nimi…")} />
            </label>
            <label>
              <span>{t("Kirjeldus")}</span>
              <textarea bind:value={profileDraftDescription} rows="3"></textarea>
            </label>
          </div>

          <div class="profile-source">
            <label class="profile-radio">
              <input type="radio" bind:group={profileMode} value="collection" />
              {t("Workshopi kollektsioon")}
              <span class="source-badge">{t("soovitatud")}</span>
            </label>
            <label class="profile-radio">
              <input type="radio" bind:group={profileMode} value="current" />
              {t("Praegune aktiivne nimekiri ({count} modi)", { count: activeOrder.length })}
            </label>
            <label class="profile-radio">
              <input type="radio" bind:group={profileMode} value="empty" />
              {t("Tühi profiil (advanced)")}
            </label>
          </div>

          {#if profileMode === "current"}
            <p class="profile-source-note">
              {t("Profiil luuakse praegusest aktiivsest nimekirjast ({count} modi).", {
                count: activeOrder.length,
              })}
            </p>
          {:else if profileMode === "empty"}
            <p class="profile-source-note">
              {t("Tühi profiil on mõeldud edasijõudnutele. Modisid saab hiljem lisada „Muuda modisid” kaudu.")}
            </p>
          {/if}

          {#if profileMode === "collection"}
            <div class="collection-input">
              <input
                type="text"
                bind:value={collectionInput}
                placeholder={t("Steam Workshopi kollektsiooni URL või ID")}
                onkeydown={(event) => event.key === "Enter" && loadCollectionPreview()}
              />
              <button class="primary" onclick={loadCollectionPreview} disabled={collectionLoading}>
                {collectionLoading ? t("Laen…") : t("Lae kollektsioon")}
              </button>
            </div>

            {#if collectionError}
              <div class="banner error profile-banner">{collectionError}</div>
            {/if}

            {#if collectionLoading}
              <div class="loading profile-loading">
                <div class="spinner"></div>
                <span>{t("Laen Steam Workshopi kollektsiooni…")}</span>
              </div>
            {:else if collectionPreview}
              <div class="collection-summary">
                <div>
                  <strong>{collectionPreview.title}</strong>
                  <p class="muted">
                    {t("{total} modi · {installed} olemas · {missing} puudub", {
                      total: collectionPreview.total,
                      installed: collectionPreview.installed,
                      missing: collectionPreview.missing,
                    })}
                  </p>
                </div>
                <button
                  class="mini"
                  onclick={() => invoke("open_workshop", { id: collectionPreview.id })}
                >{t("Ava kollektsioon Steamis")}</button>
              </div>

              {#if collectionPreview.description}
                <p class="collection-desc">{stripBbcode(collectionPreview.description)}</p>
              {/if}

              <div class="collection-actions">
                <button class="mini" onclick={selectAllInstalled}>
                  {t("Vali kõik olemasolevad ({count})", { count: collectionPreview.installed })}
                </button>
                <button class="mini" onclick={clearCollectionSelection}>
                  {t("Tühista valik")}
                </button>
                <label class="profile-radio">
                  <input type="checkbox" bind:checked={collectionIncludePending} />
                  {t("Jälgi ka puuduvaid modisid")}
                </label>
              </div>

              <div class="collection-columns">
                <section class="collection-section">
                  <h3>
                    {t("Olemasolevad")}
                    <span class="muted">{collectionPreview.installed}</span>
                  </h3>
                  <div class="collection-list">
                    {#each collectionPreview.items.filter((item) => item.installed) as item (item.workshop_id)}
                      <label class="collection-item">
                        <input
                          type="checkbox"
                          checked={collectionSelected.has(item.workshop_id)}
                          onchange={() => toggleCollectionItem(item.workshop_id)}
                        />
                        <span>{item.title || item.workshop_id}</span>
                      </label>
                    {:else}
                      <div class="empty">{t("Ühtegi olemasolevat modi ei leitud.")}</div>
                    {/each}
                  </div>
                </section>

                <section class="collection-section">
                  <h3>
                    {t("Puuduvad")}
                    <span class="muted">{collectionPreview.missing}</span>
                  </h3>
                  <div class="collection-list">
                    {#each collectionPreview.items.filter((item) => item.status === "missing") as item (item.workshop_id)}
                      <div class="collection-item">
                        <span>{item.title || item.workshop_id}</span>
                        <button
                          class="mini"
                          onclick={() => invoke("open_workshop", { id: item.workshop_id })}
                        >{t("Ava Steamis")}</button>
                      </div>
                    {:else}
                      <div class="empty">{t("Kõik kollektsiooni modid on olemas.")}</div>
                    {/each}
                  </div>
                </section>
              </div>
            {/if}
          {/if}
        </div>

        <footer class="profile-footer">
          <button class="ghost" onclick={closeProfileModal} disabled={busy}>{t("Tühista")}</button>
          <button
            class="primary"
            onclick={createProfileFromModal}
            disabled={busy || collectionLoading || !profileDraftName.trim() || (profileMode === "collection" && !collectionPreview)}
          >{t("Loo profiil")}</button>
        </footer>
      </div>
    </div>
  {/if}

  {#if historyOpen}
    <div
      class="config-backdrop"
      role="presentation"
      onclick={(event) => {
        if (event.target === event.currentTarget) closeProfileHistory();
      }}
      onkeydown={(event) => {
        if (event.key === "Escape") closeProfileHistory();
      }}
    >
      <div
        class="config-modal history-modal"
        role="dialog"
        aria-modal="true"
        aria-label={t("Profiili ajalugu")}
      >
        <header class="config-head">
          <div>
            <h2>{t("Profiili ajalugu")}</h2>
            <p class="muted">{historyProfile?.name ?? ""}</p>
          </div>
          <div class="config-head-actions">
            <button class="ghost" onclick={closeProfileHistory} disabled={historyRestoring}>
              {t("Sulge")}
            </button>
          </div>
        </header>

        <div class="history-body">
          <aside class="history-list">
            {#if historyProfile}
              <div class="history-current">
                <strong>{t("Praegune")}</strong>
                <span>{t("{count} modi", { count: historyCurrentIds.length })}</span>
              </div>
            {/if}

            {#if historyLoading}
              <div class="loading">
                <div class="spinner"></div>
                <span>{t("Laen ajalugu…")}</span>
              </div>
            {:else if historyEntries.length === 0}
              <div class="empty">
                {t("Ajalugu on tühi. Iga Salvesta profiil loob uue versiooni.")}
              </div>
            {:else}
              {#each historyEntries as entry (entry.id)}
                <button
                  type="button"
                  class="history-entry"
                  class:active={historySelected === entry.id}
                  onclick={() => selectHistoryRevision(entry.id)}
                  disabled={historyRestoring}
                >
                  <strong>{formatUnix(entry.saved_at)}</strong>
                  <span>{t("{count} modi", { count: entry.active_count })}</span>
                </button>
              {/each}
            {/if}
          </aside>

          <section class="history-detail">
            {#if historyError}
              <div class="banner error">{historyError}</div>
            {:else if historyRevisionLoading}
              <div class="loading">
                <div class="spinner"></div>
                <span>{t("Laen versiooni…")}</span>
              </div>
            {:else if historyRevision}
              <div class="history-detail-head">
                <div>
                  <strong>{formatUnix(historyRevision.saved_at)}</strong>
                  <span class="muted">{t("Võrreldes praegusega")}</span>
                </div>
                <div class="history-counts">
                  <span class="added">+{historyDiff.added.length}</span>
                  <span class="removed">−{historyDiff.removed.length}</span>
                  <span class="reordered">↕ {historyDiff.reordered.length}</span>
                </div>
              </div>

              {#if historyDiff.added.length === 0 && historyDiff.removed.length === 0 && historyDiff.reordered.length === 0}
                <div class="empty">{t("Muutusi ei ole.")}</div>
              {:else}
                {#if historyDiff.added.length > 0}
                  <h3>
                    {t("Lisatud")}
                    <span class="muted">{historyDiff.added.length}</span>
                  </h3>
                  <ul class="history-changes">
                    {#each historyDiff.added.slice(0, 100) as id (id)}
                      <li>{modLabel(id)}</li>
                    {/each}
                  </ul>
                  {#if historyDiff.added.length > 100}
                    <p class="muted">
                      {t("ja veel {count}", { count: historyDiff.added.length - 100 })}
                    </p>
                  {/if}
                {/if}

                {#if historyDiff.removed.length > 0}
                  <h3>
                    {t("Eemaldatud")}
                    <span class="muted">{historyDiff.removed.length}</span>
                  </h3>
                  <ul class="history-changes">
                    {#each historyDiff.removed.slice(0, 100) as id (id)}
                      <li>{modLabel(id)}</li>
                    {/each}
                  </ul>
                  {#if historyDiff.removed.length > 100}
                    <p class="muted">
                      {t("ja veel {count}", { count: historyDiff.removed.length - 100 })}
                    </p>
                  {/if}
                {/if}

                {#if historyDiff.reordered.length > 0}
                  <h3>
                    {t("Ümber järjestatud")}
                    <span class="muted">{historyDiff.reordered.length}</span>
                  </h3>
                  <ul class="history-changes">
                    {#each historyDiff.reordered.slice(0, 100) as id (id)}
                      <li>{modLabel(id)}</li>
                    {/each}
                  </ul>
                  {#if historyDiff.reordered.length > 100}
                    <p class="muted">
                      {t("ja veel {count}", { count: historyDiff.reordered.length - 100 })}
                    </p>
                  {/if}
                {/if}
              {/if}

              <div class="history-actions">
                <button
                  class="primary"
                  onclick={restoreHistoryRevision}
                  disabled={historyRestoring || busy}
                >
                  {historyRestoring ? t("Taastan versiooni…") : t("Taasta see versioon")}
                </button>
              </div>
            {:else}
              <div class="empty">{t("Vali versioon.")}</div>
            {/if}
          </section>
        </div>
      </div>
    </div>
  {/if}

  {#if deleteOpen}
    <div
      class="config-backdrop"
      role="presentation"
      onclick={(event) => {
        if (event.target === event.currentTarget) closeDeleteProfile();
      }}
      onkeydown={(event) => {
        if (event.key === "Escape") closeDeleteProfile();
      }}
    >
      <div
        class="config-modal delete-modal"
        role="dialog"
        aria-modal="true"
        aria-label={t("Kustuta profiil")}
      >
        <header class="config-head">
          <div>
            <h2>{t("Kustuta profiil")}</h2>
            <p class="muted">{deleteTarget?.name ?? ""}</p>
          </div>
          <div class="config-head-actions">
            <button class="ghost" onclick={closeDeleteProfile} disabled={deleteApplying}>
              {t("Sulge")}
            </button>
          </div>
        </header>

        <div class="profile-body delete-body">
          {#if deleteLoading}
            <div class="loading profile-loading">
              <div class="spinner"></div>
              <span>{t("Arvutan profiili mõju…")}</span>
            </div>
          {:else if deletePreview}
            <div class="diag-stats delete-stats">
              <div class="diag-stat">
                <strong>{deletePreview.total}</strong>
                <span>{t("modi profiilis")}</span>
              </div>
              <div class="diag-stat" class:warn={deletePreview.unique > 0}>
                <strong>{deletePreview.unique}</strong>
                <span>{t("ainult selle profiili kasutuses")}</span>
                <em>{formatSize(deletePreview.unique_size)}</em>
              </div>
              <div class="diag-stat">
                <strong>{deletePreview.shared}</strong>
                <span>{t("kasutusel mujal")}</span>
                <em>{t("teised profiilid või aktiivne nimekiri")}</em>
              </div>
            </div>

            <p class="manager-note">
              {t("Profiili kustutamine eemaldab ainult profiili. Modisid ei kustutata ega unsubscribe'ita.")}
            </p>
            {#if deletePreview.unique > 0}
              <p class="delete-warning">
                {t("Pärast kustutamist ei kasuta {count} modi ükski profiil. Need jäävad kettale ja Steami tellimusse; kasutamata modide vaates saad need üle vaadata.", {
                  count: deletePreview.unique,
                })}
              </p>
            {/if}
            {#if deleteError}
              <div class="banner error profile-banner">{deleteError}</div>
            {/if}
          {:else if deleteError}
            <div class="banner error profile-banner">{deleteError}</div>
          {/if}
        </div>

        <footer class="profile-footer">
          <button class="ghost" onclick={closeDeleteProfile} disabled={deleteApplying}>
            {t("Tühista")}
          </button>
          <button
            class="primary delete-confirm"
            onclick={confirmDeleteProfile}
            disabled={deleteApplying || deleteLoading || !deletePreview}
          >
            {deleteApplying ? t("Kustutan…") : t("Kustuta profiil")}
          </button>
        </footer>
      </div>
    </div>
  {/if}

  {#if importOpen}
    <div
      class="config-backdrop"
      role="presentation"
      onclick={(event) => {
        if (event.target === event.currentTarget) closeProfileImport();
      }}
      onkeydown={(event) => {
        if (event.key === "Escape") closeProfileImport();
      }}
    >
      <div
        class="config-modal import-modal"
        role="dialog"
        aria-modal="true"
        aria-label={t("Impordi profiil")}
      >
        <header class="config-head">
          <div>
            <h2>{t("Impordi profiil")}</h2>
            <p class="muted">{fileBaseName(importPath)}</p>
          </div>
          <div class="config-head-actions">
            <button class="ghost" onclick={closeProfileImport} disabled={importApplying}>
              {t("Sulge")}
            </button>
          </div>
        </header>

        <div class="profile-body import-body">
          {#if importLoading}
            <div class="loading profile-loading">
              <div class="spinner"></div>
              <span>{t("Loen profiilifaili…")}</span>
            </div>
          {:else if importPreview}
            <div class="import-summary">
              <div class="import-stat">
                <strong>{importPreview.active_count}</strong>
                <span>{t("modi")}</span>
              </div>
              <div class="import-stat">
                <strong>{importPreview.installed_count}</strong>
                <span>{t("olemas")}</span>
              </div>
              <div class="import-stat" class:warn={importPreview.missing_count > 0}>
                <strong>{importPreview.missing_count}</strong>
                <span>{t("puudub")}</span>
              </div>
              <div class="import-stat">
                <strong>{importPreview.deactivated_count}</strong>
                <span>{t("deaktiveeritud")}</span>
              </div>
            </div>

            {#if importPreview.source_title}
              <p class="import-source">
                <span class="muted">{t("Kollektsioon: {title}", { title: importPreview.source_title })}</span>
                {#if importPreview.source_id}
                  <button
                    class="mini"
                    onclick={() => invoke("open_workshop", { id: importPreview.source_id })}
                  >{t("Ava Steamis")}</button>
                {/if}
              </p>
            {/if}

            <label class="import-field">
              <span>{t("Profiili nimi")}</span>
              <input
                type="text"
                bind:value={importName}
                oninput={() => (importReplace = false)}
              />
            </label>

            {#if importPreview.description}
              <p class="import-desc">{importPreview.description}</p>
            {/if}

            {#if importNameConflict}
              <label class="import-check">
                <input type="checkbox" bind:checked={importReplace} />
                {t("Selle nimega profiil on juba olemas — asenda see.")}
              </label>
            {/if}

            {#if importPreview.missing.length > 0}
              <section class="import-missing">
                <h3>
                  {t("Selles masinas puuduvad modid")}
                  <span class="badge warn">{importPreview.missing.length}</span>
                </h3>
                <div class="collection-list">
                  {#each importPreview.missing as item (item.id + (item.workshop_id ?? ""))}
                    <div class="collection-item">
                      <span>{item.name || item.id}</span>
                      <span class="mono muted">{item.workshop_id ?? item.id}</span>
                      {#if item.workshop_id}
                        <button
                          class="mini"
                          onclick={() => invoke("open_workshop", { id: item.workshop_id })}
                        >{t("Ava Steamis")}</button>
                      {/if}
                    </div>
                  {/each}
                </div>
                <p class="manager-note">
                  {t("Puuduvad modid jäävad profiili ootele. Pärast nende tellimist ja allalaadimist vajuta „Laadi uuesti”.")}
                </p>
              </section>
            {:else}
              <p class="import-ok">{t("Kõik profiili modid on selles masinas olemas.")}</p>
            {/if}

            {#if importError}
              <div class="banner error profile-banner">{importError}</div>
            {/if}
          {:else if importError}
            <div class="banner error profile-banner">{importError}</div>
          {/if}
        </div>

        <footer class="profile-footer">
          <button class="ghost" onclick={closeProfileImport} disabled={importApplying}>
            {t("Tühista")}
          </button>
          <button
            class="primary"
            onclick={applyImportedProfile}
            disabled={importApplying || importLoading || !importPreview || !importName.trim() || (importNameConflict && !importReplace)}
          >
            {importApplying ? t("Impordin…") : t("Impordi profiil")}
          </button>
        </footer>
      </div>
    </div>
  {/if}

  {#if profileCollectionOpen}
    <div
      class="config-backdrop"
      role="presentation"
      onclick={(event) => {
        if (event.target === event.currentTarget) closeProfileCollection();
      }}
      onkeydown={(event) => {
        if (event.key === "Escape") closeProfileCollection();
      }}
    >
      <div class="config-modal profile-modal" role="dialog" aria-modal="true" aria-label={t("Kollektsioon")}>
        <header class="config-head">
          <div>
            <h2>{t("Kollektsioon")}</h2>
            <p class="muted">
              {profileCollectionName}
              {#if profileCollectionName !== currentProfile}
                · {t("profiil pole praegu aktiivne")}
              {/if}
            </p>
          </div>
          <div class="config-head-actions">
            <button class="ghost" onclick={closeProfileCollection} disabled={profileCollectionSaving}>
              {t("Sulge")}
            </button>
          </div>
        </header>

        <div class="profile-body">
          {#if profileCollectionError}
            <div class="banner error profile-banner">{profileCollectionError}</div>
          {/if}

          {#if profileCollectionLoading}
            <div class="loading profile-loading">
              <div class="spinner"></div>
              <span>{t("Laen Steam Workshopi kollektsiooni…")}</span>
            </div>
          {:else if profileCollectionPreview}
            <div class="collection-summary">
              <div>
                <strong>{profileCollectionPreview.title}</strong>
                <p class="muted">
                  {t("{total} modi · {installed} olemas · {missing} puudub", {
                    total: profileCollectionPreview.total,
                    installed: profileCollectionPreview.installed,
                    missing: profileCollectionPreview.missing,
                  })}
                  · {t("{count} aktiivset", { count: profileCollectionActiveCount })}
                </p>
              </div>
              <button
                class="mini"
                onclick={() => invoke("open_workshop", { id: profileCollectionPreview.id })}
              >{t("Ava kollektsioon Steamis")}</button>
            </div>

            <div class="collection-actions">
              <button class="mini" onclick={selectAllProfileCollection}>
                {t("Vali kõik")}
              </button>
              <button class="mini" onclick={() => (profileCollectionSelected = new Set())}>
                {t("Tühista valik")}
              </button>
            </div>

            <div class="collection-columns">
              <section class="collection-section">
                <h3>
                  {t("Uued paigaldatud")}
                  <span class="muted">{profileCollectionNew.length}</span>
                </h3>
                <div class="collection-list">
                  {#each profileCollectionNew as item (item.workshop_id)}
                    <label class="collection-item">
                      <input
                        type="checkbox"
                        checked={profileCollectionSelected.has(item.workshop_id)}
                        onchange={() => toggleProfileCollectionItem(item.workshop_id)}
                      />
                      <span>{item.title || item.workshop_id}</span>
                    </label>
                  {/each}
                  {#if profileCollectionDeactivated.length > 0}
                    <div class="collection-subhead">
                      {t("Deaktiveeritud")}
                      <span class="badge warn">{profileCollectionDeactivated.length}</span>
                    </div>
                    {#each profileCollectionDeactivated as item (item.workshop_id)}
                      <div class="collection-item">
                        <span>{item.title || item.workshop_id}</span>
                        <span class="badge warn">{t("deaktiveeritud")}</span>
                        <button
                          class="mini"
                          onclick={() => restoreCollectionMod(item.mod_id)}
                          disabled={busy}
                        >{profileCollectionName === currentProfile
                          ? t("Lülita sisse")
                          : t("Taasta")}</button>
                      </div>
                    {/each}
                  {/if}
                  {#if profileCollectionNew.length === 0 && profileCollectionDeactivated.length === 0}
                    <div class="empty">
                      {profileCollectionName === currentProfile
                        ? t("Kõik paigaldatud kollektsiooni modid on profiilis ja aktiivsed.")
                        : t("Kõik paigaldatud kollektsiooni modid on profiilis.")}
                    </div>
                  {/if}
                </div>
              </section>

              <section class="collection-section">
                <h3>
                  {t("Puuduvad")}
                  <span class="muted">{profileCollectionPreview.missing}</span>
                </h3>
                <div class="collection-list">
                  {#each profileCollectionPreview.items.filter((item) => item.status === "missing") as item (item.workshop_id)}
                    <div class="collection-item">
                      <span>{item.title || item.workshop_id}</span>
                      <button
                        class="mini"
                        onclick={() => invoke("open_workshop", { id: item.workshop_id })}
                      >{t("Ava Steamis")}</button>
                    </div>
                  {:else}
                    <div class="empty">{t("Kõik kollektsiooni modid on olemas.")}</div>
                  {/each}
                </div>
              </section>
            </div>
            {#if profileCollectionDeactivated.length > 0}
              <p class="manager-note">
                {#if profileCollectionName === currentProfile}
                  {t("Deaktiveeritud modide „Lülita sisse” muudab ainult aktiivset nimekirja. Vajuta pärast sulgemist Salvesta, et see mängu configusse läheks.")}
                {:else}
                  {t("„Taasta” lisab modi profiili tagasi. Rakenda profiil, et see aktiivseks läheks.")}
                {/if}
              </p>
            {/if}
          {/if}
        </div>

        <footer class="profile-footer">
          <button class="ghost" onclick={closeProfileCollection} disabled={profileCollectionSaving}>
            {t("Tühista")}
          </button>
          <button
            class="primary"
            onclick={addProfileCollectionMods}
            disabled={profileCollectionSaving || profileCollectionSelected.size === 0}
          >
            {profileCollectionSaving
              ? t("Lisan…")
              : t("Lisa valitud ({count})", { count: profileCollectionSelected.size })}
          </button>
        </footer>
      </div>
    </div>
  {/if}

  {#if aboutOpen}
    <div
      class="config-backdrop"
      role="presentation"
      onclick={(event) => {
        if (event.target === event.currentTarget) aboutOpen = false;
      }}
      onkeydown={(event) => {
        if (event.key === "Escape") aboutOpen = false;
      }}
    >
      <div class="config-modal about-modal" role="dialog" aria-modal="true" aria-label={t("Info")}>
        <header class="config-head">
          <div>
            <h2>Resistance Mod Launcher</h2>
            <p class="muted">{appVersion} · GPL-3.0-only</p>
          </div>
          <div class="config-head-actions">
            <button class="ghost" onclick={() => (aboutOpen = false)}>{t("Sulge")}</button>
          </div>
        </header>
        <div class="about-body">
          <p>{t("Unofficial mod launcher for XCOM 2: War of the Chosen on Linux. Not affiliated with Firaxis, 2K, Take-Two, Valve, or the AML project.")}</p>
          <p>{t("See projekt on GPL-3.0-only litsentsi all. Täistekst on repos failis LICENSE.")}</p>
          <p class="muted">
            <a
              class="about-link"
              href="https://github.com/reiokr/resistance-mod-launcher"
              onclick={(event) => {
                event.preventDefault();
                invoke("open_url", {
                  url: "https://github.com/reiokr/resistance-mod-launcher",
                });
              }}
            >https://github.com/reiokr/resistance-mod-launcher</a>
          </p>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .shell {
    height: 100vh;
    display: flex;
    flex-direction: column;
    background: transparent;
  }

  .topbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 24px;
    padding: 16px 22px;
    border-bottom: 1px solid #1f2937;
    background: rgba(13, 17, 23, 0.86);
    backdrop-filter: blur(14px);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 14px;
    min-width: 0;
  }

  .logo {
    width: 46px;
    height: 46px;
    border-radius: 13px;
    display: grid;
    place-items: center;
    background: linear-gradient(135deg, #3ddc97, #2bb3ff);
    color: #071018;
    font-size: 18px;
    font-weight: 800;
    letter-spacing: -0.08em;
    box-shadow: 0 8px 24px rgba(43, 179, 255, 0.18);
  }

  h1 {
    margin: 0;
    font-size: 18px;
    line-height: 1.2;
    letter-spacing: -0.01em;
  }

  .brand p {
    margin: 3px 0 0;
    color: #8b98a9;
    font-size: 12px;
    white-space: nowrap;
  }

  .dot {
    margin: 0 3px;
    color: #3b4a5f;
  }

  .top-actions {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
    flex-shrink: 0;
    flex-wrap: wrap;
  }

  .mode-toggle {
    min-width: 70px;
    font-size: 11px;
  }

  .simple-switch {
    border-color: rgba(61, 220, 151, 0.35);
    color: #9fe8c4;
  }

  .simple-switch:hover {
    border-color: #3ddc97;
    background: rgba(61, 220, 151, 0.08);
  }

  .profile-picker {
    display: flex;
    align-items: center;
    gap: 5px;
    min-width: 0;
  }

  .profile-picker select {
    max-width: 240px;
    min-width: 150px;
    padding: 8px 10px;
    border-radius: 9px;
    border: 1px solid #263244;
    background: #0f151e;
    color: #cfdae8;
    outline: none;
  }

  .profile-picker select:focus {
    border-color: #2bb3ff;
  }

  .profile-dirty {
    color: #f0b429;
    font-size: 13px;
    line-height: 1;
    cursor: help;
  }

  .top-actions .lang-select {
    padding: 8px 10px;
    border-radius: 9px;
    border: 1px solid #263244;
    background: #0f151e;
    color: #cfdae8;
    outline: none;
    font: inherit;
    cursor: pointer;
  }

  .top-actions .lang-select:focus {
    border-color: #2bb3ff;
  }

  button {
    border: 1px solid #2a3648;
    border-radius: 9px;
    padding: 9px 14px;
    background: #151c26;
    color: #d7e0ea;
    transition: background 120ms ease, border-color 120ms ease, transform 120ms ease;
  }

  button:hover:not(:disabled) {
    background: #1b2431;
    border-color: #3a4b63;
  }

  button:active:not(:disabled) {
    transform: translateY(1px);
  }

  button.primary {
    background: rgba(61, 220, 151, 0.12);
    border-color: rgba(61, 220, 151, 0.45);
    color: #8ff0c5;
  }

  button.primary:hover:not(:disabled) {
    background: rgba(61, 220, 151, 0.2);
  }

  button.launch {
    background: linear-gradient(135deg, #2bb3ff, #3ddc97);
    border: none;
    color: #041017;
    font-weight: 700;
  }

  button.launch:hover:not(:disabled) {
    filter: brightness(1.08);
  }

  button.ghost {
    background: transparent;
  }

  .activity {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 7px 22px;
    border-bottom: 1px solid #1a2330;
    background: rgba(43, 179, 255, 0.04);
    color: #9fd7ff;
    font-size: 11.5px;
  }

  .activity-track {
    width: 180px;
    height: 4px;
    overflow: hidden;
    border-radius: 999px;
    background: #1a2636;
  }

  .activity-bar {
    width: 40%;
    height: 100%;
    border-radius: 999px;
    background: linear-gradient(90deg, #2bb3ff, #3ddc97);
    animation: activity 1.1s ease-in-out infinite;
  }

  @keyframes activity {
    0% { transform: translateX(-110%); }
    100% { transform: translateX(290%); }
  }

  .config-banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px 20px;
    flex-wrap: wrap;
    padding: 10px 22px;
    border-bottom: 1px solid rgba(240, 180, 41, 0.32);
    background: linear-gradient(90deg, rgba(240, 180, 41, 0.13), rgba(240, 180, 41, 0.04));
    box-shadow: inset 4px 0 0 #f0b429;
  }

  .config-banner-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    color: #c9b37c;
    font-size: 12px;
    line-height: 1.4;
  }

  .config-banner-text strong {
    color: #ffe1a1;
    font-size: 13px;
    font-weight: 600;
  }

  .config-banner-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  .config-save {
    position: relative;
  }

  .config-save.dirty {
    border-color: #f0b429;
    box-shadow: 0 0 0 1px rgba(240, 180, 41, 0.35);
  }

  .save-dot {
    position: absolute;
    top: -3px;
    right: -3px;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #f0b429;
    box-shadow: 0 0 0 2px #0d1117;
  }

  .editing-banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px 20px;
    flex-wrap: wrap;
    padding: 10px 22px;
    border-bottom: 1px solid rgba(43, 179, 255, 0.3);
    background: rgba(43, 179, 255, 0.07);
    box-shadow: inset 4px 0 0 #2bb3ff;
  }

  .editing-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    color: #9fd7ff;
    font-size: 12px;
  }

  .editing-text strong {
    color: #cfeaff;
    font-size: 13px;
  }

  .editing-diff {
    color: #f3d28a;
    font-size: 11.5px;
    font-variant-numeric: tabular-nums;
  }

  .editing-actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }

  .banner {
    margin: 10px 22px 0;
    padding: 9px 13px;
    border-radius: 9px;
    font-size: 12.5px;
    border: 1px solid transparent;
    line-height: 1.45;
  }

  .banner.error {
    background: rgba(255, 107, 107, 0.1);
    border-color: rgba(255, 107, 107, 0.35);
    color: #ffb3b3;
  }

  .banner.info {
    background: rgba(43, 179, 255, 0.09);
    border-color: rgba(43, 179, 255, 0.3);
    color: #a9dcff;
  }

  .banner.warn {
    background: rgba(240, 180, 41, 0.08);
    border-color: rgba(240, 180, 41, 0.3);
    color: #f3d28a;
  }

  .loading {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 16px;
    color: #aab7c7;
  }

  .loading strong {
    color: #e6edf3;
  }

  .loading p {
    margin: 4px 0 0;
    font-size: 12px;
    color: #77869a;
  }

  .spinner {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    border: 3px solid #223044;
    border-top-color: #3ddc97;
    animation: spin 800ms linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .summary {
    display: flex;
    align-items: stretch;
    flex-wrap: wrap;
    gap: 6px;
    padding: 8px 22px;
    border-bottom: 1px solid #1a2330;
    background: rgba(11, 15, 20, 0.55);
  }

  .stat {
    position: relative;
    min-width: 84px;
    padding: 6px 11px 7px;
    border: 1px solid #202b3a;
    border-radius: 9px;
    background: rgba(21, 28, 38, 0.7);
    display: flex;
    flex-direction: column;
    gap: 1px;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
    appearance: none;
    transition: border-color 120ms ease, background 120ms ease;
  }

  .stat:hover {
    border-color: #2bb3ff;
    background: rgba(43, 179, 255, 0.08);
  }

  .stat:focus-visible {
    outline: 2px solid #2bb3ff;
    outline-offset: 1px;
  }

  .stat.selected {
    border-color: rgba(43, 179, 255, 0.75);
    background: rgba(43, 179, 255, 0.12);
    box-shadow: inset 0 0 0 1px rgba(43, 179, 255, 0.18);
  }

  .stat.selected::after {
    content: "";
    position: absolute;
    left: 10px;
    right: 10px;
    bottom: 3px;
    height: 2px;
    border-radius: 2px;
    background: #2bb3ff;
  }

  .stat strong {
    font-size: 16px;
    font-variant-numeric: tabular-nums;
    letter-spacing: -0.01em;
  }

  .stat span {
    font-size: 10.5px;
    text-transform: none;
    letter-spacing: 0.02em;
    color: #728196;
  }

  .stat.warn strong {
    color: #ff8f8f;
  }

  .stat.args {
    min-width: 0;
    flex: 1;
    cursor: default;
  }

  .stat.args:hover {
    border-color: #202b3a;
    background: rgba(21, 28, 38, 0.7);
  }

  .stat.args strong {
    font-family: "JetBrains Mono", "DejaVu Sans Mono", monospace;
    font-size: 11.5px;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .chips {
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 4px;
    margin-left: auto;
  }

  .chip {
    padding: 3px 8px;
    border-radius: 999px;
    font-size: 10.5px;
    background: rgba(255, 107, 107, 0.1);
    color: #ff9f9f;
    border: 1px solid rgba(255, 107, 107, 0.24);
    text-align: center;
    white-space: nowrap;
  }

  .chip.ok {
    background: rgba(61, 220, 151, 0.08);
    color: #7fe0b5;
    border-color: rgba(61, 220, 151, 0.2);
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 10px 22px;
    border-bottom: 1px solid #1a2330;
    flex-wrap: wrap;
  }

  .tabs {
    display: flex;
    gap: 4px;
    padding: 3px;
    border-radius: 10px;
    background: #10161f;
    border: 1px solid #1e2938;
  }

  .tabs button {
    border: none;
    background: transparent;
    padding: 7px 12px;
    border-radius: 8px;
    font-size: 12.5px;
    color: #8d9bb0;
  }

  .tabs button.active {
    background: #1c2635;
    color: #e6edf3;
    box-shadow: inset 0 0 0 1px #2a3648;
  }

  .tab-count {
    display: inline-block;
    margin-left: 6px;
    padding: 1px 6px;
    border-radius: 999px;
    font-size: 10.5px;
    background: rgba(43, 179, 255, 0.16);
    color: #8fd4ff;
  }

  .filters {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    min-width: 320px;
  }

  input.search {
    flex: 1;
    min-width: 180px;
    padding: 8px 12px;
    border-radius: 9px;
    border: 1px solid #263244;
    background: #0f151e;
    color: #e6edf3;
    outline: none;
  }

  input.search:focus {
    border-color: #2bb3ff;
    box-shadow: 0 0 0 3px rgba(43, 179, 255, 0.12);
  }

  select {
    padding: 8px 10px;
    border-radius: 9px;
    border: 1px solid #263244;
    background: #0f151e;
    color: #cfdae8;
    outline: none;
  }

  select:focus {
    border-color: #2bb3ff;
  }

  .result-count {
    font-size: 11.5px;
    color: #7d8ca0;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .content {
    flex: 1;
    min-height: 0;
    display: flex;
  }

  .table {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .thead,
  .tr {
    display: grid;
    grid-template-columns: 40px minmax(210px, 2.4fr) minmax(110px, 1.1fr) 150px 100px 80px 82px;
    align-items: center;
  }

  .thead {
    padding: 0 14px;
    height: 34px;
    border-bottom: 1px solid #1c2635;
    background: #0f151e;
    color: #748398;
    font-size: 10.5px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    position: sticky;
    top: 0;
    z-index: 2;
  }

  .tbody {
    overflow: auto;
    flex: 1;
  }

  .tr {
    padding: 0 14px;
    min-height: 34px;
    border-bottom: 1px solid rgba(31, 41, 55, 0.55);
    cursor: pointer;
    color: #c9d4e1;
    content-visibility: auto;
    contain-intrinsic-size: 34px;
  }

  .tr:hover {
    background: rgba(43, 179, 255, 0.045);
  }

  .tr.selected {
    background: rgba(43, 179, 255, 0.1);
    box-shadow: inset 2px 0 0 #2bb3ff;
  }

  .cell {
    padding: 6px 8px;
    font-size: 12.5px;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .cell.check {
    padding-left: 0;
    display: flex;
    align-items: center;
  }

  input[type="checkbox"] {
    width: 15px;
    height: 15px;
    accent-color: #3ddc97;
    cursor: pointer;
  }

  .name-text {
    font-weight: 550;
  }

  .muted {
    color: #7d8ca0;
  }

  .mono {
    font-family: "JetBrains Mono", "DejaVu Sans Mono", monospace;
    font-size: 11px;
    color: #8fa1b8;
    font-variant-numeric: tabular-nums;
  }

  .badge {
    display: inline-block;
    padding: 1px 6px;
    border-radius: 999px;
    font-size: 10px;
    background: #1a2330;
    color: #8b98a9;
    border: 1px solid #263244;
    vertical-align: 1px;
  }

  .badge.active {
    background: rgba(61, 220, 151, 0.1);
    color: #7fe0b5;
    border-color: rgba(61, 220, 151, 0.25);
  }

  .badge.danger {
    margin-left: 6px;
    background: rgba(255, 107, 107, 0.1);
    color: #ff9f9f;
    border-color: rgba(255, 107, 107, 0.3);
  }

  .badge.update {
    margin-left: 5px;
    background: rgba(240, 180, 41, 0.12);
    color: #f3d28a;
    border-color: rgba(240, 180, 41, 0.35);
  }

  .badge.warn {
    margin-left: 6px;
    background: rgba(240, 180, 41, 0.12);
    color: #f3d28a;
    border-color: rgba(240, 180, 41, 0.35);
  }

  .badge.manual {
    margin-left: 6px;
    background: rgba(61, 220, 151, 0.1);
    color: #7fe0b5;
    border-color: rgba(61, 220, 151, 0.3);
  }

  .tab-count.has-issues {
    background: rgba(240, 180, 41, 0.18);
    color: #f3d28a;
  }

  .tag-list {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin: 10px 0 2px;
  }

  .tag {
    padding: 2px 7px;
    border-radius: 999px;
    font-size: 10px;
    background: rgba(43, 179, 255, 0.09);
    border: 1px solid rgba(43, 179, 255, 0.22);
    color: #9fd7ff;
  }

  .description-text {
    margin-top: 8px;
    padding: 10px;
    max-height: 260px;
    overflow: auto;
    border: 1px solid #243144;
    border-radius: 9px;
    background: #0d131b;
    color: #b9c7d8;
    font-size: 12px;
    line-height: 1.5;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .open-workshop {
    margin-top: 12px;
    width: 100%;
  }

  .empty {
    padding: 28px;
    text-align: center;
    color: #728196;
    font-size: 12.5px;
  }

  .simple-hint {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px 20px;
    flex-wrap: wrap;
    margin: 14px 0 0;
    padding: 12px 14px;
    border: 1px solid rgba(61, 220, 151, 0.28);
    border-radius: 11px;
    background: linear-gradient(135deg, rgba(61, 220, 151, 0.08), rgba(43, 179, 255, 0.05));
    box-shadow: inset 4px 0 0 #3ddc97;
  }

  .simple-hint-text {
    display: flex;
    flex-direction: column;
    gap: 3px;
    color: #9fb3c8;
    font-size: 12px;
  }

  .simple-hint-text strong {
    color: #d9f7e9;
    font-size: 13px;
  }

  .simple-hint-actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }

  .manager {
    flex: 1;
    min-width: 0;
    overflow: auto;
    padding: 18px 24px 28px;
  }

  .manager-title {
    display: flex;
    align-items: baseline;
    gap: 12px;
  }

  .manager-title h2 {
    margin: 0;
    font-size: 16px;
  }

  .manager-actions {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .manager-add {
    display: flex;
    gap: 8px;
    margin: 14px 0;
    max-width: 720px;
  }

  .manager-add input {
    flex: 1;
    padding: 8px 12px;
    border-radius: 9px;
    border: 1px solid #263244;
    background: #0f151e;
    color: #e6edf3;
    outline: none;
  }

  .manager-add input:focus {
    border-color: #2bb3ff;
    box-shadow: 0 0 0 3px rgba(43, 179, 255, 0.12);
  }

  .manager-list {
    max-width: 860px;
    border: 1px solid #202b3a;
    border-radius: 11px;
    overflow: hidden;
    background: rgba(15, 21, 30, 0.6);
  }

  .manager-row {
    display: grid;
    grid-template-columns: minmax(160px, 1fr) 110px auto auto;
    gap: 8px;
    align-items: center;
    min-height: 40px;
    padding: 5px 12px;
    border-bottom: 1px solid rgba(31, 41, 55, 0.6);
    font-size: 12.5px;
  }

  .manager-row:last-child {
    border-bottom: none;
  }

  .manager-row input {
    grid-column: 1 / 2;
    padding: 6px 9px;
    border-radius: 7px;
    border: 1px solid #2a3648;
    background: #0b1118;
    color: #e6edf3;
  }

  .manager-note {
    max-width: 820px;
    margin: 14px 0 0;
    color: #728196;
    font-size: 11.5px;
    line-height: 1.5;
  }

  .manager-run {
    margin-left: auto;
  }

  .profile-list {
    max-width: 860px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .profile-card {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 14px;
    padding: 12px 14px;
    border: 1px solid #202b3a;
    border-radius: 11px;
    background: rgba(15, 21, 30, 0.6);
  }

  .profile-card.with-cover {
    flex-wrap: wrap;
  }

  .profile-card.with-cover .profile-actions {
    flex: 1 1 100%;
    justify-content: flex-end;
  }

  .profile-thumb {
    width: 72px;
    height: 72px;
    flex-shrink: 0;
    border: 1px solid #263244;
    border-radius: 9px;
    object-fit: cover;
    background: #0f151e;
  }

  .profile-card-main {
    flex: 1;
    min-width: 0;
  }

  .profile-card-title {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .profile-card-meta {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-top: 4px;
    color: #8d9bb0;
    font-size: 11.5px;
  }

  .source-badge {
    margin-left: 6px;
    padding: 1px 6px;
    border: 1px solid rgba(61, 220, 151, 0.35);
    border-radius: 999px;
    color: #3ddc97;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .profile-source-note {
    margin: 8px 0 0;
    color: #8d9bb0;
    font-size: 11.5px;
  }

  .profile-empty-hint {
    margin: 7px 0 0;
    color: #f3d28a;
    font-size: 11.5px;
  }

  .profile-desc {
    margin: 6px 0 0;
    color: #8d9bb0;
    font-size: 11.5px;
    line-height: 1.45;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
    white-space: pre-wrap;
  }

  .profile-parent {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    margin-top: 6px;
    color: #728196;
    font-size: 11px;
  }

  .profile-parent .mini {
    font-size: 10.5px;
  }

  .profile-menu {
    position: relative;
    flex-shrink: 0;
  }

  .profile-menu > summary {
    list-style: none;
    min-width: 30px;
    text-align: center;
    cursor: pointer;
  }

  .profile-menu > summary::-webkit-details-marker {
    display: none;
  }

  .profile-menu[open] > summary {
    border-color: #2bb3ff;
    color: #cfeaff;
  }

  .profile-menu-items {
    position: absolute;
    top: calc(100% + 4px);
    right: 0;
    z-index: 20;
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 190px;
    padding: 6px;
    border: 1px solid #263244;
    border-radius: 9px;
    background: #121a24;
    box-shadow: 0 12px 30px rgba(0, 0, 0, 0.45);
  }

  .profile-menu-items .mini {
    width: 100%;
    text-align: left;
    white-space: nowrap;
  }

  .history-modal {
    width: min(1100px, 100%);
    height: min(820px, 100%);
  }

  .history-body {
    flex: 1;
    min-height: 0;
    display: flex;
  }

  .history-list {
    width: 270px;
    flex-shrink: 0;
    padding: 10px;
    border-right: 1px solid #1c2635;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .history-current {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-bottom: 6px;
    padding: 8px 10px;
    border: 1px solid rgba(61, 220, 151, 0.3);
    border-radius: 8px;
    background: rgba(61, 220, 151, 0.06);
  }

  .history-current strong {
    color: #8ff0c5;
    font-size: 12px;
  }

  .history-current span {
    color: #728196;
    font-size: 10.5px;
  }

  .history-entry {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 8px 10px;
    border: 1px solid transparent;
    border-radius: 8px;
    background: transparent;
    color: #b9c7d8;
    text-align: left;
    cursor: pointer;
  }

  .history-entry:hover {
    background: #151c26;
    border-color: #263244;
  }

  .history-entry.active {
    background: #1c2635;
    border-color: #2bb3ff;
  }

  .history-entry strong {
    color: #d7e0ea;
    font-size: 11.5px;
  }

  .history-entry span {
    color: #728196;
    font-size: 10.5px;
  }

  .history-detail {
    flex: 1;
    min-width: 0;
    padding: 14px 18px;
    overflow: auto;
  }

  .history-detail-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
    margin-bottom: 10px;
  }

  .history-detail-head strong {
    display: block;
    font-size: 13px;
  }

  .history-detail-head .muted {
    font-size: 11px;
  }

  .history-counts {
    display: flex;
    gap: 8px;
    font-size: 11px;
    font-variant-numeric: tabular-nums;
  }

  .history-counts .added {
    color: #3ddc97;
  }

  .history-counts .removed {
    color: #ff6b6b;
  }

  .history-counts .reordered {
    color: #f3d28a;
  }

  .history-detail h3 {
    display: flex;
    align-items: baseline;
    gap: 7px;
    margin: 14px 0 5px;
    color: #cfdae8;
    font-size: 12px;
  }

  .history-changes {
    max-height: 220px;
    margin: 0;
    padding: 6px 9px;
    border: 1px solid #1c2635;
    border-radius: 8px;
    background: #0b1017;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 2px;
    overflow: auto;
    color: #9fb3c8;
    font-size: 11px;
  }

  .history-changes li {
    overflow-wrap: anywhere;
  }

  .history-actions {
    display: flex;
    gap: 8px;
    margin-top: 18px;
  }

  .profile-actions {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    flex-wrap: wrap;
    flex-shrink: 0;
  }

  .profile-modal {
    width: min(1000px, 100%);
    height: min(880px, 100%);
  }

  .profile-body {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 14px 18px;
  }

  .profile-fields {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }

  .profile-fields label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    color: #9fb0c6;
    font-size: 11.5px;
  }

  .profile-fields input,
  .profile-fields textarea,
  .collection-input input {
    width: 100%;
    box-sizing: border-box;
    padding: 8px 10px;
    border: 1px solid #263244;
    border-radius: 9px;
    background: #0f151e;
    color: #e6edf3;
    outline: none;
    font: inherit;
    resize: vertical;
  }

  .profile-fields input:focus,
  .profile-fields textarea:focus,
  .collection-input input:focus {
    border-color: #2bb3ff;
    box-shadow: 0 0 0 3px rgba(43, 179, 255, 0.12);
  }

  .profile-source {
    display: flex;
    align-items: center;
    gap: 18px;
    flex-wrap: wrap;
    margin: 14px 0 10px;
  }

  .profile-radio {
    display: flex;
    align-items: center;
    gap: 6px;
    color: #b9c7d8;
    font-size: 12px;
  }

  .collection-input {
    display: flex;
    gap: 8px;
    margin: 10px 0;
  }

  .collection-input input {
    flex: 1;
  }

  .collection-summary {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 14px;
    padding: 10px 12px;
    border: 1px solid #243144;
    border-radius: 9px;
    background: #0d131b;
  }

  .collection-summary p {
    margin: 3px 0 0;
    color: #8d9bb0;
    font-size: 11.5px;
  }

  .collection-desc {
    max-height: 110px;
    overflow: auto;
    margin: 10px 0;
    padding: 8px 10px;
    border: 1px solid #202b3a;
    border-radius: 9px;
    color: #9fb0c6;
    font-size: 11.5px;
    line-height: 1.5;
    white-space: pre-wrap;
  }

  .collection-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    margin: 10px 0;
  }

  .collection-columns {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }

  .collection-section h3 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 0 6px;
    font-size: 12px;
  }

  .collection-list {
    max-height: 300px;
    overflow: auto;
    border: 1px solid #202b3a;
    border-radius: 9px;
    background: rgba(15, 21, 30, 0.6);
  }

  .collection-subhead {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border-top: 1px solid #2a3648;
    border-bottom: 1px solid rgba(31, 41, 55, 0.6);
    background: rgba(240, 180, 41, 0.06);
    color: #d9b76a;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.07em;
  }

  .collection-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    min-height: 27px;
    padding: 4px 8px;
    border-bottom: 1px solid rgba(31, 41, 55, 0.6);
    font-size: 11.5px;
    color: #b9c7d8;
  }

  .collection-item:last-child {
    border-bottom: none;
  }

  .collection-item span:first-child {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-align: left;
  }

  .collection-item input[type="checkbox"] {
    flex-shrink: 0;
  }

  .profile-banner {
    margin: 8px 0;
  }

  .profile-loading {
    padding: 22px;
  }

  .import-modal {
    width: min(760px, 100%);
    height: auto;
    max-height: min(820px, 100%);
  }

  .import-body {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .import-summary {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 8px;
  }

  .import-stat {
    padding: 8px 10px;
    border: 1px solid #202b3a;
    border-radius: 9px;
    background: rgba(15, 21, 30, 0.6);
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .import-stat strong {
    font-size: 18px;
    font-variant-numeric: tabular-nums;
  }

  .import-stat span {
    font-size: 10.5px;
    color: #728196;
  }

  .import-stat.warn {
    border-color: rgba(240, 180, 41, 0.3);
  }

  .import-stat.warn strong {
    color: #f3d28a;
  }

  .import-source {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    margin: 0;
    font-size: 12px;
  }

  .import-field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    color: #9fb0c6;
    font-size: 11.5px;
  }

  .import-field input {
    box-sizing: border-box;
    padding: 8px 10px;
    border: 1px solid #263244;
    border-radius: 9px;
    background: #0f151e;
    color: #e6edf3;
    outline: none;
    font: inherit;
  }

  .import-field input:focus {
    border-color: #2bb3ff;
    box-shadow: 0 0 0 3px rgba(43, 179, 255, 0.12);
  }

  .import-desc {
    margin: 0;
    max-height: 100px;
    overflow: auto;
    padding: 8px 10px;
    border: 1px solid #202b3a;
    border-radius: 9px;
    background: rgba(15, 21, 30, 0.45);
    color: #aab7c7;
    font-size: 11.5px;
    white-space: pre-wrap;
  }

  .import-check {
    display: flex;
    align-items: center;
    gap: 8px;
    color: #f3d28a;
    font-size: 12px;
  }

  .import-missing h3 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 0 7px;
    color: #9fb0c6;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .import-ok {
    margin: 0;
    color: #8ee0b3;
    font-size: 12px;
  }

  .delete-modal {
    width: min(700px, 100%);
    height: auto;
    max-height: min(760px, 100%);
  }

  .delete-body {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .delete-stats {
    margin: 0;
  }

  .delete-warning {
    margin: 0;
    padding: 9px 11px;
    border: 1px solid rgba(240, 180, 41, 0.3);
    border-radius: 9px;
    background: rgba(240, 180, 41, 0.06);
    color: #f3d28a;
    font-size: 12px;
  }

  button.delete-confirm {
    background: #b83b3b;
    border-color: #c74c4c;
  }

  button.delete-confirm:hover:not(:disabled) {
    background: #c94b4b;
  }

  .cleanup-panel {
    max-width: 860px;
    margin-top: 26px;
  }

  .cleanup-stats {
    margin: 10px 0 6px;
    max-width: none;
  }

  .cleanup-list {
    max-height: 360px;
    overflow: auto;
    border: 1px solid #202b3a;
    border-radius: 11px;
    background: rgba(15, 21, 30, 0.6);
  }

  .cleanup-row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 9px 13px;
    border-bottom: 1px solid rgba(31, 41, 55, 0.6);
  }

  .cleanup-row:last-child {
    border-bottom: none;
  }

  .cleanup-name {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .cleanup-name strong {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: #d7e1ec;
    font-size: 12.5px;
    font-weight: 600;
  }

  .cleanup-name .mono {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 10.5px;
  }

  .profile-footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 12px 18px;
    border-top: 1px solid #1c2635;
  }

  .auto-card {
    max-width: 860px;
    margin: 14px 0 18px;
    padding: 12px 14px;
    border: 1px solid #202b3a;
    border-radius: 11px;
    background: rgba(43, 179, 255, 0.04);
  }

  .auto-head {
    display: flex;
    align-items: flex-start;
    gap: 14px;
    flex-wrap: wrap;
  }

  .auto-head > div:first-child {
    flex: 1;
    min-width: 240px;
  }

  .auto-head p {
    margin: 3px 0 0;
    color: #728196;
    font-size: 11.5px;
    line-height: 1.45;
  }

  .auto-check {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11.5px;
    color: #9fb0c6;
  }

  .auto-summary {
    display: flex;
    flex-wrap: wrap;
    gap: 16px;
    margin-top: 12px;
    font-size: 11.5px;
    color: #9fb0c6;
  }

  .auto-empty {
    margin: 10px 0 0;
    color: #728196;
    font-size: 11.5px;
  }

  .auto-groups {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
    gap: 6px;
    margin-top: 10px;
  }

  .auto-group {
    display: flex;
    align-items: baseline;
    gap: 7px;
    min-width: 0;
    padding: 5px 7px;
    border-radius: 7px;
    background: #0d131b;
    font-size: 11px;
  }

  .auto-group .badge {
    flex-shrink: 0;
  }

  .auto-group strong {
    white-space: nowrap;
  }

  .auto-group .muted {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .auto-actions {
    display: flex;
    gap: 8px;
    margin-top: 12px;
  }

  .diag-stats {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: 10px;
    max-width: 860px;
    margin: 16px 0 6px;
  }

  .diag-stat {
    padding: 12px 14px;
    border-radius: 11px;
    border: 1px solid #202b3a;
    background: rgba(15, 21, 30, 0.6);
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .diag-stat strong {
    font-size: 22px;
    font-variant-numeric: tabular-nums;
  }

  .diag-stat span {
    font-size: 11.5px;
    color: #8d9bb0;
  }

  .diag-stat em {
    font-style: normal;
    font-size: 10.5px;
    color: #6f7f94;
  }

  .diag-stat.warn {
    border-color: rgba(240, 180, 41, 0.28);
  }

  .diag-stat.warn strong {
    color: #f3d28a;
  }

  .diag-heading {
    scroll-margin-top: 14px;
    margin: 20px 0 8px;
    font-size: 12.5px;
    color: #9fb0c6;
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .diag-list {
    max-width: 860px;
    border: 1px solid #202b3a;
    border-radius: 11px;
    overflow: hidden;
    background: rgba(15, 21, 30, 0.6);
  }

  .diag-item {
    border-bottom: 1px solid rgba(31, 41, 55, 0.6);
  }

  .diag-item:last-child {
    border-bottom: none;
  }

  .unknown-row {
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    gap: 4px 10px;
    padding: 9px 13px;
    border-bottom: 1px solid rgba(31, 41, 55, 0.6);
    font-size: 12px;
  }

  .unknown-row:last-child {
    border-bottom: none;
  }

  .unknown-row .mono {
    color: #f3d28a;
    font-size: 12.5px;
    word-break: break-all;
  }

  .diag-item summary {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 9px 13px;
    cursor: pointer;
    font-size: 12.5px;
    list-style: none;
  }

  .diag-item summary::-webkit-details-marker {
    display: none;
  }

  .diag-item summary::before {
    content: "▸";
    color: #526277;
    font-size: 10px;
  }

  .diag-item[open] summary::before {
    content: "▾";
  }

  .diag-item summary:hover {
    background: rgba(43, 179, 255, 0.045);
  }

  .diag-body {
    padding: 2px 13px 10px 28px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .dep-row,
  .override-row {
    display: flex;
    align-items: center;
    gap: 9px;
    min-height: 26px;
    font-size: 11.5px;
    color: #b9c7d8;
  }

  .dep-row {
    flex-wrap: wrap;
  }

  .override-row .mono {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .detail-issue {
    margin-top: 10px;
    padding: 9px 10px;
    border-radius: 9px;
    border: 1px solid rgba(240, 180, 41, 0.25);
    background: rgba(240, 180, 41, 0.05);
    font-size: 12px;
  }

  .detail-issue strong {
    display: block;
    margin-bottom: 4px;
    color: #f3d28a;
  }

  .detail-issue .dep-row {
    flex-wrap: wrap;
    color: #b9c7d8;
  }

  .order {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .order-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 22px;
    border-bottom: 1px solid #1c2635;
    background: #0f151e;
    font-size: 12.5px;
  }

  .order-list {
    overflow: auto;
    flex: 1;
    padding: 6px 10px 16px;
  }

  .order-row {
    display: grid;
    grid-template-columns: 26px 42px minmax(220px, 2fr) minmax(120px, 1fr) 120px auto;
    align-items: center;
    gap: 4px;
    padding: 5px 12px;
    min-height: 36px;
    border-bottom: 1px solid rgba(31, 41, 55, 0.5);
    border-radius: 7px;
    font-size: 12.5px;
  }

  .order-row:hover {
    background: rgba(43, 179, 255, 0.045);
  }

  .order-row.dragging {
    opacity: 0.45;
  }

  .handle {
    color: #526277;
    cursor: grab;
    letter-spacing: -2px;
    user-select: none;
  }

  .pos {
    color: #728196;
    font-variant-numeric: tabular-nums;
    text-align: right;
    padding-right: 6px;
  }

  .order-row .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .order-actions {
    display: flex;
    gap: 4px;
    justify-content: flex-end;
  }

  button.mini {
    padding: 3px 8px;
    border-radius: 6px;
    font-size: 12px;
    line-height: 1.2;
    background: #151c26;
  }

  button.mini.danger {
    color: #ff9f9f;
    border-color: rgba(255, 107, 107, 0.3);
  }

  .details-resizer {
    flex-shrink: 0;
    width: 8px;
    cursor: col-resize;
    position: relative;
    background: transparent;
    outline: none;
  }

  .details-resizer::after {
    content: "";
    position: absolute;
    left: 3px;
    top: 50%;
    width: 2px;
    height: 48px;
    border-radius: 2px;
    background: #2a3648;
    transform: translateY(-50%);
    transition: background 120ms ease;
  }

  .details-resizer:hover::after,
  .details-resizer:focus-visible::after,
  .details-resizer.active::after {
    background: #2bb3ff;
  }

  .details {
    width: 340px;
    flex-shrink: 0;
    border-left: 1px solid #1f2937;
    background: rgba(13, 17, 23, 0.78);
    padding: 18px;
    overflow: auto;
    position: relative;
  }

  .details h2 {
    margin: 0 26px 4px 0;
    font-size: 15px;
    line-height: 1.3;
  }

  .details h3 {
    margin: 18px 0 0;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: #728196;
  }

  .details .close {
    position: absolute;
    top: 10px;
    right: 10px;
    padding: 2px 8px;
    font-size: 16px;
    background: transparent;
    border: none;
    color: #7d8ca0;
  }

  .details dl {
    display: grid;
    grid-template-columns: 86px 1fr;
    gap: 7px 10px;
    margin: 16px 0 0;
    font-size: 12px;
  }

  .details dt {
    color: #728196;
  }

  .details dd {
    margin: 0;
    color: #c9d4e1;
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .details .path {
    font-family: "JetBrains Mono", "DejaVu Sans Mono", monospace;
    font-size: 10.5px;
    color: #8fa1b8;
  }

  .status {
    display: flex;
    gap: 18px;
    padding: 6px 14px;
    height: 28px;
    align-items: center;
    border-top: 1px solid #1a2330;
    background: #0d1219;
    color: #64748b;
    font-size: 10.5px;
    font-family: "JetBrains Mono", "DejaVu Sans Mono", monospace;
    white-space: nowrap;
    overflow: hidden;
  }

  .status span {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .status-info {
    margin-left: auto;
    flex-shrink: 0;
    min-width: 44px;
    padding: 1px 7px;
    text-align: center;
    white-space: nowrap;
    border: 1px solid #263244;
    border-radius: 999px;
    background: transparent;
    color: #8fa1b8;
    font-size: 10px;
    cursor: pointer;
  }

  .status-info:hover {
    color: #e6edf3;
    border-color: #2bb3ff;
  }

  .config-backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: flex;
    padding: 30px;
    background: rgba(4, 8, 13, 0.72);
    backdrop-filter: blur(3px);
  }

  .config-modal {
    margin: auto;
    width: min(1180px, 100%);
    height: min(820px, 100%);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    border: 1px solid #2a3648;
    border-radius: 14px;
    background: #0d131b;
    box-shadow: 0 24px 80px rgba(0, 0, 0, 0.55);
  }

  .config-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 12px 18px;
    border-bottom: 1px solid #1c2635;
  }

  .config-head h2 {
    margin: 0;
    font-size: 15px;
  }

  .config-head p {
    margin: 3px 0 0;
    font-size: 11.5px;
    overflow-wrap: anywhere;
  }

  .config-head-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  .config-layout {
    flex: 1;
    min-height: 0;
    display: flex;
  }

  .config-files {
    width: 280px;
    flex-shrink: 0;
    overflow: auto;
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    border-right: 1px solid #1c2635;
    background: #0b1118;
  }

  .config-files-title {
    display: flex;
    justify-content: space-between;
    padding: 4px 6px 8px;
    color: #7d8ca0;
    font-size: 10.5px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .config-file {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    padding: 6px 8px;
    border: 0;
    border-radius: 7px;
    background: transparent;
    color: #b9c7d8;
    font-size: 11.5px;
    text-align: left;
    cursor: pointer;
  }

  .config-file span:first-child {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .config-file:hover {
    background: rgba(43, 179, 255, 0.06);
  }

  .config-file.active {
    background: #1c2635;
    color: #e6edf3;
  }

  .config-main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 12px 12px;
  }

  .config-meta {
    display: flex;
    align-items: center;
    gap: 14px;
    min-height: 15px;
    color: #7d8ca0;
    font-size: 10.5px;
  }

  .config-meta-right {
    margin-left: auto;
  }

  .config-banner {
    margin: 0;
  }

  .config-loading {
    flex: 1;
  }

  .config-text {
    flex: 1;
    width: 100%;
    min-height: 0;
    resize: none;
    padding: 12px 14px;
    border: 1px solid #243144;
    border-radius: 9px;
    outline: none;
    background: #080d13;
    color: #cfdae8;
    font-family: "JetBrains Mono", "DejaVu Sans Mono", ui-monospace, monospace;
    font-size: 12px;
    line-height: 1.55;
    tab-size: 2;
    white-space: pre;
    overflow: auto;
  }

  .config-text:focus {
    border-color: #2bb3ff;
    box-shadow: 0 0 0 3px rgba(43, 179, 255, 0.1);
  }

  .readme-modal {
    width: min(900px, 100%);
    height: min(760px, 100%);
  }

  .readme-body {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 14px 18px 20px;
  }

  .readme-loading {
    height: 100%;
  }

  .readme-text {
    margin: 0;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    color: #cfdae8;
    font-family: "Inter", "Noto Sans", system-ui, sans-serif;
    font-size: 13px;
    line-height: 1.6;
  }

  .about-modal {
    width: min(620px, 100%);
    height: auto;
    max-height: min(620px, 100%);
  }

  .about-body {
    padding: 16px 20px 22px;
    overflow: auto;
    color: #b9c7d8;
    font-size: 12.5px;
    line-height: 1.6;
  }

  .about-link {
    color: #2bb3ff;
    text-decoration: none;
    word-break: break-all;
  }

  .about-link:hover {
    text-decoration: underline;
  }

  .about-body p {
    margin: 0 0 10px;
  }

  .about-body p:last-child {
    margin-bottom: 0;
  }

  .about-body .muted {
    font-family: "JetBrains Mono", "DejaVu Sans Mono", monospace;
    font-size: 11px;
    overflow-wrap: anywhere;
  }

  @media (max-width: 1180px) {
    .thead,
    .tr {
      grid-template-columns: 38px minmax(190px, 2.2fr) minmax(100px, 1fr) 140px 84px 76px;
    }

    .thead .cell:nth-child(5),
    .tr .cell:nth-child(5) {
      display: none;
    }

    .details {
      width: 300px;
    }

    .stat.args,
    .chips {
      display: none;
    }
  }
</style>
