import { Cover } from "./Cover";
import { formatSize } from "./gameArt";
import { DownloadIcon, PlayIcon } from "./icons";
import type { Game } from "./types";

export function GameCard({
  game,
  onOpen,
  onLaunch,
}: {
  game: Game;
  onOpen: (game: Game) => void;
  onLaunch: (game: Game) => void;
}) {
  const size = game.installed ? formatSize(game.sizeBytes) : null;

  return (
    <article className={`card${game.installed ? "" : " not-installed"}`}>
      <div className="card-art">
        <button className="card-cover" onClick={() => onOpen(game)} title={game.title}>
          <Cover game={game} />
          {!game.installed && <span className="chip chip-overlay">Not installed</span>}
        </button>
        <button
          className={`card-action${game.installed ? " play" : ""}`}
          onClick={() => onLaunch(game)}
          aria-label={`${game.installed ? "Play" : "Install"} ${game.title}`}
          title={game.installed ? "Play" : "Install"}
        >
          {game.installed ? (
            <PlayIcon width={24} height={24} />
          ) : (
            <DownloadIcon width={22} height={22} />
          )}
        </button>
      </div>
      <div className="card-meta">
        <h2>{game.title}</h2>
        <p>
          <span className={`source-dot ${game.source}`} />
          {size ?? (game.installed ? "Installed" : "Not installed")}
        </p>
      </div>
    </article>
  );
}
