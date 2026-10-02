import type { Game } from "./types";

export function GameCard({ game }: { game: Game }) {
  return (
    <article className={`card${game.installed ? "" : " not-installed"}`} title={game.title}>
      <div className="cover">
        <span className="cover-fallback">{game.title}</span>
      </div>
      <div className="card-body">
        <h2>{game.title}</h2>
        <span className={`badge ${game.source}`}>{game.source}</span>
        {!game.installed && <span className="badge muted">not installed</span>}
      </div>
    </article>
  );
}
