import { useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { api, errorMessage } from "../api";
import { useI18n, type MessageKey } from "../i18n";
import type { Confirmation, ControllerView, OutputItem, Support } from "../types";

type Status = "pass" | "fail" | "untested" | "unavailable";

const STATUS_KEY: Record<Status, MessageKey> = {
  pass: "statusPass",
  fail: "statusFail",
  untested: "statusUntested",
  unavailable: "statusUnavailable",
};

/** Seuils alignés sur le rapport généré par le cœur (report/document.rs). */
const EDGE_RADIUS = 0.95;
const GYRO_MOVED_DPS = 45;

function Pill({ status }: { status: Status }) {
  const i18n = useI18n();
  return <span className={`status ${status}`}>{i18n.t(STATUS_KEY[status])}</span>;
}

function fileName(view: ControllerView, ext: string): string {
  const name = view.info.name.replace(/[^\p{L}\p{N}]+/gu, "-").replace(/^-|-$/g, "");
  const d = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  const stamp = `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}_${pad(d.getHours())}${pad(d.getMinutes())}`;
  return `PatchPad-${name}-${stamp}.${ext}`;
}

export function ReportTab({ view, onNewTest }: { view: ControllerView; onNewTest: () => void }) {
  const i18n = useI18n();
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState<string | null>(null);
  const { info, report } = view;
  const id = info.id;

  const run = (p: Promise<unknown>) => p.then(() => setError(null)).catch((e) => setError(errorMessage(e)));

  // --- Vérifications en direct (le verdict détaillé est dans le rapport exporté).
  const checks: { label: string; status: Status }[] = [];
  for (const [title, stick] of [
    [i18n.t("leftStick"), view.leftStick],
    [i18n.t("rightStick"), view.rightStick],
  ] as const) {
    checks.push({ label: `${title} · ${i18n.t("stickEdge")}`, status: stick.maxRadius >= EDGE_RADIUS ? "pass" : "untested" });
    checks.push({
      label: `${title} · ${i18n.t("stickRest")}`,
      status: stick.drift === "drift" ? "fail" : stick.drift === "ok" ? "pass" : "untested",
    });
  }
  for (const [title, trigger] of [
    [i18n.t("leftTrigger"), view.leftTrigger],
    [i18n.t("rightTrigger"), view.rightTrigger],
  ] as const) {
    checks.push({
      label: `${title} · ${i18n.t("triggerFull")}`,
      status: trigger?.reachesFull && trigger.reachesZero ? "pass" : "untested",
    });
  }
  if (report.touchpad) {
    report.touchpad.fingerSeen.forEach((seen, i) =>
      checks.push({ label: `${i18n.t("finger")} ${i + 1} · ${i18n.t("fingerSeen")}`, status: seen ? "pass" : "untested" }),
    );
    checks.push({ label: i18n.t("twoFingers"), status: report.touchpad.bothAtOnce ? "pass" : "untested" });
  }
  if (report.imu) {
    const moved = report.imu.maxGyro.some((g) => g >= GYRO_MOVED_DPS);
    checks.push({ label: `IMU · ${i18n.t("imuMoved")}`, status: moved ? "pass" : "untested" });
    const g = report.imu.accelRestNorm;
    checks.push({
      label: `IMU · ${i18n.t("imuGravity")}`,
      status: g === null ? "untested" : g >= 0.9 && g <= 1.1 ? "pass" : "fail",
    });
  }

  const outputStatus = (support: Support, value: Confirmation): Status =>
    support !== "supported" ? "unavailable" : value === "ok" ? "pass" : value === "failed" ? "fail" : "untested";

  const outputs: { item: OutputItem; label: MessageKey; support: Support; test: () => Promise<unknown> }[] = [
    { item: "rumbleLeft", label: "left", support: info.capabilities.rumble, test: () => api.rumble(id, 0.8, 0, 1000) },
    { item: "rumbleRight", label: "right", support: info.capabilities.rumble, test: () => api.rumble(id, 0, 0.8, 1000) },
  ];
  if (info.capabilities.lightbar !== "noHardware") {
    outputs.push({
      item: "lightbar",
      label: "lightbar",
      support: info.capabilities.lightbar,
      // Rouge, vert, bleu puis retour au bleu Patchbay's.
      test: async () => {
        for (const rgb of [
          [255, 0, 0],
          [0, 255, 0],
          [0, 0, 255],
          [31, 69, 96],
        ] as [number, number, number][]) {
          await api.setLightbar(id, rgb);
          await new Promise((r) => setTimeout(r, 500));
        }
      },
    });
  }

  const done =
    report.buttonsTested +
    checks.filter((c) => c.status === "pass").length +
    outputs.filter((o) => report.outputs[o.item] !== "untested" && o.support === "supported").length;
  const total = report.buttonsTotal + checks.length + outputs.filter((o) => o.support === "supported").length;

  const exportAs = async (format: "pdf" | "html") => {
    setSaved(null);
    const path = await save({
      defaultPath: fileName(view, format),
      filters: [{ name: format.toUpperCase(), extensions: [format] }],
    });
    if (!path) return;
    const generatedAt = new Date().toLocaleString(i18n.lang === "fr" ? "fr-FR" : "en-US");
    const written = await api.exportReport(id, format, path, i18n.lang, generatedAt);
    setSaved(written);
  };

  const minutes = Math.floor(view.reportElapsedS / 60);
  const seconds = Math.floor(view.reportElapsedS % 60);

  return (
    <div className="report">
      <p className="muted report-intro">{i18n.t("reportIntro")}</p>
      <div className="toolbar">
        <button
          onClick={() => {
            onNewTest();
            run(api.startReport(id));
          }}
        >
          {i18n.t("newTest")}
        </button>
        <span className="muted">
          {i18n.t("elapsed")} <span className="mono">{`${minutes}:${String(seconds).padStart(2, "0")}`}</span>
        </span>
        <span className="spacer" />
        <button className="primary" onClick={() => run(exportAs("pdf"))}>
          {i18n.t("exportPdf")}
        </button>
        <button onClick={() => run(exportAs("html"))}>{i18n.t("exportHtml")}</button>
      </div>
      {saved && (
        <div className="saved">
          {i18n.t("exported")} : <span className="mono">{saved}</span>
          <button className="link" onClick={() => run(revealItemInDir(saved))}>
            {i18n.t("reveal")}
          </button>
        </div>
      )}
      {error && <p className="error">{error}</p>}

      <div className="panel">
        <h3>
          {i18n.t("progress")}{" "}
          <span className="muted small">
            {done}/{total}
          </span>
        </h3>
        <div className="bar" role="progressbar" aria-valuenow={done} aria-valuemin={0} aria-valuemax={total}>
          <div className="bar-fill" style={{ width: `${total ? (done / total) * 100 : 0}%` }} />
        </div>
      </div>

      <div className="panel">
        <h3>
          {i18n.t("buttons")}{" "}
          <span className="muted small">
            {report.buttonsTested}/{report.buttonsTotal}
          </span>
        </h3>
        <div className="chips">
          {report.buttons.map((check) => {
            const pressed = view.state.buttons.includes(check.button);
            const cls = !check.available ? "chip unavailable" : check.presses > 0 ? "chip done" : "chip";
            return (
              <div
                key={check.button}
                className={pressed ? `${cls} pressed` : cls}
                title={check.available ? undefined : i18n.t("unavailableOs")}
              >
                <span className="chip-label">{i18n.button(info.family, check.button)}</span>
                <span className="chip-count">{check.available ? `×${check.presses}` : "—"}</span>
              </div>
            );
          })}
        </div>
      </div>

      <div className="report-grid">
        <div className="panel">
          <h3>{i18n.t("checks")}</h3>
          <ul className="checklist">
            {checks.map((c) => (
              <li key={c.label}>
                <span>{c.label}</span>
                <Pill status={c.status} />
              </li>
            ))}
          </ul>
        </div>
        <div className="panel">
          <h3>{i18n.t("tabOutputs")}</h3>
          <ul className="checklist">
            {outputs.map((o) => {
              const status = outputStatus(o.support, report.outputs[o.item]);
              const disabled = o.support !== "supported";
              return (
                <li key={o.item} title={disabled ? i18n.t("rumbleUnsupportedOs") : undefined}>
                  <span>{i18n.t(o.label)}</span>
                  <span className="output-actions">
                    <button disabled={disabled} onClick={() => run(o.test())}>
                      {i18n.t("testOutput")}
                    </button>
                    <button disabled={disabled} onClick={() => run(api.confirmOutput(id, o.item, "ok"))}>
                      ✓ {i18n.t("works")}
                    </button>
                    <button disabled={disabled} onClick={() => run(api.confirmOutput(id, o.item, "failed"))}>
                      ✗ {i18n.t("broken")}
                    </button>
                    <Pill status={status} />
                  </span>
                </li>
              );
            })}
          </ul>
        </div>
      </div>
    </div>
  );
}
