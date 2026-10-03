import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { CachedMetadata, Game, ImportReport, MetadataSettings } from "./types";

export const listGames = () => invoke<Game[]>("list_games");
export const importSteam = () => invoke<ImportReport>("import_steam");
export const launchGame = (id: number) => invoke<void>("launch_game", { id });

/** Cached metadata while fresh, otherwise looked up online. `refresh` forces a lookup. */
export const gameMetadata = (id: number, refresh = false) =>
  invoke<CachedMetadata | null>("game_metadata", { id, refresh });
export const metadataSettings = () => invoke<MetadataSettings>("metadata_settings");
/** Checks and saves IGDB credentials; two empty strings switch IGDB off. */
export const setIgdbCredentials = (clientId: string, clientSecret: string) =>
  invoke<MetadataSettings>("set_igdb_credentials", { clientId, clientSecret });

/** Opens a web page in the user's browser. */
export const openExternal = (url: string) => openUrl(url);
