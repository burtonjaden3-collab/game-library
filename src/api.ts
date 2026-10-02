import { invoke } from "@tauri-apps/api/core";
import type { Game } from "./types";

export const listGames = () => invoke<Game[]>("list_games");
