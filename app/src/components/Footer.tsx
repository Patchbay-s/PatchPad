import { useI18n } from "../i18n";
import type { ControllerView, LogEntry } from "../types";

function formatTime(ms: number): string {
  const s = ms / 1000;
  const m = Math.floor(s / 60);
  return `${String(m).padStart(2, "0")}:${(s % 60).toFixed(3).padStart(6, "0")}`;
}

export function Footer({
  view,
  controllers,
  log,
  onClearLog,
}: {
  view: ControllerView | null;
  controllers: ControllerView[];
  log: LogEntry[];
  onClearLog: () => void;
}) {
  const i18n = useI18n();
  // gilrs ne publie que les changements : on l'indique pour ne pas confondre avec la fréquence matérielle.
  const rateLabel = view?.info.backend === "gilrs" ? i18n.t("changesRate") : i18n.t("reportRate");
  const familyOf = new Map(controllers.map((c) => [c.info.id, c.info.family]));

  return (
    <footer className="footer">
      <div className="rate">
        <span className="muted">{rateLabel}</span>
        <span className="rate-value">{view ? `${view.rate.hz.toFixed(0)} Hz` : "—"}</span>
        {view && view.rate.hz > 0 && (
          <span className="muted">
            {view.rate.meanIntervalMs.toFixed(2)} ms · {i18n.t("jitter")} {view.rate.jitterMs.toFixed(2)} ms · max{" "}
            {view.rate.maxIntervalMs.toFixed(1)} ms
          </span>
        )}
      </div>
      <div className="log">
        <div className="log-header">
          <span>{i18n.t("log")}</span>
          <button className="link" onClick={onClearLog}>
            {i18n.t("clearLog")}
          </button>
        </div>
        <ol className="log-list" aria-live="polite">
          {log.length === 0 && <li className="muted">{i18n.t("noEvents")}</li>}
          {[...log].reverse().slice(0, 60).map((entry) => {
            const family = familyOf.get(entry.controllerId) ?? "generic";
            const what = entry.button
              ? `${i18n.button(family, entry.button)} ${i18n.t(entry.kind)}`
              : `${entry.controllerName} ${i18n.t(entry.kind)}`;
            return (
              <li key={entry.seq} className={`log-${entry.kind}`}>
                <span className="log-time">{formatTime(entry.tMs)}</span>
                <span>{what}</span>
              </li>
            );
          })}
        </ol>
      </div>
    </footer>
  );
}
