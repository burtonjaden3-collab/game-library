import { type ReactNode, useCallback, useEffect, useRef, useState } from "react";
import { openExternal } from "./api";
import { Art } from "./Art";
import { Cover } from "./Cover";
import { SOURCE_LABELS, artFor, formatDate, formatSize, generatedPalette } from "./gameArt";
import {
  ArrowLeftIcon,
  BuildingIcon,
  CalendarIcon,
  ClockIcon,
  CloseIcon,
  CloudIcon,
  CodeIcon,
  DownloadIcon,
  DriveIcon,
  ExternalIcon,
  FolderIcon,
  PlayIcon,
  RefreshIcon,
  StarIcon,
} from "./icons";
import type { Game, GameMetadata } from "./types";
import { useGameMetadata } from "./useGameMetadata";

const PROVIDER_LABELS: Record<string, string> = { steam: "Steam store", igdb: "IGDB" };

/**
 * A game's page. The info tiles are laid out as a fixed set of widgets for now; they are
 * the pieces the drag-and-drop layout editor will arrange later. Render it with
 * `key={game.id}` so per-game state starts fresh.
 */
export function GameDetails({
  game,
  metadataVersion,
  onBack,
  onLaunch,
}: {
  game: Game;
  metadataVersion: number;
  onBack: () => void;
  onLaunch: (game: Game) => void;
}) {
  const art = artFor(game);
  const [logoFailed, setLogoFailed] = useState(false);
  const onLogoExhausted = useCallback(() => setLogoFailed(true), []);
  const { cached, loading, error, refresh } = useGameMetadata(game.id, metadataVersion);
  const meta = cached?.metadata ?? null;
  const [shot, setShot] = useState<number | null>(null);
  const shotRef = useRef(shot);
  shotRef.current = shot;
  const shotCount = meta?.screenshots.length ?? 0;

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const open = shotRef.current;
      if (open === null) {
        if (e.key === "Escape") onBack();
        return;
      }
      if (e.key === "Escape") setShot(null);
      if (e.key === "ArrowRight") setShot((open + 1) % shotCount);
      if (e.key === "ArrowLeft") setShot((open - 1 + shotCount) % shotCount);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onBack, shotCount]);

  const size = formatSize(game.sizeBytes);
  const updated = formatDate(game.lastUpdated);
  const added = formatDate(game.addedAt);
  const year = meta?.releaseDate?.match(/\d{4}/)?.[0];
  const tagline = [...(meta?.genres.slice(0, 3) ?? []), year].filter(Boolean).join(" · ");

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
          {tagline && <p className="hero-tagline">{tagline}</p>}
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
          <Cover game={game} coverUrl={meta?.coverUrl} />
        </div>
        <div className="widget-stack">
          <div className="widget-row">
            <Stat icon={<DriveIcon />} label="Size on disk" value={size ?? "—"} />
            <Stat icon={<CloudIcon />} label="Store" value={SOURCE_LABELS[game.source]} />
            <Stat icon={<ClockIcon />} label="Last updated" value={updated ?? "—"} />
            <Stat icon={<ClockIcon />} label="Added" value={added ?? "—"} />
          </div>

          {meta && <InfoRow meta={meta} />}

          <About
            meta={meta}
            loading={loading}
            error={error}
            notFound={cached !== undefined && meta === null}
            source={game.source}
            onRefresh={refresh}
          />

          {meta &&
            (meta.genres.length > 0 || meta.features.length > 0 || meta.platforms.length > 0) && (
              <div className="widget">
                <TagGroup label="Genres" tags={meta.genres} />
                <TagGroup label="Features" tags={meta.features} />
                <TagGroup label="Platforms" tags={meta.platforms} highlight="Linux" />
              </div>
            )}

          {meta && meta.screenshots.length > 0 && (
            <div className="widget">
              <div className="widget-label">Screenshots</div>
              <div className="shots">
                {meta.screenshots.map((s, i) => (
                  <button key={s.full} className="shot" onClick={() => setShot(i)}>
                    <img
                      src={s.thumbnail}
                      alt={`Screenshot ${i + 1}`}
                      loading="lazy"
                      draggable={false}
                    />
                  </button>
                ))}
              </div>
            </div>
          )}

          <div className="widget">
            <div className="widget-label">
              <FolderIcon /> Install location
            </div>
            <code className="path">{game.installDir ?? "Not installed"}</code>
          </div>
        </div>
      </section>

      {meta && shot !== null && meta.screenshots[shot] && (
        <div
          className="lightbox"
          role="dialog"
          aria-label="Screenshot"
          onClick={() => setShot(null)}
        >
          <img src={meta.screenshots[shot].full} alt={`Screenshot ${shot + 1}`} />
          <button className="icon-btn lightbox-close" aria-label="Close">
            <CloseIcon />
          </button>
          <span className="lightbox-count">
            {shot + 1} / {meta.screenshots.length}
          </span>
        </div>
      )}
    </div>
  );
}

