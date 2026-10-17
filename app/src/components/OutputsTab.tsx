import { useEffect, useRef, useState } from "react";
import { api, errorMessage } from "../api";
import { useI18n, type MessageKey } from "../i18n";
import type { ControllerView, Support } from "../types";

const DURATIONS = [250, 500, 1000, 2000];
/** Délai minimal entre deux envois de couleur pendant le glissement du sélecteur. */
const LIGHTBAR_THROTTLE_MS = 50;

function unsupportedKey(support: Support): MessageKey | null {
  if (support === "unsupportedOs") return "rumbleUnsupportedOs";
  if (support === "noHardware") return "rumbleNoHardware";
  return null;
}

function hexToRgb(hex: string): [number, number, number] {
  const n = parseInt(hex.slice(1), 16);
  return [(n >> 16) & 0xff, (n >> 8) & 0xff, n & 0xff];
}

export function OutputsTab({ view }: { view: ControllerView }) {
  const i18n = useI18n();
  const [intensity, setIntensity] = useState(0.6);
  const [duration, setDuration] = useState(1000);
  const [color, setColor] = useState("#2563eb");
  const [error, setError] = useState<string | null>(null);
  const lastSent = useRef(0);
  const pending = useRef<number | null>(null);
  const { id, capabilities, family } = view.info;

  useEffect(() => () => {
    if (pending.current !== null) window.clearTimeout(pending.current);
  }, []);

  const rumbleBlocked = unsupportedKey(capabilities.rumble);
  const rumble = (strong: number, weak: number) =>
    api
      .rumble(id, strong, weak, duration)
      .then(() => setError(null))
      .catch((e) => setError(errorMessage(e)));

  const sendColor = (hex: string) =>
    api
      .setLightbar(id, hexToRgb(hex))
      .then(() => setError(null))
      .catch((e) => setError(errorMessage(e)));

  // Application en direct pendant le glissement, limitée en fréquence.
  const onColor = (hex: string) => {
    setColor(hex);
    if (pending.current !== null) window.clearTimeout(pending.current);
    const wait = Math.max(0, lastSent.current + LIGHTBAR_THROTTLE_MS - Date.now());
    pending.current = window.setTimeout(() => {
      pending.current = null;
      lastSent.current = Date.now();
      sendColor(hex);
    }, wait);
  };

  const tooltip = rumbleBlocked ? i18n.t(rumbleBlocked) : undefined;

  return (
    <div className="outputs">
      <div className="panel">
        <h3>{i18n.t("vibration")}</h3>
        {/* Infobulle aussi sur le conteneur : certains WebView n'en affichent pas sur un bouton désactivé. */}
        <div className="row" title={tooltip}>
          <button disabled={!!rumbleBlocked} title={tooltip} onClick={() => rumble(intensity, 0)}>
            {i18n.t("left")}
          </button>
          <button disabled={!!rumbleBlocked} title={tooltip} onClick={() => rumble(0, intensity)}>
            {i18n.t("right")}
          </button>
          <button disabled={!!rumbleBlocked} title={tooltip} onClick={() => rumble(intensity, intensity)}>
            {i18n.t("both")}
          </button>
        </div>
        <label className="row">
          {i18n.t("intensity")}
          <input
            type="range"
            min={0.05}
            max={1}
            step={0.05}
            value={intensity}
            disabled={!!rumbleBlocked}
            onChange={(e) => setIntensity(Number(e.target.value))}
          />
          <span className="mono">{Math.round(intensity * 100)} %</span>
        </label>
        <label className="row">
          {i18n.t("duration")}
          <select value={duration} disabled={!!rumbleBlocked} onChange={(e) => setDuration(Number(e.target.value))}>
            {DURATIONS.map((d) => (
              <option key={d} value={d}>
                {d} ms
              </option>
            ))}
          </select>
        </label>
        {rumbleBlocked && <p className="muted">{i18n.t(rumbleBlocked)}</p>}
        {family === "xbox" && <p className="muted small">{i18n.t("triggerMotorsNote")}</p>}
      </div>

      <div className="panel">
        <h3>{i18n.t("lightbar")}</h3>
        {capabilities.lightbar === "supported" ? (
          <div className="row">
            <input type="color" value={color} onChange={(e) => onColor(e.target.value)} />
            <span className="mono">{color}</span>
            <button onClick={() => sendColor(color)}>{i18n.t("apply")}</button>
          </div>
        ) : (
          <p className="muted">{i18n.t("lightbarNoHardware")}</p>
        )}
      </div>
      {error && <p className="error">{error}</p>}
    </div>
  );
}
