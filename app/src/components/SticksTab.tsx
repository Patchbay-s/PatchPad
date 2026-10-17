import { useEffect, useRef, useState } from "react";
import { api, errorMessage } from "../api";
import { setupCanvas, themeColors, type History } from "../history";
import { useI18n } from "../i18n";
import type { ControllerView, Stick, StickReport, TriggerReport } from "../types";

export function SticksTab({
  view,
  history,
  threshold,
  onClear,
}: {
  view: ControllerView;
  history: History;
  threshold: number;
  onClear: () => void;
}) {
  const i18n = useI18n();
  const [error, setError] = useState<string | null>(null);
  const run = (p: Promise<void>) => p.then(() => setError(null)).catch((e) => setError(errorMessage(e)));

  return (
    <div className="sticks">
      <div className="toolbar">
        <label className="threshold">
          {i18n.t("driftThreshold")}
          <input
            type="range"
            min={0.01}
            max={0.3}
            step={0.01}
            value={threshold}
            onChange={(e) => run(api.setDriftThreshold(Number(e.target.value)))}
          />
          <span className="mono">{threshold.toFixed(2)}</span>
        </label>
        <button onClick={onClear}>{i18n.t("clearTrace")}</button>
        <button
          onClick={() => {
            onClear();
            run(api.resetAnalysis(view.info.id));
          }}
        >
          {i18n.t("resetAnalysis")}
        </button>
        {error && <span className="error">{error}</span>}
      </div>
      <div className="stick-panels">
        <StickPanel
          title={i18n.t("leftStick")}
          stick={view.state.leftStick}
          report={view.leftStick}
          cloud={history.leftCloud}
          threshold={threshold}
        />
        <StickPanel
          title={i18n.t("rightStick")}
          stick={view.state.rightStick}
          report={view.rightStick}
          cloud={history.rightCloud}
          threshold={threshold}
        />
      </div>
      <div className="trigger-panels">
        <TriggerPanel title={i18n.t("leftTrigger")} value={view.state.leftTrigger} report={view.leftTrigger} />
        <TriggerPanel title={i18n.t("rightTrigger")} value={view.state.rightTrigger} report={view.rightTrigger} />
      </div>
    </div>
  );
}

function StickPanel({
  title,
  stick,
  report,
  cloud,
  threshold,
}: {
  title: string;
  stick: Stick;
  report: StickReport;
  cloud: Stick[];
  threshold: number;
}) {
  const i18n = useI18n();
  const canvas = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    if (canvas.current) drawStick(canvas.current, stick, report, cloud, threshold);
  });

  const verdict =
    report.drift === "drift" ? i18n.t("driftDetected") : report.drift === "ok" ? i18n.t("driftOk") : i18n.t("driftUnknown");
  const fmt = (n: number) => (n >= 0 ? "+" : "") + n.toFixed(3);

  return (
    <div className="panel">
      <h3>
        {title} <span className="muted small">{report.atRest ? i18n.t("atRest") : i18n.t("moving")}</span>
      </h3>
      <canvas ref={canvas} className="stick-canvas" />
      <dl className="facts">
        <dt>{i18n.t("drift")}</dt>
        <dd className={`verdict ${report.drift}`}>{verdict}</dd>
        <dt>{i18n.t("rest")}</dt>
        <dd className="mono">{report.rest ? `${fmt(report.rest.x)} ${fmt(report.rest.y)}` : i18n.t("restUnknown")}</dd>
        <dt>{i18n.t("restOffset")}</dt>
        <dd className="mono">{report.restOffset !== null ? report.restOffset.toFixed(3) : "—"}</dd>
        <dt>{i18n.t("suggestedDeadzone")}</dt>
        <dd className="mono">{report.suggestedDeadzone !== null ? report.suggestedDeadzone.toFixed(2) : "—"}</dd>
        <dt>{i18n.t("circularity")}</dt>
        <dd>
          {report.circularityError !== null ? (
            <span className="mono">
              {i18n.t("avgError")} {report.circularityError.toFixed(1)} %
            </span>
          ) : (
            <span className="muted">{i18n.t("circularityHint")}</span>
          )}{" "}
          <span className="muted">
            ({i18n.t("coverage")} {Math.round(report.coverage * 100)} %)
          </span>
        </dd>
        <dt>{i18n.t("maxRadius")}</dt>
        <dd className="mono">{report.maxRadius.toFixed(3)}</dd>
      </dl>
    </div>
  );
}

