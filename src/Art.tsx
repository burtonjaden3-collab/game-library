import { useEffect, useState } from "react";

/**
 * Shows the first of `sources` that loads. Renders nothing once every source has failed,
 * so whatever sits behind it (a generated cover) shows through.
 */
export function Art({
  sources,
  className,
  onExhausted,
}: {
  sources: string[];
  className?: string;
  onExhausted?: () => void;
}) {
  const [index, setIndex] = useState(0);
  const [loaded, setLoaded] = useState(false);
  const key = sources.join("|");

  useEffect(() => {
    setIndex(0);
    setLoaded(false);
  }, [key]);

  useEffect(() => {
    if (index >= sources.length) onExhausted?.();
  }, [index, sources.length, onExhausted]);

  if (index >= sources.length) return null;
  return (
    <img
      className={`${className ?? ""}${loaded ? " loaded" : ""}`}
      src={sources[index]}
      alt=""
      loading="lazy"
      draggable={false}
      onLoad={() => setLoaded(true)}
      onError={() => setIndex((i) => i + 1)}
    />
  );
}
