// Schéma de manette générique dessiné à la main : ce n'est volontairement la reproduction
// d'aucun visuel Sony ou Microsoft. La disposition change seulement selon la présence d'un
// pavé tactile (DS4) ou non.

import { useI18n } from "../i18n";
import type { Button, ControllerView, Stick } from "../types";

const STICK_RANGE = 26;

function stickPoint(cx: number, cy: number, s: Stick) {
  return { x: cx + s.x * STICK_RANGE, y: cy - s.y * STICK_RANGE };
}

export function Schematic({ view, leftTrace, rightTrace }: { view: ControllerView; leftTrace: Stick[]; rightTrace: Stick[] }) {
  const i18n = useI18n();
  const { state, info } = view;
  const pressed = new Set<Button>(state.buttons);
  const label = (b: Button) => i18n.button(info.family, b);
  const on = (b: Button) => (pressed.has(b) ? "sch-btn on" : "sch-btn");
  const hasTouchpad = info.capabilities.touchpad;

  const centerY = hasTouchpad ? 150 : 165;
  const homePos = hasTouchpad ? { x: 320, y: 228 } : { x: 320, y: 125 };

  return (
    <svg className="schematic" viewBox="0 0 640 400" role="img" aria-label={info.name}>
      {/* Gâchettes analogiques */}
      <TriggerBar x={115} label={label("l2")} value={state.leftTrigger} active={pressed.has("l2")} />
      <TriggerBar x={415} label={label("r2")} value={state.rightTrigger} active={pressed.has("r2")} />

      {/* Boutons de tranche */}
      <rect className={on("l1")} x={115} y={62} width={110} height={22} rx={10} />
      <text className="sch-label" x={170} y={78}>{label("l1")}</text>
      <rect className={on("r1")} x={415} y={62} width={110} height={22} rx={10} />
      <text className="sch-label" x={470} y={78}>{label("r1")}</text>

      {/* Corps */}
      <path
        className="sch-body"
        d="M120 100 C 170 84 470 84 520 100 C 600 140 640 330 592 372 C 552 402 506 360 468 300 L 172 300 C 134 360 88 402 48 372 C 0 330 40 140 120 100 Z"
      />

      {/* Croix directionnelle */}
      <g>
        <rect className={on("dpadUp")} x={157} y={150} width={26} height={30} rx={4} />
        <rect className={on("dpadDown")} x={157} y={200} width={26} height={30} rx={4} />
        <rect className={on("dpadLeft")} x={125} y={177} width={30} height={26} rx={4} />
        <rect className={on("dpadRight")} x={185} y={177} width={30} height={26} rx={4} />
      </g>

      {/* Boutons de façade */}
      <FaceButton x={470} y={155} cls={on("north")} text={label("north")} />
      <FaceButton x={470} y={225} cls={on("south")} text={label("south")} />
      <FaceButton x={435} y={190} cls={on("west")} text={label("west")} />
      <FaceButton x={505} y={190} cls={on("east")} text={label("east")} />

      {/* Pavé tactile (DS4) */}
      {hasTouchpad && (
        <g>
          <rect
            className={state.touchpad?.clicked ? "sch-pad on" : "sch-pad"}
            x={250}
            y={100}
            width={140}
            height={70}
            rx={10}
          />
          {state.touchpad?.fingers.map((finger, i) =>
            finger.active ? (
              <circle
                key={i}
                className={`sch-finger f${i}`}
                cx={250 + (finger.x / state.touchpad!.width) * 140}
                cy={100 + (finger.y / state.touchpad!.height) * 70}
                r={6}
              />
            ) : null,
          )}
        </g>
      )}

      {/* Boutons centraux */}
      <rect className={on("select")} x={hasTouchpad ? 222 : 268} y={centerY - 10} width={34} height={20} rx={10} />
      <text className="sch-small" x={hasTouchpad ? 239 : 285} y={centerY + 26}>{label("select")}</text>
      <rect className={on("start")} x={hasTouchpad ? 384 : 338} y={centerY - 10} width={34} height={20} rx={10} />
      <text className="sch-small" x={hasTouchpad ? 401 : 355} y={centerY + 26}>{label("start")}</text>
      <circle className={on("home")} cx={homePos.x} cy={homePos.y} r={14} />
      <text className="sch-small" x={homePos.x} y={homePos.y + 30}>{label("home")}</text>

      <StickView cx={240} cy={262} stick={state.leftStick} trace={leftTrace} pressed={pressed.has("l3")} label={label("l3")} />
      <StickView cx={400} cy={262} stick={state.rightStick} trace={rightTrace} pressed={pressed.has("r3")} label={label("r3")} />
    </svg>
  );
}

function FaceButton({ x, y, cls, text }: { x: number; y: number; cls: string; text: string }) {
  return (
    <g>
      <circle className={cls} cx={x} cy={y} r={17} />
      <text className="sch-label" x={x} y={y + 5}>{text}</text>
    </g>
  );
}

function TriggerBar({ x, label, value, active }: { x: number; label: string; value: number; active: boolean }) {
  return (
    <g>
      <rect className="sch-trigger-bg" x={x} y={22} width={110} height={26} rx={6} />
      <rect className={active ? "sch-trigger on" : "sch-trigger"} x={x} y={22} width={110 * value} height={26} rx={6} />
      <text className="sch-label" x={x + 55} y={40}>
        {label} {Math.round(value * 100)}%
      </text>
    </g>
  );
}

function StickView({
  cx,
  cy,
  stick,
  trace,
  pressed,
  label,
}: {
  cx: number;
  cy: number;
  stick: Stick;
  trace: Stick[];
  pressed: boolean;
  label: string;
}) {
  const p = stickPoint(cx, cy, stick);
  const points = trace.map((s) => stickPoint(cx, cy, s)).map((q) => `${q.x.toFixed(1)},${q.y.toFixed(1)}`).join(" ");
  return (
    <g>
      <circle className={pressed ? "sch-stick-well on" : "sch-stick-well"} cx={cx} cy={cy} r={STICK_RANGE + 12} />
      <circle className="sch-stick-range" cx={cx} cy={cy} r={STICK_RANGE} />
      <polyline className="sch-trace" points={points} />
      <circle className="sch-stick" cx={p.x} cy={p.y} r={11} />
      <text className="sch-small" x={cx} y={cy + STICK_RANGE + 30}>{label}</text>
    </g>
  );
}
