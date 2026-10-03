import { type ReactNode, useCallback, useEffect, useState } from "react";
import { Art } from "./Art";
import { Cover } from "./Cover";
import { SOURCE_LABELS, artFor, formatDate, formatSize, generatedPalette } from "./gameArt";
import {
  ArrowLeftIcon,
  ClockIcon,
  CloudIcon,
  DownloadIcon,
  DriveIcon,
  FolderIcon,
  PlayIcon,
} from "./icons";
import type { Game } from "./types";

/**
 * A game's page. The info tiles are laid out as a fixed set of widgets for now; they are
 * the pieces the drag-and-drop layout editor will arrange later.
 */
export function GameDetails({
  game,
  onBack,
  onLaunch,
}: {
  game: Game;
  onBack: () => void;
  onLaunch: (game: Game) => void;
}) {
  const art = artFor(game);
  const [logoFailed, setLogoFailed] = useState(false);
  const onLogoExhausted = useCallback(() => setLogoFailed(true), []);
  useEffect(() => setLogoFailed(false), [game.id]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onBack();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onBack]);

  const size = formatSize(game.sizeBytes);
  const updated = formatDate(game.lastUpdated);
  const added = formatDate(game.addedAt);

  return (
    <div className="details">
      <section className="hero" style={generatedPalette(game.title)}>
        <div className="hero-generated" aria-hidden />
        <Art className="hero-img" sources={art.hero} />
        <div className="hero-fade" aria-hidden />
        <button className="ghost back" onClick={onBack}>
          <ArrowLeftIcon /> Library
        </button>
        <div className="hero-content">
          {logoFailed || art.logo.length === 0 ? (
            <h1 className="hero-title">{game.title}</h1>
          ) : (
            <>
              <h1 className="sr-only">{game.title}</h1>
              <Art className="hero-logo" sources={art.logo} onExhausted={onLogoExhausted} />
            </>
          )}
          <div className="hero-actions">
            <button
              className={game.installed ? "btn-play" : "btn-install"}
              onClick={() => onLaunch(game)}
            >
              {game.installed ? <PlayIcon /> : <DownloadIcon />}
              {game.installed ? "Play" : "Install"}
            </button>
            <span className={`chip ${game.installed ? "chip-ok" : ""}`}>
              {game.installed ? "Installed" : "Not installed"}
            </span>
            <span className="chip">
              <span className={`source-dot ${game.source}`} />
              {SOURCE_LABELS[game.source]}
            </span>
          </div>
        </div>
      </section>

      <section className="widgets">
        <div className="widget widget-cover">
          <Cover game={game} />
        </div>
        <div className="widget-stack">
          <div className="widget-row">
            <Stat icon={<DriveIcon />} label="Size on disk" value={size ?? "—"} />
            <Stat icon={<CloudIcon />} label="Store" value={SOURCE_LABELS[game.source]} />
            <Stat icon={<ClockIcon />} label="Last updated" value={updated ?? "—"} />
            <Stat icon={<ClockIcon />} label="Added" value={added ?? "—"} />
          </div>
          <div className="widget">
            <div className="widget-label">
              <FolderIcon /> Install location
            </div>
            <code className="path">{game.installDir ?? "Not installed"}</code>
          </div>
          <div className="widget">
            <div className="widget-label">About</div>
            <p className="muted">
              Descriptions, genres, playtime and screenshots arrive with metadata support. You will
              be able to rearrange this page by dragging widgets around.
            </p>
          </div>
        </div>
      </section>
    </div>
  );
}

function Stat({ icon, label, value }: { icon: ReactNode; label: string; value: string }) {
  return (
    <div className="widget stat">
      <div className="widget-label">
        {icon}
        <span>{label}</span>
      </div>
      <div className="stat-value">{value}</div>
    </div>
  );
}
