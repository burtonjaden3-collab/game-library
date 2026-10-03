import { useCallback, useState } from "react";
import { Art } from "./Art";
import { artFor, generatedPalette } from "./gameArt";
import type { Game } from "./types";

/**
 * Portrait box art. Falls back to the landscape header (centered over a blurred copy of
 * itself), then to a generated cover built from the title.
 */
export function Cover({ game }: { game: Game }) {
  const art = artFor(game);
  const [stage, setStage] = useState<"portrait" | "landscape" | "generated">("portrait");
  const toLandscape = useCallback(() => setStage("landscape"), []);
  const toGenerated = useCallback(() => setStage("generated"), []);

  return (
    <div className="cover" style={generatedPalette(game.title)}>
      <div className="cover-generated" aria-hidden>
        <span className="cover-monogram">{game.title.trim().charAt(0)}</span>
        <span className="cover-title">{game.title}</span>
      </div>
      {stage === "portrait" && (
        <Art className="cover-img" sources={art.portrait} onExhausted={toLandscape} />
      )}
      {stage === "landscape" && (
        <>
          <Art className="cover-img blurred" sources={art.landscape} onExhausted={toGenerated} />
          <Art className="cover-img contained" sources={art.landscape} />
        </>
      )}
    </div>
  );
}
