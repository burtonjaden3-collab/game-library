import { type FormEvent, useEffect, useState } from "react";
import { metadataSettings, openExternal, setIgdbCredentials } from "./api";
import { CloseIcon, ExternalIcon } from "./icons";
import type { MetadataSettings } from "./types";

/** App settings. For now: the optional IGDB credentials used for game info. */
export function Settings({ onClose, onSaved }: { onClose: () => void; onSaved: () => void }) {
  const [settings, setSettings] = useState<MetadataSettings | null>(null);
  const [clientId, setClientId] = useState("");
  const [clientSecret, setClientSecret] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    metadataSettings()
      .then((s) => {
        setSettings(s);
        setClientId(s.igdbClientId ?? "");
      })
      .catch((e) => setError(String(e)));
  }, []);

  useEffect(() => {
    // Capture phase, so Escape closes this dialog without also leaving a game page.
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      onClose();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [onClose]);

  const save = async (id: string, secret: string, done: string) => {
    setSaving(true);
    setError(null);
    setNotice(null);
    try {
      const s = await setIgdbCredentials(id, secret);
      setSettings(s);
      setClientId(s.igdbClientId ?? "");
      setClientSecret("");
      setNotice(done);
      onSaved();
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  const submit = (e: FormEvent) => {
    e.preventDefault();
    void save(clientId, clientSecret, "Connected. Games without info will be looked up in IGDB.");
  };

  const connected = Boolean(settings?.igdbClientId);

  return (
    <div className="modal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="modal" role="dialog" aria-modal="true" aria-labelledby="settings-title">
        <header className="modal-head">
          <h2 id="settings-title">Settings</h2>
          <button className="icon-btn" onClick={onClose} aria-label="Close">
            <CloseIcon />
          </button>
        </header>

        <section className="modal-section">
          <h3>Game info</h3>
          <p className="muted">
            Descriptions, genres, developers, release dates and screenshots come from Steam's store
            for Steam games. For games from other stores, and Steam games the store no longer lists,
            connect IGDB with your own free Twitch developer app.
          </p>
          <ol className="steps muted">
            <li>
              Sign in at{" "}
              <button
                className="link"
                onClick={() => openExternal("https://dev.twitch.tv/console/apps")}
              >
                dev.twitch.tv/console/apps <ExternalIcon />
              </button>{" "}
              and register an application.
            </li>
            <li>
              Use <code>http://localhost</code> as the OAuth redirect URL and pick Confidential as
              the client type.
            </li>
            <li>Paste its client ID and a new client secret below.</li>
          </ol>

          <form className="form" onSubmit={submit}>
            <label>
              <span>Client ID</span>
              <input
                value={clientId}
                onChange={(e) => setClientId(e.target.value)}
                autoComplete="off"
                spellCheck={false}
              />
            </label>
            <label>
              <span>Client secret</span>
              <input
                type="password"
                value={clientSecret}
                onChange={(e) => setClientSecret(e.target.value)}
                placeholder={connected ? "Saved. Enter a new one to replace it" : ""}
                autoComplete="off"
                spellCheck={false}
              />
            </label>
            <p className="muted small">Stored only in this computer's library database.</p>
            {error && <p className="about-error">{error}</p>}
            {notice && <p className="form-ok">{notice}</p>}
            <div className="form-actions">
              {connected && (
                <button
                  type="button"
                  className="btn-secondary"
                  disabled={saving}
                  onClick={() => void save("", "", "IGDB is off.")}
                >
                  Disconnect
                </button>
              )}
              <button
                type="submit"
                className="btn-play"
                disabled={saving || !clientId.trim() || !clientSecret.trim()}
              >
                {saving ? "Checking…" : connected ? "Update" : "Connect IGDB"}
              </button>
            </div>
          </form>
        </section>
      </div>
    </div>
  );
}
