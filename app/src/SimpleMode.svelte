<script>
  let {
    snapshot,
    currentProfile,
    busy,
    busyText,
    gameConfigDirty,
    diagnosticsFresh,
    coverUrls = {},
    t,
    onSelectProfile,
    onStart,
    onAdvanced,
    onManageProfiles,
    onChecks,
    onOpenWorkshop,
  } = $props();

  const selected = $derived(
    (snapshot?.profiles ?? []).find((profile) => profile.name === currentProfile) ?? null
  );
  const modsById = $derived(
    new Map((snapshot?.mods ?? []).map((mod) => [mod.id, mod]))
  );
  const profilePreviews = $derived.by(() => {
    const map = new Map();
    for (const profile of snapshot?.profiles ?? []) {
      const urls = [];
      const seen = new Set();
      for (const id of profile.active_ids ?? []) {
        const url = modsById.get(id)?.preview_url;
        if (url && !seen.has(url)) {
          seen.add(url);
          urls.push(url);
        }
        if (urls.length >= 4) break;
      }
      map.set(profile.name, urls);
    }
    return map;
  });
  const selectedPreviews = $derived(
    selected ? profilePreviews.get(selected.name) ?? [] : []
  );
  const conflicts = $derived(snapshot?.diagnostics?.conflicts?.length ?? 0);
  const missingDeps = $derived(snapshot?.diagnostics?.missing_dependency_count ?? 0);
  const missing = $derived(snapshot?.summary?.missing ?? 0);
  const activeCount = $derived(snapshot?.summary?.active ?? 0);

  function diagnosticsTime() {
    const value = snapshot?.diagnostics?.scanned_at;
    return value ? new Date(value * 1000).toLocaleString() : "";
  }
</script>

