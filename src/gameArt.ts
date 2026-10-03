import type { Game } from "./types";
import type { CSSProperties } from "react";

const steamApp = (appId: string, file: string) => [
  `https://cdn.cloudflare.steamstatic.com/steam/apps/${appId}/${file}`,
  `https://shared.cloudflare.steamstatic.com/store_item_assets/steam/apps/${appId}/${file}`,
];

/** Candidate image URLs for a piece of art, tried in order until one loads. */
export interface ArtSources {
  portrait: string[];
  landscape: string[];
  hero: string[];
  logo: string[];
}

export function artFor(game: Game): ArtSources {
  if (game.source !== "steam") return { portrait: [], landscape: [], hero: [], logo: [] };
  const id = game.sourceId;
  return {
    portrait: steamApp(id, "library_600x900.jpg"),
    landscape: steamApp(id, "header.jpg"),
    hero: steamApp(id, "library_hero.jpg"),
    logo: steamApp(id, "logo.png"),
  };
}

function hash(text: string) {
  let h = 2166136261;
  for (let i = 0; i < text.length; i++) {
    h ^= text.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return h >>> 0;
}

// Hand-picked gradient pairs so generated covers always look intentional.
const PALETTES: [string, string, string][] = [
  ["#5b5bd6", "#1b1840", "#8b8bff"],
  ["#c2410c", "#2a0f08", "#ff9a5c"],
  ["#0e7490", "#06222b", "#4fd3f0"],
  ["#be185d", "#2a0717", "#ff7ab6"],
  ["#15803d", "#071f12", "#5ee38f"],
  ["#7e22ce", "#1d0933", "#c38bff"],
  ["#b45309", "#2b1704", "#ffc56b"],
  ["#1d4ed8", "#0a1638", "#79a2ff"],
  ["#0f766e", "#05201d", "#4ee6d1"],
  ["#b91c1c", "#2a0909", "#ff7c7c"],
  ["#475569", "#12161d", "#a9b8cc"],
  ["#a21caf", "#25072a", "#f08bff"],
];

/** A stable palette per title, used when no cover art is available. */
export function generatedPalette(title: string) {
  const [a, b, glow] = PALETTES[hash(title) % PALETTES.length];
  return { "--gen-a": a, "--gen-b": b, "--gen-glow": `${glow}55` } as CSSProperties;
}

export function formatSize(bytes: number | null) {
  if (!bytes) return null;
  const gb = bytes / 1024 ** 3;
  return gb >= 1 ? `${gb.toFixed(1)} GB` : `${Math.max(1, Math.round(bytes / 1024 ** 2))} MB`;
}

export function formatDate(unixSeconds: number | null) {
  if (!unixSeconds) return null;
  return new Date(unixSeconds * 1000).toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

export const SOURCE_LABELS: Record<Game["source"], string> = {
  steam: "Steam",
  epic: "Epic Games",
  gog: "GOG",
  itch: "itch.io",
  manual: "Manual",
};