function drawStick(canvas: HTMLCanvasElement, stick: Stick, report: StickReport, cloud: Stick[], threshold: number) {
  const { ctx, width, height } = setupCanvas(canvas);
  if (!ctx) return;
  const colors = themeColors();
  const size = Math.min(width, height);
  const cx = width / 2;
  const cy = height / 2;
  const radius = size / 2 - 14;
  const px = (s: Stick) => [cx + s.x * radius, cy - s.y * radius] as const;

  // Repères : cercle unité, axes.
  ctx.strokeStyle = colors.border;
  ctx.lineWidth = 1;
  ctx.beginPath();
  ctx.arc(cx, cy, radius, 0, Math.PI * 2);
  ctx.moveTo(cx - radius, cy);
  ctx.lineTo(cx + radius, cy);
  ctx.moveTo(cx, cy - radius);
  ctx.lineTo(cx, cy + radius);
  ctx.stroke();

  // Seuil de drift (pointillés) et zone morte conseillée.
  ctx.setLineDash([4, 4]);
  ctx.strokeStyle = colors.muted;
  ctx.beginPath();
  ctx.arc(cx, cy, threshold * radius, 0, Math.PI * 2);
  ctx.stroke();
  ctx.setLineDash([]);
  if (report.suggestedDeadzone !== null) {
    ctx.strokeStyle = colors.warn;
    ctx.beginPath();
    ctx.arc(cx, cy, report.suggestedDeadzone * radius, 0, Math.PI * 2);
    ctx.stroke();
  }

  // Nuage de points.
  ctx.fillStyle = colors.accent;
  ctx.globalAlpha = 0.25;
  for (const s of cloud) {
    const [x, y] = px(s);
    ctx.fillRect(x - 1, y - 1, 2, 2);
  }
  ctx.globalAlpha = 1;

  // Contour mesuré (rayon max par secteur).
  const n = report.sectors.length;
  if (n > 0) {
    ctx.strokeStyle = colors.ok;
    ctx.lineWidth = 2;
    ctx.beginPath();
    let drawing = false;
    for (let i = 0; i <= n; i++) {
      const r = report.sectors[i % n];
      if (r <= 0) {
        drawing = false;
        continue;
      }
      const angle = ((i % n) + 0.5) * ((Math.PI * 2) / n);
      const [x, y] = px({ x: Math.cos(angle) * r, y: Math.sin(angle) * r });
      if (drawing) ctx.lineTo(x, y);
      else ctx.moveTo(x, y);
      drawing = true;
    }
    ctx.stroke();
  }

  // Position de repos.
  if (report.rest) {
    const [x, y] = px(report.rest);
    ctx.strokeStyle = report.drift === "drift" ? colors.bad : colors.ok;
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.moveTo(x - 6, y - 6);
    ctx.lineTo(x + 6, y + 6);
    ctx.moveTo(x + 6, y - 6);
    ctx.lineTo(x - 6, y + 6);
    ctx.stroke();
  }

  // Position actuelle.
  const [x, y] = px(stick);
  ctx.fillStyle = colors.fg;
  ctx.beginPath();
  ctx.arc(x, y, 5, 0, Math.PI * 2);
  ctx.fill();
}

function TriggerPanel({ title, value, report }: { title: string; value: number; report: TriggerReport | null }) {
  const i18n = useI18n();
  const yesNo = (b: boolean) => (b ? i18n.t("yes") : i18n.t("no"));
  return (
    <div className="panel">
      <h3>{title}</h3>
      <div className="bar" aria-valuenow={value} role="meter" aria-valuemin={0} aria-valuemax={1}>
        {report && (
          <div className="bar-range" style={{ left: `${report.min * 100}%`, width: `${(report.max - report.min) * 100}%` }} />
        )}
        <div className="bar-fill" style={{ width: `${value * 100}%` }} />
      </div>
      <dl className="facts">
        <dt>{i18n.t("triggerRange")}</dt>
        <dd className="mono">{report ? `${report.min.toFixed(3)} – ${report.max.toFixed(3)}` : "—"}</dd>
        <dt>{i18n.t("reachesZero")}</dt>
        <dd className={report?.reachesZero ? "verdict ok" : "verdict unknown"}>{report ? yesNo(report.reachesZero) : "—"}</dd>
        <dt>{i18n.t("reachesFull")}</dt>
        <dd className={report?.reachesFull ? "verdict ok" : "verdict unknown"}>{report ? yesNo(report.reachesFull) : "—"}</dd>
      </dl>
    </div>
  );
}
