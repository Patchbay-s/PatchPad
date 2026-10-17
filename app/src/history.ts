// Historiques côté frontend (traces, nuages de points, courbes), alimentés à chaque instantané.
// Gardés hors de l'état React : seules les vues qui dessinent sur canvas les lisent.

import type { ControllerView, Stick } from "./types";

const STICK_TRACE = 90;
const STICK_CLOUD = 3000;
const TOUCH_TRACE = 240;
const IMU_SAMPLES = 360;

export interface TouchPoint { x: number; y: number; id: number }

export interface ImuSample { gyro: [number, number, number]; accel: [number, number, number] }

export interface History {
  leftTrace: Stick[];
  rightTrace: Stick[];
  leftCloud: Stick[];
  rightCloud: Stick[];
  touch: [TouchPoint[], TouchPoint[]];
  imu: ImuSample[];
}

function push<T>(buffer: T[], value: T, max: number) {
  buffer.push(value);
  if (buffer.length > max) buffer.splice(0, buffer.length - max);
}

function emptyHistory(): History {
  return { leftTrace: [], rightTrace: [], leftCloud: [], rightCloud: [], touch: [[], []], imu: [] };
}

export class HistoryStore {
  private byId = new Map<string, History>();

  get(id: string): History {
    let history = this.byId.get(id);
    if (!history) {
      history = emptyHistory();
      this.byId.set(id, history);
    }
    return history;
  }

  record(views: ControllerView[]) {
    const present = new Set(views.map((v) => v.info.id));
    for (const id of this.byId.keys()) if (!present.has(id)) this.byId.delete(id);

    for (const view of views) {
      const h = this.get(view.info.id);
      const { leftStick, rightStick, touchpad, imu } = view.state;
      push(h.leftTrace, leftStick, STICK_TRACE);
      push(h.rightTrace, rightStick, STICK_TRACE);
      push(h.leftCloud, leftStick, STICK_CLOUD);
      push(h.rightCloud, rightStick, STICK_CLOUD);
      if (touchpad) {
        touchpad.fingers.forEach((finger, i) => {
          if (finger.active) push(h.touch[i], { x: finger.x, y: finger.y, id: finger.id }, TOUCH_TRACE);
        });
      }
      if (imu) push(h.imu, { gyro: imu.gyro, accel: imu.accel }, IMU_SAMPLES);
    }
  }

  clearSticks(id: string) {
    const h = this.get(id);
    h.leftTrace = [];
    h.rightTrace = [];
    h.leftCloud = [];
    h.rightCloud = [];
  }

  clearTouch(id: string) {
    this.get(id).touch = [[], []];
  }
}

/** Couleurs du thème courant, lues depuis les variables CSS (pour les canvas). */
export function themeColors() {
  const style = getComputedStyle(document.documentElement);
  const v = (name: string) => style.getPropertyValue(name).trim();
  return {
    fg: v("--fg"),
    muted: v("--muted"),
    border: v("--border"),
    bgAlt: v("--bg-alt"),
    accent: v("--accent"),
    ok: v("--ok"),
    warn: v("--warn"),
    bad: v("--bad"),
    series: [v("--series-1"), v("--series-2"), v("--series-3")],
  };
}

/** Prépare un canvas net sur écran haute densité ; renvoie le contexte et la taille CSS. */
export function setupCanvas(canvas: HTMLCanvasElement) {
  const ratio = window.devicePixelRatio || 1;
  const { width, height } = canvas.getBoundingClientRect();
  if (canvas.width !== Math.round(width * ratio) || canvas.height !== Math.round(height * ratio)) {
    canvas.width = Math.round(width * ratio);
    canvas.height = Math.round(height * ratio);
  }
  const ctx = canvas.getContext("2d");
  ctx?.setTransform(ratio, 0, 0, ratio, 0, 0);
  ctx?.clearRect(0, 0, width, height);
  return { ctx, width, height };
}
