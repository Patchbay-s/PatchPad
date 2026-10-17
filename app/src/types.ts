// Miroir des types sérialisés par patchpad-core (serde, camelCase).

export type Button =
  | "south" | "east" | "west" | "north"
  | "l1" | "r1" | "l2" | "r2"
  | "select" | "start" | "l3" | "r3" | "home" | "touchpad"
  | "dpadUp" | "dpadDown" | "dpadLeft" | "dpadRight";

export interface Stick { x: number; y: number }
export interface Finger { active: boolean; id: number; x: number; y: number }
export interface Touchpad { fingers: [Finger, Finger]; clicked: boolean; width: number; height: number }
export interface Imu { gyro: [number, number, number]; accel: [number, number, number] }
export type BatteryStatus = "discharging" | "charging" | "full" | "wired" | "unknown";
export interface Battery { level: number | null; status: BatteryStatus }

export interface ControllerState {
  buttons: Button[];
  leftStick: Stick;
  rightStick: Stick;
  leftTrigger: number;
  rightTrigger: number;
  touchpad: Touchpad | null;
  imu: Imu | null;
  battery: Battery | null;
  cableConnected: boolean | null;
}

export type Family = "dualShock4" | "xbox" | "generic";
export type Connection = "usb" | "bluetooth" | "wireless" | "dongle" | "unknown";
export type Support = "supported" | "unsupportedOs" | "noHardware";

export interface ControllerInfo {
  id: string;
  name: string;
  family: Family;
  backend: string;
  connection: Connection;
  vendorId: number | null;
  productId: number | null;
  capabilities: {
    touchpad: boolean;
    imu: boolean;
    rumble: Support;
    lightbar: Support;
    analogTriggers: boolean;
  };
  reportMode: "full" | "reduced" | null;
  imuFactoryCalibration: boolean | null;
  dongleIdle: boolean;
}

export interface RateStats { hz: number; meanIntervalMs: number; jitterMs: number; maxIntervalMs: number }

export type DriftVerdict = "unknown" | "ok" | "drift";

export interface StickReport {
  rest: Stick | null;
  restOffset: number | null;
  restRadius: number | null;
  suggestedDeadzone: number | null;
  drift: DriftVerdict;
  atRest: boolean;
  sectors: number[];
  coverage: number;
  circularityError: number | null;
  maxRadius: number;
}

export interface TriggerReport { min: number; max: number; reachesZero: boolean; reachesFull: boolean }
export interface Orientation { pitch: number; roll: number; yaw: number }

export interface ControllerView {
  info: ControllerInfo;
  state: ControllerState;
  rate: RateStats;
  leftStick: StickReport;
  rightStick: StickReport;
  leftTrigger: TriggerReport | null;
  rightTrigger: TriggerReport | null;
  orientation: Orientation | null;
  imuZeroing: boolean;
  report: ReportProgress;
  reportElapsedS: number;
}

export type Confirmation = "untested" | "ok" | "failed";
export type OutputItem = "rumbleLeft" | "rumbleRight" | "lightbar";

export interface ButtonCheck {
  button: Button;
  available: boolean;
  presses: number;
  firstPressMs: number | null;
}

export interface ReportProgress {
  buttons: ButtonCheck[];
  touchpad: {
    fingerSeen: [boolean, boolean];
    bothAtOnce: boolean;
    clicked: boolean;
    xRange: [number, number] | null;
    yRange: [number, number] | null;
  } | null;
  imu: { samples: number; maxGyro: [number, number, number]; accelRestNorm: number | null } | null;
  outputs: { rumbleLeft: Confirmation; rumbleRight: Confirmation; lightbar: Confirmation };
  buttonsTested: number;
  buttonsTotal: number;
}

export type NoticeCode =
  | "hidrawPermission"
  | "macHidDenied"
  | "hidOpenFailed"
  | "hidUnavailable"
  | "gilrsUnavailable"
  | "xboxUsbMacUnsupported"
  | "ds4ReducedMode";

export interface Notice { code: NoticeCode; detail: string }

export interface LogEntry {
  seq: number;
  tMs: number;
  controllerId: string;
  controllerName: string;
  kind: "pressed" | "released" | "connected" | "disconnected";
  button: Button | null;
}

export interface Snapshot {
  controllers: ControllerView[];
  notices: Notice[];
  newLog: LogEntry[];
  driftThreshold: number;
}

export interface PlatformInfo { os: string; udevInstall: string | null }