<div class="simple">
  <div class="simple-inner">
    <section class="simple-hero">
      <div>
        <h2>{t("Vali modpack")}</h2>
        <p>
          {t("Vali profiil ja vajuta Start. Launcher rakendab ja salvestab profiili enne käivitamist.")}
        </p>
      </div>
      <div class="hero-actions">
        <button class="link-button" onclick={onManageProfiles}>{t("Halda profiile")}</button>
        <button class="link-button" onclick={onAdvanced}>{t("Täisvaade")}</button>
      </div>
    </section>

    {#if (snapshot?.profiles ?? []).length === 0}
      <div class="empty-card">
        <strong>{t("Sul pole veel profiile.")}</strong>
        <p>{t("Ava täisvaade, et profiil luua või Steam Workshopi kollektsioon importida.")}</p>
        <button class="primary-button" onclick={onAdvanced}>{t("Ava täisvaade")}</button>
      </div>
    {:else}
      <section class="profile-grid" aria-label={t("Profiilid")}>
        {#each snapshot.profiles as profile (profile.name)}
          {@const previews = profilePreviews.get(profile.name) ?? []}
          {@const coverUrl = coverUrls[profile.name] ?? previews[0]}
          <button
            type="button"
            class="profile-card"
            class:selected={profile.name === currentProfile}
            onclick={() => profile.name !== currentProfile && onSelectProfile(profile.name)}
            disabled={busy}
          >
            <div class="profile-cover">
              {#if coverUrl}
                <img
                  class="profile-cover-image"
                  src={coverUrl}
                  alt=""
                  loading="lazy"
                  onerror={(event) => (event.currentTarget.style.display = "none")}
                />
              {:else}
                <div class="mosaic-empty">{profile.name.slice(0, 2)}</div>
              {/if}
              <div class="cover-shade"></div>
            </div>
            <div class="profile-content">
              <div class="cover-name">{profile.name}</div>
              {#if profile.description}
                <p class="profile-desc" title={profile.description}>{profile.description}</p>
              {/if}
              <div class="profile-meta">
                <span>{t("{count} modi", { count: profile.count })}</span>
                {#if profile.deactivated_mod_ids?.length > 0}
                  <span>· {t("{count} deaktiveeritud", { count: profile.deactivated_mod_ids.length })}</span>
                {/if}
                {#if profile.pending_count > 0}
                  <span>· {t("{count} ootab", { count: profile.pending_count })}</span>
                {/if}
              </div>
              {#if profile.source_title}
                <span class="profile-source">{profile.source_title}</span>
              {/if}
            </div>
          </button>
        {/each}
      </section>

      <section class="ready-card">
        {#if selected}
          <div class="ready-cover">
            {#if selectedPreviews.length > 0}
              <div class="ready-mosaic ready-mosaic-{selectedPreviews.length}">
                {#each selectedPreviews as url (url)}
                  <img
                    src={url}
                    alt=""
                    loading="lazy"
                    onerror={(event) => (event.currentTarget.style.display = "none")}
                  />
                {/each}
              </div>
            {:else}
              <div class="ready-mosaic-empty">{selected.name.slice(0, 2)}</div>
            {/if}
            <div class="ready-shade"></div>
            <div class="ready-cover-content">
              <span class="eyebrow">{t("Valmis mängima")}</span>
              <h2>{selected.name}</h2>
              {#if selected.source_title}
                <p>{selected.source_title}</p>
              {/if}
            </div>
          </div>

          <div class="ready-body">
          <div class="stats">
            <div class="stat">
              <strong>{selected.count}</strong>
              <span>{t("profiili modi")}</span>
            </div>
            <div class="stat">
              <strong>{activeCount}</strong>
              <span>{t("aktiivset")}</span>
            </div>
            <div class="stat" class:bad={missing > 0}>
              <strong>{missing}</strong>
              <span>{t("puudub")}</span>
            </div>
            <div class="stat">
              <strong>{selected.deactivated_mod_ids?.length ?? 0}</strong>
              <span>{t("deaktiveeritud")}</span>
            </div>
            <div class="stat" class:warn={selected.pending_count > 0}>
              <strong>{selected.pending_count}</strong>
              <span>{t("ootab")}</span>
            </div>
            <div class="stat" class:warn={conflicts > 0 || missingDeps > 0}>
              <strong>{snapshot.diagnostics ? conflicts : "—"}</strong>
              <span>{t("konflikti")}</span>
            </div>
            <div class="stat" class:warn={missingDeps > 0}>
              <strong>{snapshot.diagnostics ? missingDeps : "—"}</strong>
              <span>{t("puuduvat sõltuvust")}</span>
            </div>
          </div>

          <div class="status-row">
            <span class="status-chip" class:ok={snapshot.summary.steam_running} class:bad={!snapshot.summary.steam_running}>
              {snapshot.summary.steam_running ? t("Steam töötab") : t("Steam ei tööta")}
            </span>
            <span class="status-chip" class:ok={!snapshot.summary.aml_running} class:bad={snapshot.summary.aml_running}>
              {snapshot.summary.aml_running ? t("AML on lahti") : t("AML suletud")}
            </span>
            <span class="status-chip" class:ok={!snapshot.summary.game_running} class:bad={snapshot.summary.game_running}>
              {snapshot.summary.game_running ? t("XCOM 2 töötab") : t("XCOM 2 suletud")}
            </span>
          </div>

          <div class="warnings">
            {#if missing > 0}
              <div class="simple-warning danger">
                {t("Selles profiilis on {count} puuduvat aktiivset modi. Start küsib enne kinnitust.", { count: missing })}
              </div>
            {/if}
            {#if selected.pending_count > 0}
              <div class="simple-warning">
                {t("Selles profiilis on {count} kollektsiooni modi, mis pole veel paigaldatud.", { count: selected.pending_count })}
                {#if selected.source_id}
                  <button class="inline-link" onclick={() => onOpenWorkshop(selected.source_id)}>
                    {t("Ava Steamis")}
                  </button>
                {/if}
              </div>
            {/if}
            {#if !diagnosticsFresh}
              <div class="simple-note">
                {t("Kontroll on aegunud või veel tegemata.")}
                <button class="inline-link" onclick={onChecks}>{t("Kontrolli enne käivitamist")}</button>
              </div>
            {:else if conflicts > 0 || missingDeps > 0}
              <div class="simple-warning">
                {t("Kontroll: {conflicts} konflikti, {missing} puudulikku sõltuvust.", {
                  conflicts,
                  missing: missingDeps,
                })}
                <button class="inline-link" onclick={onChecks}>{t("Vaata kontrolli")}</button>
              </div>
            {:else}
              <div class="simple-note ok">
                {t("Kontrollitud: {time}", { time: diagnosticsTime() })}
              </div>
            {/if}
            {#if gameConfigDirty}
              <div class="simple-note">
                {t("Start rakendab ja salvestab selle profiili mängu configusse enne käivitamist.")}
              </div>
            {/if}
          </div>

          <div class="ready-footer">
            <button class="start" onclick={onStart} disabled={busy || !currentProfile}>
              {busy ? busyText || t("Käivitan…") : t("Käivita mäng")}
            </button>
            <div class="secondary-actions">
              <button class="secondary-action" onclick={onChecks}>{t("Vaata kontrolli")}</button>
              {#if selected.source_id}
                <button class="secondary-action" onclick={() => onOpenWorkshop(selected.source_id)}>
                  {t("Ava Steamis")}
                </button>
              {/if}
              <button class="secondary-action" onclick={onAdvanced}>{t("Täisvaade")}</button>
            </div>
          </div>
          </div>
        {:else}
          <div class="empty-ready">
            <strong>{t("Vali ülal profiil.")}</strong>
            <p>{t("Seejärel saad profiili info ja Start-nupu.")}</p>
          </div>
        {/if}
      </section>
    {/if}
  </div>
</div>

<style>
  .simple {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 28px 40px 48px;
  }

  .simple-inner {
    width: 100%;
    max-width: 1800px;
    margin: 0 auto;
    display: flex;
    flex-direction: column;
    gap: 18px;
  }

  .simple-hero {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 18px;
    flex-wrap: wrap;
  }

  .simple-hero h2 {
    margin: 0;
    font-size: 24px;
    letter-spacing: -0.02em;
  }

  .simple-hero p {
    margin: 5px 0 0;
    color: #8d9bb0;
    font-size: 12.5px;
  }

  .hero-actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }

  .profile-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(290px, 1fr));
    gap: 14px;
    width: 100%;
  }

  .profile-card {
    position: relative;
    text-align: left;
    display: flex;
    flex-direction: column;
    padding: 0;
    overflow: hidden;
    border: 1px solid #202b3a;
    border-radius: 12px;
    background: rgba(21, 28, 38, 0.65);
    color: #d7e1ec;
    cursor: pointer;
    transition: border-color 120ms ease, background 120ms ease, transform 120ms ease;
  }

  .profile-card:hover:not(:disabled) {
    border-color: #3a4b63;
    background: rgba(27, 36, 49, 0.85);
    transform: translateY(-1px);
  }

  .profile-card.selected {
    border-color: rgba(61, 220, 151, 0.75);
    background: rgba(61, 220, 151, 0.07);
    box-shadow: inset 0 0 0 1px rgba(61, 220, 151, 0.2);
  }

  .profile-card:disabled {
    cursor: wait;
    opacity: 0.7;
  }

  .profile-cover {
    position: relative;
    aspect-ratio: 16 / 10;
    overflow: hidden;
    background: #0b1016;
  }

  .profile-cover-image {
    width: 100%;
    height: 100%;
    object-fit: contain;
    display: block;
  }

  .mosaic-empty,
  .ready-mosaic-empty {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
    background: linear-gradient(135deg, #16212e, #0d131b);
    color: #3ddc97;
    font-size: 30px;
    font-weight: 700;
    letter-spacing: 0.05em;
  }

  .profile-content {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 11px 14px 14px;
  }

  .cover-name {
    color: #f0f6fc;
    font-size: 19px;
    font-weight: 600;
    line-height: 1.25;
    white-space: normal;
    overflow: visible;
  }

  .profile-card.selected .cover-name {
    color: #c6ffe1;
  }

  .profile-desc {
    margin: 0;
    color: #8d9bb0;
    font-size: 11.5px;
    line-height: 1.4;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .profile-meta {
    display: flex;
    flex-wrap: wrap;
    gap: 3px 6px;
    color: #728196;
    font-size: 10.5px;
  }

  .profile-source {
    color: #5f7187;
    font-size: 10px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .simple-badge {
    flex-shrink: 0;
    padding: 2px 6px;
    border: 1px solid #2a3648;
    border-radius: 999px;
    color: #8d9bb0;
    font-size: 9px;
    white-space: nowrap;
  }

  .ready-card {
    padding: 0;
    overflow: hidden;
    border: 1px solid #263244;
    border-radius: 14px;
    background: rgba(15, 21, 30, 0.72);
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.18);
  }

  .ready-cover {
    position: relative;
    height: 210px;
    overflow: hidden;
    background: #111821;
  }

  .ready-mosaic {
    display: grid;
    width: 100%;
    height: 100%;
    gap: 2px;
    background: #0d131b;
  }

  .ready-mosaic-1 {
    grid-template-columns: 1fr;
  }

  .ready-mosaic-2 {
    grid-template-columns: repeat(2, 1fr);
  }

  .ready-mosaic-3 {
    grid-template-columns: repeat(3, 1fr);
  }

  .ready-mosaic-4 {
    grid-template-columns: repeat(4, 1fr);
  }

  .ready-mosaic img {
    width: 100%;
    height: 100%;
    object-fit: contain;
    display: block;
    background: #0b1016;
  }

  .ready-shade {
    position: absolute;
    inset: 0;
    background:
      linear-gradient(90deg, rgba(7, 11, 16, 0.94) 0%, rgba(7, 11, 16, 0.42) 50%, rgba(7, 11, 16, 0.18) 100%),
      linear-gradient(0deg, rgba(7, 11, 16, 0.82), transparent 48%);
  }

  .ready-cover-content {
    position: absolute;
    left: 22px;
    right: 22px;
    bottom: 17px;
  }

  .ready-cover-content h2 {
    margin: 3px 0 2px;
    color: #f4f8ff;
    font-size: 30px;
    letter-spacing: -0.03em;
    text-shadow: 0 2px 16px rgba(0, 0, 0, 0.7);
  }

  .ready-cover-content p {
    margin: 0;
    color: #aebdcd;
    font-size: 12px;
  }

  .ready-body {
    padding: 14px 20px 18px;
  }

  .eyebrow {
    color: #3ddc97;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.1em;
  }

  .muted {
    color: #728196;
    font-size: 11.5px;
  }

  .stats {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(105px, 1fr));
    gap: 7px;
    margin: 15px 0 12px;
  }

  .stat {
    padding: 8px 10px;
    border: 1px solid #202b3a;
    border-radius: 9px;
    background: rgba(21, 28, 38, 0.55);
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .stat strong {
    font-size: 18px;
    font-variant-numeric: tabular-nums;
  }

  .stat span {
    color: #728196;
    font-size: 10px;
  }

  .stat.warn {
    border-color: rgba(240, 180, 41, 0.3);
  }

  .stat.warn strong {
    color: #f3d28a;
  }

  .stat.bad {
    border-color: rgba(255, 107, 107, 0.35);
  }

  .stat.bad strong {
    color: #ff9f9f;
  }

  .status-row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .status-chip {
    padding: 3px 8px;
    border-radius: 999px;
    border: 1px solid #263244;
    color: #8d9bb0;
    font-size: 10px;
  }

  .status-chip.ok {
    border-color: rgba(61, 220, 151, 0.25);
    color: #8ee0b3;
    background: rgba(61, 220, 151, 0.05);
  }

  .status-chip.bad {
    border-color: rgba(255, 107, 107, 0.3);
    color: #ff9f9f;
    background: rgba(255, 107, 107, 0.05);
  }

  .warnings {
    display: flex;
    flex-direction: column;
    gap: 7px;
    margin: 12px 0;
  }

  .simple-warning {
    padding: 8px 10px;
    border: 1px solid rgba(240, 180, 41, 0.28);
    border-radius: 9px;
    background: rgba(240, 180, 41, 0.05);
    color: #d8c28d;
    font-size: 11.5px;
  }

  .simple-warning.danger {
    border-color: rgba(255, 107, 107, 0.3);
    background: rgba(255, 107, 107, 0.05);
    color: #ffb3b3;
  }

  .simple-note {
    color: #8d9bb0;
    font-size: 11.5px;
  }

  .simple-note.ok {
    color: #8ee0b3;
  }

  .inline-link {
    padding: 0;
    border: 0;
    background: transparent;
    color: #2bb3ff;
    font: inherit;
    text-decoration: underline;
    cursor: pointer;
  }

  .ready-footer {
    display: flex;
    align-items: stretch;
    gap: 10px;
    flex-wrap: wrap;
    margin-top: 14px;
  }

  .start {
    flex: 1;
    min-width: 220px;
    min-height: 48px;
    padding: 13px 18px;
    border: 1px solid #3ddc97;
    border-radius: 10px;
    background: #3ddc97;
    color: #06140c;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 15px;
    font-weight: 700;
    cursor: pointer;
  }

  .start:hover:not(:disabled) {
    background: #54e5a8;
  }

  .start:disabled {
    cursor: not-allowed;
    opacity: 0.55;
  }

  .secondary-actions {
    display: flex;
    align-items: stretch;
    gap: 8px;
    flex-wrap: wrap;
  }

  .secondary-action {
    min-width: 112px;
    min-height: 48px;
    padding: 0 16px;
    border: 1px solid #2a3648;
    border-radius: 10px;
    background: #151c26;
    color: #b9c7d8;
    font: inherit;
    font-size: 13px;
    cursor: pointer;
    white-space: nowrap;
  }

  .secondary-action:hover {
    background: #1b2431;
    border-color: #3a4b63;
    color: #e6edf3;
  }

  .link-button {
    padding: 7px 9px;
    border: 1px solid #2a3648;
    border-radius: 8px;
    background: transparent;
    color: #9fb0c6;
    font: inherit;
    font-size: 11px;
    cursor: pointer;
  }

  .link-button:hover {
    border-color: #3a4b63;
    color: #d7e1ec;
  }

  .primary-button {
    padding: 9px 14px;
    border: 1px solid #3ddc97;
    border-radius: 9px;
    background: #3ddc97;
    color: #06140c;
    font-weight: 700;
    cursor: pointer;
  }

  .empty-card,
  .empty-ready {
    padding: 22px;
    border: 1px solid #202b3a;
    border-radius: 12px;
    background: rgba(21, 28, 38, 0.55);
  }

  .empty-card p,
  .empty-ready p {
    color: #8d9bb0;
    font-size: 12px;
    margin: 5px 0 12px;
  }

  .empty-ready strong {
    font-size: 13px;
  }

  @media (max-width: 720px) {
    .simple {
      padding: 18px;
    }

    .profile-grid {
      grid-template-columns: 1fr;
    }

    .start {
      min-width: 100%;
    }

    .secondary-actions {
      width: 100%;
    }

    .secondary-action {
      flex: 1;
    }
  }
</style>