function InfoRow({ meta }: { meta: GameMetadata }) {
  const tiles: ReactNode[] = [];
  if (meta.developers.length > 0)
    tiles.push(
      <Stat key="dev" icon={<CodeIcon />} label="Developer" value={meta.developers.join(", ")} />,
    );
  if (meta.publishers.length > 0)
    tiles.push(
      <Stat
        key="pub"
        icon={<BuildingIcon />}
        label="Publisher"
        value={meta.publishers.join(", ")}
      />,
    );
  if (meta.releaseDate)
    tiles.push(
      <Stat key="date" icon={<CalendarIcon />} label="Released" value={meta.releaseDate} />,
    );
  if (meta.rating) {
    const { score, source, url } = meta.rating;
    tiles.push(
      <Stat
        key="rating"
        icon={<StarIcon />}
        label={source}
        value={
          <span className={`score ${score >= 75 ? "good" : score >= 50 ? "mixed" : "bad"}`}>
            {score}
            {url && (
              <button
                className="icon-btn inline"
                onClick={() => openExternal(url)}
                aria-label={`Open on ${source}`}
              >
                <ExternalIcon />
              </button>
            )}
          </span>
        }
      />,
    );
  }
  return tiles.length > 0 ? <div className="widget-row">{tiles}</div> : null;
}

function About({
  meta,
  loading,
  error,
  notFound,
  source,
  onRefresh,
}: {
  meta: GameMetadata | null;
  loading: boolean;
  error: string | null;
  notFound: boolean;
  source: Game["source"];
  onRefresh: () => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const long = (meta?.description?.length ?? 0) > 600;

  let body: ReactNode;
  if (meta) {
    body = (
      <>
        {meta.summary && <p className="about-summary">{meta.summary}</p>}
        {meta.description && meta.description !== meta.summary && (
          <div className={`about-text${long && !expanded ? " clamped" : ""}`}>
            {meta.description}
          </div>
        )}
        {long && (
          <button className="link" onClick={() => setExpanded((x) => !x)}>
            {expanded ? "Show less" : "Read more"}
          </button>
        )}
        {!meta.summary && !meta.description && <p className="muted">No description available.</p>}
      </>
    );
  } else if (loading) {
    body = (
      <div className="skeleton" aria-label="Loading game info">
        <i />
        <i />
        <i />
      </div>
    );
  } else if (error) {
    body = <p className="about-error">{error}</p>;
  } else if (notFound) {
    body = (
      <p className="muted">
        {source === "steam"
          ? "Steam's store has no page for this game."
          : "No game info found for this game."}{" "}
        Add an IGDB key in Settings to look games up in IGDB too.
      </p>
    );
  }

  return (
    <div className="widget">
      <div className="widget-label about-head">
        <span>About</span>
        <span className="about-actions">
          {meta?.sourceUrl && (
            <button className="link" onClick={() => openExternal(meta.sourceUrl!)}>
              {PROVIDER_LABELS[meta.provider] ?? meta.provider} <ExternalIcon />
            </button>
          )}
          {meta?.website && (
            <button className="link" onClick={() => openExternal(meta.website!)}>
              Website <ExternalIcon />
            </button>
          )}
          <button
            className="icon-btn inline"
            onClick={onRefresh}
            disabled={loading}
            aria-label="Refresh game info"
          >
            <RefreshIcon className={loading ? "spin" : undefined} />
          </button>
        </span>
      </div>
      {body}
    </div>
  );
}

function TagGroup({
  label,
  tags,
  highlight,
}: {
  label: string;
  tags: string[];
  highlight?: string;
}) {
  if (tags.length === 0) return null;
  return (
    <div className="tag-group">
      <div className="widget-label">{label}</div>
      <div className="tags">
        {tags.map((t) => (
          <span key={t} className={`chip${t === highlight ? " chip-ok" : ""}`}>
            {t}
          </span>
        ))}
      </div>
    </div>
  );
}

function Stat({ icon, label, value }: { icon: ReactNode; label: string; value: ReactNode }) {
  return (
    <div className="widget stat">
      <div className="widget-label">
        {icon}
        <span>{label}</span>
      </div>
      <div className="stat-value" title={typeof value === "string" ? value : undefined}>
        {value}
      </div>
    </div>
  );
}
