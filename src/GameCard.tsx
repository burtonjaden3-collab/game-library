import { useState } from "react";
import type { Game } from "./types";

const steamHeader = (appId: string) =>
  `https://cdn.cloudflare.steamstatic.com/steam/apps/${appId}/header.jpg`;

function formatSize(bytes: number | null) {
  if (!bytes) return null;
  const gb = bytes / 1024 ** 3;
  return gb >= 1 ? `${gb.toFixed(1)} GB` : `${Math.max(1, Math.round(bytes / 1024 ** 2))} MB`;
}

export function GameCard({ game, onLaunch }: { game: Game; onLaunch: (game: Game) => void }) {
  const [imageFailed, setImageFailed] = useState(false);
  const cover = game.source === "steam" && !imageFailed ? steamHeader(game.sourceId) : null;
  const size = game.installed ? formatSize(game.sizeBytes) : null;

  return (
    <article className={`card${game.installed ? "" : " not-installed"}`} title={game.title}>
      <div className="cover">
        <span className="cover-fallback">{game.title}</span>
        {cover && <img src={cover} alt="" loading="lazy" onError={() => setImageFailed(true)} />}
      </div>
      <div className="card-body">
        <h2>{game.title}</h2>
        <span className={`badge ${game.source}`}>{game.source}</span>
        {size && <span className="muted small">{size}</span>}
        {!game.installed && <span className="badge muted">not installed</span>}
        <div className="card-actions">
          <button className={game.installed ? "primary" : ""} onClick={() => onLaunch(game)}>
            {game.installed ? "Play" : "Install"}
          </button>
        </div>
      </div>
    </article>
  );
}
