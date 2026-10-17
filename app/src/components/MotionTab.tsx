import { useEffect, useRef, useState } from "react";
import { api, errorMessage } from "../api";
import { setupCanvas, themeColors, type History, type ImuSample } from "../history";
import { useI18n } from "../i18n";
import type { ControllerView, Orientation } from "../types";

const AXES = ["X", "Y", "Z"];

export function MotionTab({ view, history }: { view: ControllerView; history: History }) {
  const i18n = useI18n();
  const gyroCanvas = useRef<HTMLCanvasElement>(null);
  const accelCanvas = useRef<HTMLCanvasElement>(null);
  const [error, setError] = useState<string | null>(null);
  const imu = view.state.imu;

  useEffect(() => {
    if (gyroCanvas.current) drawSeries(gyroCanvas.current, history.imu, (s) => s.gyro, 250);
    if (accelCanvas.current) drawSeries(accelCanvas.current, history.imu, (s) => s.accel, 2);
  });

  if (!view.info.capabilities.imu) return <p className="muted">{i18n.t("noImu")}</p>;
  if (!imu) return <p className="muted">{i18n.t("imuReduced")}</p>;

  const fmt = (n: number, digits: number) => (n >= 0 ? "+" : "") + n.toFixed(digits);

  return (
    <div className="motion">
      <div className="toolbar">
        <button
          disabled={view.imuZeroing}
          title={i18n.t("recalibrateHint")}
          onClick={() =>
            api
              .recalibrateImu(view.info.id)
              .then(() => setError(null))
              .catch((e) => setError(errorMessage(e)))
          }
        >
          {i18n.t("recalibrate")}
        </button>
        {view.imuZeroing && <span className="warn-text">{i18n.t("recalibrating")}</span>}
        {view.info.imuFactoryCalibration === false && <span className="muted">{i18n.t("nominalCalibration")}</span>}
        {error && <span className="error">{error}</span>}
      </div>
      <div className="motion-grid">
        <div className="panel">
          <h3>{i18n.t("gyro")}</h3>
          <canvas ref={gyroCanvas} className="series-canvas" />
          <Legend values={imu.gyro.map((v) => fmt(v, 1))} />
        </div>
        <div className="panel">
          <h3>{i18n.t("accel")}</h3>
          <canvas ref={accelCanvas} className="series-canvas" />
          <Legend values={imu.accel.map((v) => fmt(v, 3))} />
        </div>
        <div className="panel">
          <h3>{i18n.t("orientation")}</h3>
          {view.orientation && <Attitude orientation={view.orientation} />}
          {view.orientation && (
            <dl className="facts">
              <dt>{i18n.t("pitch")}</dt>
              <dd className="mono">{fmt(view.orientation.pitch, 1)}°</dd>
              <dt>{i18n.t("roll")}</dt>
              <dd className="mono">{fmt(view.orientation.roll, 1)}°</dd>
              <dt>{i18n.t("yaw")}</dt>
              <dd className="mono">{fmt(view.orientation.yaw, 1)}°</dd>
            </dl>
          )}
        </div>
      </div>
    </div>
  );
}

function Legend({ values }: { values: string[] }) {
  return (
    <div className="legend">
      {values.map((v, i) => (
        <span key={i} className={`legend-item s${i}`}>
          <i /> {AXES[i]} <span className="mono">{v}</span>
        </span>
      ))}
    </div>
  );
}

/** Courbes défilantes, échelle symétrique qui s'élargit si les valeurs dépassent `minRange`. */
function drawSeries(
  canvas: HTMLCanvasElement,
  samples: ImuSample[],
  pick: (s: ImuSample) => [number, number, number],
  minRange: number,
) {
  const { ctx, width, height } = setupCanvas(canvas);
  if (!ctx) return;
  const colors = themeColors();
  let range = minRange;
  for (const s of samples) for (const v of pick(s)) range = Math.max(range, Math.abs(v));
  const y = (v: number) => height / 2 - (v / range) * (height / 2 - 6);

  ctx.strokeStyle = colors.border;
  ctx.lineWidth = 1;
  ctx.beginPath();
  ctx.moveTo(0, height / 2);
  ctx.lineTo(width, height / 2);
  ctx.stroke();
  ctx.fillStyle = colors.muted;
  ctx.font = "11px system-ui, sans-serif";
  ctx.textBaseline = "top";
  ctx.fillText(`±${range < 10 ? range.toFixed(1) : range.toFixed(0)}`, 4, 2);

  const capacity = 360;
  const step = width / (capacity - 1);
  const offset = capacity - samples.length;
  for (let axis = 0; axis < 3; axis++) {
    ctx.strokeStyle = colors.series[axis];
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    samples.forEach((s, i) => {
      const x = (offset + i) * step;
      const value = pick(s)[axis];
      if (i === 0) ctx.moveTo(x, y(value));
      else ctx.lineTo(x, y(value));
    });
    ctx.stroke();
  }
}

/** Indicateur d'orientation simple : horizon artificiel (tangage, roulis) et cap (lacet). */
function Attitude({ orientation }: { orientation: Orientation }) {
  const pitchOffset = Math.max(-60, Math.min(60, orientation.pitch)) * 0.8;
  return (
    <svg className="attitude" viewBox="-60 -60 120 120" role="img" aria-label="attitude">
      <defs>
        <clipPath id="attitude-clip">
          <circle r={50} />
        </clipPath>
      </defs>
      <g clipPath="url(#attitude-clip)">
        <g transform={`rotate(${orientation.roll}) translate(0 ${pitchOffset})`}>
          <rect className="att-sky" x={-120} y={-120} width={240} height={120} />
          <rect className="att-ground" x={-120} y={0} width={240} height={120} />
          <line className="att-horizon" x1={-120} y1={0} x2={120} y2={0} />
        </g>
      </g>
      <circle className="att-ring" r={50} />
      <path className="att-plane" d="M-28 0 H-8 L0 6 L8 0 H28" />
      <g transform={`rotate(${-orientation.yaw})`}>
        <path className="att-heading" d="M0 -58 L4 -50 L-4 -50 Z" />
      </g>
    </svg>
  );
}
