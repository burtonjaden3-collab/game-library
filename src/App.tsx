import { useCallback, useEffect, useMemo, useState } from "react";
import { listGames } from "./api";
import { GameCard } from "./GameCard";
import type { Game } from "./types";

export default function App() {
  const [games, setGames] = useState<Game[]>([]);
  const [query, setQuery] = useState("");
  const [installedOnly, setInstalledOnly] = useState(false);
  const [error, setError] = useState<string | null>(null);

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
      </header>

      {error && (
        <div className="banner error" role="alert">
          {error}
          <button onClick={() => setError(null)}>Dismiss</button>
        </div>
      )}

      {games.length === 0 ? (
        <div className="empty">
          <p>Your library is empty.</p>
          <p className="muted">Store importers are on the way, starting with Steam.</p>
        </div>
      ) : (
        <main className="grid">
          {visible.map((g) => (
            <GameCard key={g.id} game={g} />
          ))}
        </main>
      )}
    </div>
  );
}
