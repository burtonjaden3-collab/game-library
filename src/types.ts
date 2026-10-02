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
}

/** Result of one store import. Mirrors ImportReport in src-tauri/src/lib.rs. */
export interface ImportReport {
  libraries: string[];
  found: number;
  added: number;
  updated: number;
  uninstalled: number;
  warnings: string[];
}
