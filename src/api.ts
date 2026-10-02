import { invoke } from "@tauri-apps/api/core";
import type { Game, ImportReport } from "./types";

export const listGames = () => invoke<Game[]>("list_games");
export const importSteam = () => invoke<ImportReport>("import_steam");
export const launchGame = (id: number) => invoke<void>("launch_game", { id });
