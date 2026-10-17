import { useI18n } from "../i18n";
import type { History } from "../history";
import type { ControllerView } from "../types";
import { Schematic } from "./Schematic";
import { batteryLabel } from "./ControllerList";

function hex(value: number | null) {
  return value === null ? "—" : value.toString(16).padStart(4, "0");
}

export function Overview({ view, history }: { view: ControllerView; history: History }) {
  const i18n = useI18n();
  const { info, state } = view;
  const battery = batteryLabel(state.battery, i18n.batteryStatus);
  const fmt = (n: number) => (n >= 0 ? "+" : "") + n.toFixed(3);

  return (
    <div className="overview">
      <Schematic view={view} leftTrace={history.leftTrace} rightTrace={history.rightTrace} />
      <dl className="facts">
        <dt>{i18n.t("leftStick")}</dt>
        <dd className="mono">
          {fmt(state.leftStick.x)} {fmt(state.leftStick.y)}
        </dd>
        <dt>{i18n.t("rightStick")}</dt>
        <dd className="mono">
          {fmt(state.rightStick.x)} {fmt(state.rightStick.y)}
        </dd>
        <dt>{i18n.t("leftTrigger")}</dt>
        <dd className="mono">{state.leftTrigger.toFixed(3)}</dd>
        <dt>{i18n.t("rightTrigger")}</dt>
        <dd className="mono">{state.rightTrigger.toFixed(3)}</dd>
        <dt>{i18n.t("battery")}</dt>
        <dd>
          {battery ?? "—"}
          {state.cableConnected ? ` · ${i18n.t("cable")}` : ""}
        </dd>
        <dt>USB ID</dt>
        <dd className="mono">
          {hex(info.vendorId)}:{hex(info.productId)}
        </dd>
        <dt>Backend</dt>
        <dd>
          {info.backend} · {i18n.connection(info.connection)}
          {info.reportMode === "reduced" ? ` · ${i18n.t("reducedMode")}` : ""}
        </dd>
        {info.imuFactoryCalibration !== null && (
          <>
            <dt>IMU</dt>
            <dd>{info.imuFactoryCalibration ? i18n.t("factoryCalibration") : i18n.t("nominalCalibration")}</dd>
          </>
        )}
      </dl>
    </div>
  );
}
