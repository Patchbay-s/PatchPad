import { useI18n } from "../i18n";
import type { Battery, ControllerView } from "../types";

export function batteryLabel(battery: Battery | null, status: (s: Battery["status"]) => string): string | null {
  if (!battery) return null;
  const level = battery.level !== null ? `${battery.level} %` : "";
  const detail = status(battery.status);
  return [level, detail].filter(Boolean).join(" · ") || null;
}

export function ControllerList({
  controllers,
  selected,
  onSelect,
}: {
  controllers: ControllerView[];
  selected: string | null;
  onSelect: (id: string) => void;
}) {
  const i18n = useI18n();
  if (controllers.length === 0) {
    return <div className="list-empty muted">{i18n.t("noController")}</div>;
  }
  return (
    <ul className="controller-list">
      {controllers.map(({ info, state }) => {
        const battery = batteryLabel(state.battery, i18n.batteryStatus);
        return (
          <li key={info.id}>
            <button
              className={info.id === selected ? "controller active" : "controller"}
              onClick={() => onSelect(info.id)}
              title={`${info.backend} · ${info.id}`}
            >
              <span className="controller-name">{info.name}</span>
              <span className="controller-meta">
                <span className="pill">{i18n.connection(info.connection)}</span>
                {battery && <span className="pill">🔋 {battery}</span>}
                {info.reportMode === "reduced" && <span className="pill warn">{i18n.t("reducedMode")}</span>}
              </span>
              {info.dongleIdle && <span className="controller-note muted">{i18n.t("dongleIdle")}</span>}
            </button>
          </li>
        );
      })}
    </ul>
  );
}
