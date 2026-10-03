import { useCallback, useEffect, useState } from "react";
import { gameMetadata } from "./api";
import type { CachedMetadata } from "./types";

export interface MetadataState {
  /** `undefined` until the first answer arrives. `null` metadata inside means "not found". */
  cached: CachedMetadata | null | undefined;
  loading: boolean;
  error: string | null;
  refresh: () => void;
}

/**
 * Loads a game's metadata from the cache, fetching it online when missing or stale.
 * `version` forces a reload, e.g. after a new metadata provider was switched on.
 * Callers key the component by game so state never leaks from one game to the next.
 */
export function useGameMetadata(gameId: number, version: number): MetadataState {
  const [cached, setCached] = useState<CachedMetadata | null | undefined>(undefined);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [force, setForce] = useState(0);

  useEffect(() => {
    let live = true;
    setLoading(true);
    setError(null);
    gameMetadata(gameId, force > 0)
      .then((result) => live && setCached(result))
      .catch((e) => live && setError(String(e)))
      .finally(() => live && setLoading(false));
    return () => {
      live = false;
    };
  }, [gameId, version, force]);

  const refresh = useCallback(() => setForce((n) => n + 1), []);
  return { cached, loading, error, refresh };
}
