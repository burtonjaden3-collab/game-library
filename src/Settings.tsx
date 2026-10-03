import { type FormEvent, type ReactNode, useEffect, useState } from "react";
import {
  metadataSettings,
  openExternal,
  setIgdbCredentials,
  setSteamAccount,
  steamSettings,
} from "./api";
import { CloseIcon, ExternalIcon } from "./icons";
import type { MetadataSettings, SteamSettings } from "./types";

/** App settings: the Steam account to import owned games from, and the optional IGDB
 * credentials used for game info. */
export function Settings({
  onClose,
  onSaved,
  onSteamConnected,
}: {
  onClose: () => void;
  onSaved: () => void;
  onSteamConnected: () => void;
}) {
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

  return (
    <div className="modal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="modal" role="dialog" aria-modal="true" aria-labelledby="settings-title">
        <header className="modal-head">
          <h2 id="settings-title">Settings</h2>
          <button className="icon-btn" onClick={onClose} aria-label="Close">
            <CloseIcon />
          </button>
        </header>
        <SteamSection onConnected={onSteamConnected} />
        <IgdbSection onSaved={onSaved} />
      </div>
    </div>
  );
}

/** Tracks one settings form's save: busy flag, error and success notice. */
function useSave() {
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const run = async (action: () => Promise<void>, done: string) => {
    setSaving(true);
    setError(null);
    setNotice(null);
    try {
      await action();
      setNotice(done);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };
  return { saving, error, notice, setError, run };
}

function ExternalLink({ url, children }: { url: string; children: ReactNode }) {
  return (
    <button className="link" onClick={() => openExternal(url)}>
      {children} <ExternalIcon />
    </button>
  );
}

function SteamSection({ onConnected }: { onConnected: () => void }) {
  const [settings, setSettings] = useState<SteamSettings | null>(null);
  const [apiKey, setApiKey] = useState("");
  const [profile, setProfile] = useState("");
  const { saving, error, notice, setError, run } = useSave();

  useEffect(() => {
    steamSettings()
      .then((s) => {
        setSettings(s);
        setProfile(s.steamId ?? "");
      })
      .catch((e) => setError(String(e)));
  }, [setError]);

  const connected = Boolean(settings?.steamId);

  const save = (key: string, prof: string, done: string, after?: () => void) =>
    run(async () => {
      const s = await setSteamAccount(key, prof);
      setSettings(s);
      setProfile(s.steamId ?? "");
      setApiKey("");
      after?.();
    }, done);

  const submit = (e: FormEvent) => {
    e.preventDefault();
    void save(apiKey, profile, "Connected. Importing your Steam games…", onConnected);
  };

  return (
    <section className="modal-section">
      <h3>Steam account</h3>
      <p className="muted">
        Connect your account to import every game you own on Steam, including ones that aren't
        installed, along with your playtime.
      </p>
      <ol className="steps muted">
        <li>
          Get a Web API key at{" "}
          <ExternalLink url="https://steamcommunity.com/dev/apikey">
            steamcommunity.com/dev/apikey
          </ExternalLink>
          . Any domain name works, for example <code>localhost</code>.
        </li>
        <li>
          Paste your profile URL, like <code>steamcommunity.com/id/yourname</code>, or your
          SteamID64.
        </li>
        <li>
          If no games show up, set Game details to Public in your{" "}
          <ExternalLink url="https://steamcommunity.com/my/edit/settings">
            Steam privacy settings
          </ExternalLink>
          .
        </li>
      </ol>

      <form className="form" onSubmit={submit}>
        <label>
          <span>Profile</span>
          <input
            value={profile}
            onChange={(e) => setProfile(e.target.value)}
            placeholder="https://steamcommunity.com/id/yourname"
            autoComplete="off"
            spellCheck={false}
          />
        </label>
        <label>
          <span>Web API key</span>
          <input
            type="password"
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
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
              onClick={() => void save("", "", "Steam account disconnected.")}
            >
              Disconnect
            </button>
          )}
          <button
            type="submit"
            className="btn-play"
            disabled={saving || !profile.trim() || (!connected && !apiKey.trim())}
          >
            {saving ? "Checking…" : connected ? "Update" : "Connect Steam"}
          </button>
        </div>
      </form>
    </section>
  );
}

function IgdbSection({ onSaved }: { onSaved: () => void }) {
  const [settings, setSettings] = useState<MetadataSettings | null>(null);
  const [clientId, setClientId] = useState("");
  const [clientSecret, setClientSecret] = useState("");
  const { saving, error, notice, setError, run } = useSave();

  useEffect(() => {
    metadataSettings()
      .then((s) => {
        setSettings(s);
        setClientId(s.igdbClientId ?? "");
      })
      .catch((e) => setError(String(e)));
  }, [setError]);

  const save = (id: string, secret: string, done: string) =>
    run(async () => {
      const s = await setIgdbCredentials(id, secret);
      setSettings(s);
      setClientId(s.igdbClientId ?? "");
      setClientSecret("");
      onSaved();
    }, done);

  const submit = (e: FormEvent) => {
    e.preventDefault();
    void save(clientId, clientSecret, "Connected. Games without info will be looked up in IGDB.");
  };

  const connected = Boolean(settings?.igdbClientId);

  return (
    <section className="modal-section">
      <h3>Game info</h3>
      <p className="muted">
        Descriptions, genres, developers, release dates and screenshots come from Steam's store for
        Steam games. For games from other stores, and Steam games the store no longer lists, connect
        IGDB with your own free Twitch developer app.
      </p>
      <ol className="steps muted">
        <li>
          Sign in at{" "}
          <ExternalLink url="https://dev.twitch.tv/console/apps">
            dev.twitch.tv/console/apps
          </ExternalLink>{" "}
          and register an application.
        </li>
        <li>
          Use <code>http://localhost</code> as the OAuth redirect URL and pick Confidential as the
          client type.
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
  );
}
