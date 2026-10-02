import { useCallback, useEffect, useMemo, useState } from "react";
import { importSteam, launchGame, listGames } from "./api";
import { GameCard } from "./GameCard";
import type { Game, ImportReport } from "./types";

export default function App() {
  const [games, setGames] = useState<Game[]>([]);
  const [query, setQuery] = useState("");
  const [installedOnly, setInstalledOnly] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [importing, setImporting] = useState(false);
  const [report, setReport] = useState<ImportReport | null>(null);

  const refresh = useCallback(async () => {
    try {
      setGames(await listGames());
    } catch (e) {
      setError(String(e));
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

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase();
    return games.filter(
      (g) => (!installedOnly || g.installed) && (q === "" || g.title.toLowerCase().includes(q)),
    );
  }, [games, query, installedOnly]);

  return (
    <div className="app">
      <header className="toolbar">
        <h1>Game Library</h1>
        <input
          className="search"
          type="search"
          placeholder="Search your games"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <label className="toggle">
          <input
            type="checkbox"
            checked={installedOnly}
            onChange={(e) => setInstalledOnly(e.target.checked)}
          />
          Installed only
        </label>
        <button className="primary" onClick={runSteamImport} disabled={importing}>
          {importing ? "Importing…" : "Import from Steam"}
        </button>
      </header>

      {report && (
        <div className="banner" role="status">
          <span>
            Steam: {report.found} games in {report.libraries.length}{" "}
            {report.libraries.length === 1 ? "library" : "libraries"} ({report.added} new,{" "}
            {report.updated} updated
            {report.uninstalled > 0 && `, ${report.uninstalled} no longer installed`})
            {report.warnings.length > 0 && (
              <span className="muted" title={report.warnings.join("\n")}>
                {" "}
                · {report.warnings.length} files skipped
              </span>
            )}
          </span>
          <button onClick={() => setReport(null)}>Dismiss</button>
        </div>
      )}

      {error && (
        <div className="banner error" role="alert">
          {error}
          <button onClick={() => setError(null)}>Dismiss</button>
        </div>
      )}

      {games.length === 0 ? (
        <div className="empty">
          <p>Your library is empty.</p>
          <p className="muted">Import your installed Steam games to get started.</p>
          <button className="primary" onClick={runSteamImport} disabled={importing}>
            {importing ? "Importing…" : "Import from Steam"}
          </button>
        </div>
      ) : (
        <main className="grid">
          {visible.map((g) => (
            <GameCard key={g.id} game={g} onLaunch={launch} />
          ))}
        </main>
      )}
    </div>
  );
}
