<script>
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { getVersion } from "@tauri-apps/api/app";
  import { translate } from "./i18n.js";

  let snapshot = $state(null);
  let loading = $state(true);
  let busy = $state(false);
  let busyText = $state("");
  let error = $state("");
  let message = $state("");
  let query = $state("");
  let stateFilter = $state("all");
  let categoryFilter = $state("all");
  let tagFilter = $state("all");
  let tab = $state("mods");
  let lang = $state(localStorage.getItem("resistance-lang") || "et");
  const t = (text, vars) => translate(lang, text, vars);
  function toggleLanguage() {
    lang = lang === "et" ? "en" : "et";
    localStorage.setItem("resistance-lang", lang);
    document.documentElement.lang = lang;
  }
  let selectedId = $state(null);
  let details = $state(null);
  let activeOrder = $state([]);
  let dragId = $state(null);
  let newCategory = $state("");
  let editingCategory = $state(null);
  let editingName = $state("");
  let profileName = $state("");
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
  let appVersion = $state("0.1.0");

  onMount(() => {
    document.documentElement.lang = lang;
    getVersion().then((version) => (appVersion = version)).catch(() => {});
    load();
  });

  async function load() {
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

  const modsById = $derived(new Map((snapshot?.mods ?? []).map((mod) => [mod.id, mod])));
  const activeMods = $derived(
    activeOrder.map((id) => modsById.get(id)).filter(Boolean)
  );
  const selected = $derived(selectedId ? modsById.get(selectedId) : null);
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
      if (stateFilter === "missing" && !mod.missing) return false;
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
    for (const mod of snapshot.mods) {
      const index = activeOrder.indexOf(mod.id);
      mod.active = index >= 0;
      mod.order = index >= 0 ? index : null;
    }
    snapshot.summary.active = activeOrder.length;
  }

  function toggle(mod) {
    if (mod.active) {
      mod.active = false;
      activeOrder = activeOrder.filter((id) => id !== mod.id);
    } else {
      activeOrder = [...activeOrder, mod.id];
    }
    syncOrderFields();
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
    busyText = t("Värskendan Workshopi andmeid (590 modi)…");
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

  async function saveProfile() {
    const name = profileName.trim();
    if (!name) return;
    busy = true;
    busyText = t("Salvestan profiili…");
    try {
      await invoke("save_profile", { name, active_ids: activeOrder });
      profileName = "";
      await load();
      tab = "profiles";
      message = t("Profiil salvestatud: {name}", { name });
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
      busyText = "";
    }
  }

  async function applyProfile(name) {
    busy = true;
    busyText = t("Rakendan profiili…");
    try {
      const ids = await invoke("get_profile", { name });
      activeOrder = [...ids];
      syncOrderFields();
      tab = "order";
      message = t("Profiil \"{name}\" rakendatud. Vajuta Salvesta, et see mängu configusse kirjutada.", { name });
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
      busyText = "";
    }
  }

  async function deleteProfile(name) {
    if (!window.confirm(t("Kustuta profiil \"{name}\"?", { name }))) return;
    busy = true;
    busyText = t("Kustutan profiili…");
    try {
      await invoke("delete_profile", { name });
      await load();
      tab = "profiles";
      message = t("Profiil kustutatud: {name}", { name });
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
      busyText = "";
    }
  }

  async function runDiagnostics() {
    busy = true;
    busyText = t("Kontrollin konflikte ja sõltuvusi ({count} aktiivset modi)…", { count: activeOrder.length });
    error = "";
    message = "";
    try {
      const report = await invoke("scan_diagnostics", { active_ids: activeOrder });
      snapshot.diagnostics = report;
      snapshot.summary.conflicts = report.conflicts.length;
      snapshot.summary.missing_dependencies = report.missing_dependency_links;
      tab = "diagnostics";
      message = t("Kontroll valmis: {missing} modi puudulike sõltuvustega, {conflicts} konflikti, {duplicates} duplikaatrühma.", { missing: report.missing_dependency_count, conflicts: report.conflicts.length, duplicates: report.duplicate_groups.length });
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
      busyText = "";
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

<svelte:window onkeydown={handleWindowKeydown} />

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
      <button
        class="ghost lang"
        onclick={toggleLanguage}
        title={lang === "et" ? "Switch to English" : "Lülita eesti keelde"}
      >{lang === "et" ? "EN" : "ET"}</button>
      <button class="ghost" onclick={load} disabled={loading || busy}>{t("Laadi uuesti")}</button>
      <button class="ghost" onclick={refreshWorkshop} disabled={loading || busy}>
        {t("Värskenda Workshopi")}
      </button>
      <button class="primary" onclick={save} disabled={loading || busy || !snapshot}>{t("Salvesta")}</button>
      <button class="launch" onclick={launch} disabled={loading || busy || !snapshot}>{t("Käivita XCOM 2")}</button>
    </div>
  </header>

  {#if busy && !loading}
    <div class="activity">
      <div class="activity-track"><div class="activity-bar"></div></div>
      <span>{busyText || t("Töötlen…")}</span>
    </div>
  {/if}

  {#if error}
    <div class="banner error">{error}</div>
  {/if}
  {#if message}
    <div class="banner info">{message}</div>
  {/if}

  {#if loading}
    <div class="loading">
      <div class="spinner"></div>
      <div>
        <strong>{t("Laen modi nimekirja…")}</strong>
        <p>{t("791 modi, AML settings + workshop metadata")}</p>
      </div>
    </div>
  {:else if snapshot}
    <section class="summary">
      <div class="stat">
        <strong>{snapshot.summary.active}</strong>
        <span>{t("aktiivset")}</span>
      </div>
      <div class="stat">
        <strong>{snapshot.summary.total}</strong>
        <span>{t("kokku")}</span>
      </div>
      <div class="stat" class:warn={snapshot.summary.missing > 0}>
        <strong>{snapshot.summary.missing}</strong>
        <span>{t("puudub")}</span>
      </div>
      <div class="stat" class:warn={snapshot.summary.unknown_active > 0}>
        <strong>{snapshot.summary.unknown_active}</strong>
        <span>{t("tundmatut")}</span>
      </div>
      <div class="stat" class:warn={snapshot.summary.updates > 0}>
        <strong>{snapshot.summary.updates}</strong>
        <span>{t("uuendust")}</span>
      </div>
      <div class="stat" class:warn={snapshot.summary.conflicts > 0}>
        <strong>{snapshot.summary.conflicts}</strong>
        <span>{t("konflikti")}</span>
      </div>
      <div class="stat" class:warn={snapshot.summary.missing_dependencies > 0}>
        <strong>{snapshot.summary.missing_dependencies}</strong>
        <span>{t("puuduvat sõltuvust")}</span>
      </div>
      <div class="stat">
        <strong>{snapshot.summary.categories}</strong>
        <span>{t("kategooriat")}</span>
      </div>
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
          </div>
          <div class="manager-add">
            <input
              type="text"
              placeholder={t("Profiili nimi…")}
              bind:value={profileName}
              onkeydown={(event) => event.key === "Enter" && saveProfile()}
            />
            <button class="primary" onclick={saveProfile} disabled={busy}>
              {t("Salvesta praegune ({count})", { count: activeOrder.length })}
            </button>
          </div>
          <div class="manager-list">
            {#each snapshot.profiles as profile (profile.name)}
              <div class="manager-row">
                <strong>{profile.name}</strong>
                <span class="muted">{t("{count} modi", { count: profile.count })}</span>
                <button class="mini" onclick={() => applyProfile(profile.name)}>{t("Rakenda")}</button>
                <button class="mini danger" onclick={() => deleteProfile(profile.name)}>{t("Kustuta")}</button>
              </div>
            {/each}
            {#if snapshot.profiles.length === 0}
              <div class="empty">{t("Profiile veel pole.")}</div>
            {/if}
          </div>
          <p class="manager-note">
            {t("Profiil salvestab aktiivsete modide järjekorra. Pärast Rakenda vajuta Salvesta, et see mängu XComModOptions.ini-sse kirjutada.")}
          </p>
        </div>
      {:else}
        <div class="manager diagnostics">
          <div class="manager-title">
            <h2>{t("Kontroll")}</h2>
            <span class="muted">
              {#if snapshot.diagnostics}
                {t("viimane kontroll {date} · {count} aktiivset modi", {
                  date: formatUnix(snapshot.diagnostics.scanned_at),
                  count: snapshot.diagnostics.active_mods,
                })}
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

            <h3 class="diag-heading">{t("Puuduvad sõltuvused")}</h3>
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

            <h3 class="diag-heading">{t("Klassikonfliktid")}</h3>
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
          {/if}
        </div>
      {/if}

      {#if selected}
        <aside class="details">
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
            https://github.com/reiokr/resistance-mod-launcher
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
    gap: 8px;
    flex-shrink: 0;
  }

  .top-actions .lang {
    min-width: 42px;
    padding-left: 8px;
    padding-right: 8px;
    font-weight: 700;
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
    gap: 8px;
    padding: 12px 22px;
    border-bottom: 1px solid #1a2330;
    background: rgba(11, 15, 20, 0.55);
  }

  .stat {
    min-width: 88px;
    padding: 7px 11px;
    border: 1px solid #202b3a;
    border-radius: 10px;
    background: rgba(21, 28, 38, 0.7);
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .stat strong {
    font-size: 16px;
    font-variant-numeric: tabular-nums;
    letter-spacing: -0.01em;
  }

  .stat span {
    font-size: 10.5px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: #728196;
  }

  .stat.warn strong {
    color: #ff8f8f;
  }

  .stat.args {
    min-width: 0;
    flex: 1;
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
