// Mirrors library_core::model. Keep in sync with crates/library-core/src/model.rs.

export type Source = "steam" | "epic" | "gog" | "itch" | "manual";

export interface Game {
  id: number;
  source: Source;
  sourceId: string;
  title: string;
  installDir: string | null;
  installed: boolean;
  sizeBytes: number | null;
  lastUpdated: number | null;
  addedAt: number;
  /** Known once a Steam account is connected. */
  playtimeMinutes: number | null;
  lastPlayed: number | null;
}

/** Result of one store import. Mirrors ImportReport in src-tauri/src/lib.rs. */
export interface ImportReport {
  libraries: string[];
  found: number;
  /** Games the connected Steam account owns; null when no account is connected. */
  owned: number | null;
  added: number;
  updated: number;
  uninstalled: number;
  warnings: string[];
}

/** Mirrors library_core::metadata::GameMetadata. */
export interface GameMetadata {
  provider: "steam" | "igdb" | string;
  sourceUrl: string | null;
  summary: string | null;
  description: string | null;
  genres: string[];
  features: string[];
  developers: string[];
  publishers: string[];
  releaseDate: string | null;
  platforms: string[];
  rating: { score: number; source: string; url: string | null } | null;
  website: string | null;
  coverUrl: string | null;
  screenshots: { thumbnail: string; full: string }[];
}

/** Mirrors library_core::metadata::CachedMetadata. `metadata` is null when nothing knew the game. */
export interface CachedMetadata {
  metadata: GameMetadata | null;
  fetchedAt: number;
}

/** Mirrors MetadataSettings in src-tauri/src/lib.rs. */
export interface MetadataSettings {
  igdbClientId: string | null;
}

/** Mirrors SteamSettings in src-tauri/src/lib.rs. */
export interface SteamSettings {
  steamId: string | null;
}
