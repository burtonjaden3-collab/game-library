import { type ReactNode, useCallback, useEffect, useMemo, useState } from "react";
import { importSteam, launchGame, listGames } from "./api";
import { SOURCE_LABELS } from "./gameArt";
import { GameCard } from "./GameCard";
import { GameDetails } from "./GameDetails";
import {
  CheckCircleIcon,
  CloseIcon,
  CloudIcon,
  GearIcon,
  GridIcon,
  RefreshIcon,
  SearchIcon,
} from "./icons";
import { Settings } from "./Settings";
import type { Game, ImportReport, Source } from "./types";

type Filter = "all" | "installed" | "not-installed" | `source:${Source}`;
type Sort = "title" | "played" | "recent" | "updated" | "size";

const FILTER_TITLES: Record<string, string> = {
  all: "All games",
  installed: "Installed",
  "not-installed": "Not installed",
};

function matches(game: Game, filter: Filter) {
  if (filter === "installed") return game.installed;
  if (filter === "not-installed") return !game.installed;
  if (filter.startsWith("source:")) return game.source === filter.slice(7);
  return true;
}

const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" });

export default function App() {
  const [games, setGames] = useState<Game[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [sort, setSort] = useState<Sort>("title");
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [importing, setImporting] = useState(false);
  const [report, setReport] = useState<ImportReport | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  // Bumped when metadata settings change so an open game page looks itself up again.
  const [metadataVersion, setMetadataVersion] = useState(0);

  const refresh = useCallback(async () => {
    try {
      setGames(await listGames());
    } catch (e) {
      setError(String(e));
    } finally {
      setLoaded(true);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const runSteamImport = async () => {
    setImporting(true);
    setError(null);
    try {
      setReport(await importSteam());
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setImporting(false);
    }
  };

  const launch = (game: Game) => {
    launchGame(game.id).catch((e) => setError(String(e)));
  };

  useEffect(() => {
    if (!report) return;
    const timer = window.setTimeout(() => setReport(null), 8000);
    return () => window.clearTimeout(timer);
  }, [report]);

  const closeDetails = useCallback(() => setSelectedId(null), []);
  const closeSettings = useCallback(() => setSettingsOpen(false), []);
  const onSettingsSaved = useCallback(() => setMetadataVersion((v) => v + 1), []);

  const counts = useMemo(() => {
    const bySource = new Map<Source, number>();
    for (const g of games) bySource.set(g.source, (bySource.get(g.source) ?? 0) + 1);
    const installed = games.filter((g) => g.installed).length;
    return { installed, notInstalled: games.length - installed, bySource };
  }, [games]);

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase();
    const list = games.filter(
      (g) => matches(g, filter) && (q === "" || g.title.toLowerCase().includes(q)),
    );
    const byTitle = (a: Game, b: Game) => collator.compare(a.title, b.title);
    if (sort === "title") return list.sort(byTitle);
    if (sort === "played")
      return list.sort(
        (a, b) => (b.playtimeMinutes ?? 0) - (a.playtimeMinutes ?? 0) || byTitle(a, b),
      );
    if (sort === "recent")
      return list.sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0) || byTitle(a, b));
    if (sort === "updated")
      return list.sort((a, b) => (b.lastUpdated ?? 0) - (a.lastUpdated ?? 0) || byTitle(a, b));
    return list.sort((a, b) => (b.sizeBytes ?? 0) - (a.sizeBytes ?? 0) || byTitle(a, b));
  }, [games, query, filter, sort]);

  const selected = games.find((g) => g.id === selectedId) ?? null;
  const viewTitle = filter.startsWith("source:")
    ? SOURCE_LABELS[filter.slice(7) as Source]
    : FILTER_TITLES[filter];

  const nav = (value: Filter, label: string, count: number, icon: ReactNode) => (
    <button
      key={value}
      className={`nav-item${filter === value && !selected ? " active" : ""}`}
      onClick={() => {
        setFilter(value);
        setSelectedId(null);
      }}
    >
      {icon}
      <span>{label}</span>
      <span className="nav-count">{count}</span>
    </button>
  );

  return (
    <div className="shell">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-mark" aria-hidden>
            <i />
            <i />
            <i />
          </span>
          Game Library
        </div>

        <nav className="nav">
          <div className="nav-heading">Library</div>
          {nav("all", "All games", games.length, <GridIcon />)}
          {nav("installed", "Installed", counts.installed, <CheckCircleIcon />)}
          {nav("not-installed", "Not installed", counts.notInstalled, <CloudIcon />)}

          {counts.bySource.size > 0 && <div className="nav-heading">Stores</div>}
          {[...counts.bySource].map(([source, count]) =>
            nav(
              `source:${source}`,
              SOURCE_LABELS[source],
              count,
              <span className={`source-dot lg ${source}`} />,
            ),
          )}
        </nav>

        <div className="sidebar-footer">
          <button className="btn-secondary wide" onClick={runSteamImport} disabled={importing}>
            <RefreshIcon className={importing ? "spin" : undefined} />
            {importing ? "Importing…" : "Import from Steam"}
          </button>
          <button className="btn-secondary wide" onClick={() => setSettingsOpen(true)}>
            <GearIcon />
            Settings
          </button>
        </div>
      </aside>

      <main className="main">
        {selected ? (
          <GameDetails
            key={selected.id}
            game={selected}
            metadataVersion={metadataVersion}
            onBack={closeDetails}
            onLaunch={launch}
          />
        ) : (
          <>
            <header className="topbar">
              <div className="view-title">
                <h1>{viewTitle}</h1>
                <span className="muted">{visible.length}</span>
              </div>
              <label className="search">
                <SearchIcon />
                <input
                  type="search"
                  placeholder="Search your games"
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                />
              </label>
              <select
                className="select"
                value={sort}
                onChange={(e) => setSort(e.target.value as Sort)}
                aria-label="Sort by"
              >
                <option value="title">Name</option>
                <option value="played">Most played</option>
                <option value="recent">Recently played</option>
                <option value="updated">Recently updated</option>
                <option value="size">Size on disk</option>
              </select>
            </header>

            {loaded && games.length === 0 ? (
              <div className="empty">
                <span className="brand-mark xl" aria-hidden>
                  <i />
                  <i />
                  <i />
                </span>
                <h2>Your library is empty</h2>
                <p className="muted">Import your Steam games to get started. Connect your Steam account in
                  Settings to include the ones you haven't installed.</p>
                <button className="btn-play" onClick={runSteamImport} disabled={importing}>
                  <RefreshIcon className={importing ? "spin" : undefined} />
                  {importing ? "Importing…" : "Import from Steam"}
                </button>
              </div>
            ) : visible.length === 0 && loaded ? (
              <div className="empty">
                <h2>No matches</h2>
                <p className="muted">
                  Nothing in {viewTitle.toLowerCase()} matches “{query}”.
                </p>
              </div>
            ) : (
              <section className="grid">
                {visible.map((g) => (
                  <GameCard
                    key={g.id}
                    game={g}
                    onOpen={(game) => setSelectedId(game.id)}
                    onLaunch={launch}
                  />
                ))}
              </section>
            )}
          </>
        )}
      </main>

      {settingsOpen && (
        <Settings
          onClose={closeSettings}
          onSaved={onSettingsSaved}
          onSteamConnected={() => void runSteamImport()}
        />
      )}

      <div className="toasts">
        {report && (
          <div className="toast" role="status">
            <CheckCircleIcon className="toast-icon ok" />
            <div>
              <strong>Steam import finished</strong>
              <p>
                {report.owned !== null
                  ? `${report.found} games, ${report.owned} owned on Steam`
                  : `${report.found} games in ${report.libraries.length} ${
                      report.libraries.length === 1 ? "library" : "libraries"
                    }`}{" "}
                · {report.added} new,{" "}
                {report.updated} updated
                {report.uninstalled > 0 && `, ${report.uninstalled} no longer installed`}
                {report.warnings.length > 0 && (
                  <span title={report.warnings.join("\n")}>
                    {" "}
                    · {report.warnings.length}{" "}
                    {report.warnings.length === 1 ? "warning" : "warnings"}
                  </span>
                )}
              </p>
            </div>
            <button className="icon-btn" onClick={() => setReport(null)} aria-label="Dismiss">
              <CloseIcon />
            </button>
          </div>
        )}
        {error && (
          <div className="toast error" role="alert">
            <div>
              <strong>Something went wrong</strong>
              <p>{error}</p>
            </div>
            <button className="icon-btn" onClick={() => setError(null)} aria-label="Dismiss">
              <CloseIcon />
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
