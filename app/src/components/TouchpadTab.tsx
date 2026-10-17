import { useEffect, useRef } from "react";
import { setupCanvas, themeColors, type History, type TouchPoint } from "../history";
import { useI18n } from "../i18n";
import type { ControllerView, Touchpad } from "../types";

export function TouchpadTab({ view, history, onClear }: { view: ControllerView; history: History; onClear: () => void }) {
  const i18n = useI18n();
  const canvas = useRef<HTMLCanvasElement>(null);
  const touchpad = view.state.touchpad;

  useEffect(() => {
    if (canvas.current && touchpad) drawTouchpad(canvas.current, touchpad, history.touch);
  });

  if (!view.info.capabilities.touchpad) return <p className="muted">{i18n.t("noTouchpad")}</p>;
  if (!touchpad) return <p className="muted">{i18n.t("touchpadReduced")}</p>;

  return (
    <div className="touchpad">
      <div className="toolbar">
        <span className={touchpad.clicked ? "pill on" : "pill"}>{i18n.t("touchpadClick")}</span>
        <button onClick={onClear}>{i18n.t("clearTrace")}</button>
      </div>
      <canvas
        ref={canvas}
        className={touchpad.clicked ? "touch-canvas clicked" : "touch-canvas"}
        style={{ aspectRatio: `${touchpad.width} / ${touchpad.height}` }}
      />
      <dl className="facts fingers">
        {touchpad.fingers.map((finger, i) => (
          <div key={i} className={`finger f${i}`}>
            <dt>
              {i18n.t("finger")} {i + 1}
            </dt>
            <dd className="mono">
              {finger.active ? `#${finger.id} · ${finger.x} × ${finger.y}` : i18n.t("inactive")}
            </dd>
          </div>
        ))}
      </dl>
    </div>
  );
}

function drawTouchpad(canvas: HTMLCanvasElement, touchpad: Touchpad, traces: [TouchPoint[], TouchPoint[]]) {
  const { ctx, width, height } = setupCanvas(canvas);
  if (!ctx) return;
  const colors = themeColors();
  const sx = width / touchpad.width;
  const sy = height / touchpad.height;

  // Repère : moitiés gauche / droite (le pavé est souvent utilisé comme deux zones).
  ctx.strokeStyle = colors.border;
  ctx.setLineDash([4, 6]);
  ctx.beginPath();
  ctx.moveTo(width / 2, 0);
  ctx.lineTo(width / 2, height);
  ctx.stroke();
  ctx.setLineDash([]);

  traces.forEach((trace, i) => {
    ctx.strokeStyle = colors.series[i];
    ctx.lineWidth = 2;
    ctx.globalAlpha = 0.5;
    ctx.beginPath();
    let previous: TouchPoint | null = null;
    for (const p of trace) {
      // Nouveau contact (id différent) : on ne relie pas au tracé précédent.
      if (previous && previous.id === p.id) ctx.lineTo(p.x * sx, p.y * sy);
      else ctx.moveTo(p.x * sx, p.y * sy);
      previous = p;
    }
    ctx.stroke();
    ctx.globalAlpha = 1;
  });

  touchpad.fingers.forEach((finger, i) => {
    if (!finger.active) return;
    const x = finger.x * sx;
    const y = finger.y * sy;
    ctx.fillStyle = colors.series[i];
    ctx.beginPath();
    ctx.arc(x, y, 14, 0, Math.PI * 2);
    ctx.fill();
    ctx.fillStyle = "#fff";
    ctx.font = "bold 12px system-ui, sans-serif";
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.fillText(String(i + 1), x, y);
  });
}
