// Traductions FR / EN. La langue suit le système, modifiable dans l'en-tête.

import { createContext, useContext } from "react";
import type { BatteryStatus, Button, Connection, Family, NoticeCode } from "./types";

export type Lang = "fr" | "en";

const fr = {
  appSubtitle: "Testeur de manettes",
  controllers: "Manettes",
  noController: "Aucune manette détectée",
  noControllerHint: "Branchez une manette en USB ou appairez-la en Bluetooth.",
  tabOverview: "Vue",
  tabSticks: "Sticks",
  tabTouchpad: "Pavé tactile",
  tabMotion: "Mouvement",
  tabOutputs: "Sorties",
  reportRate: "Fréquence",
  changesRate: "Changements",
  jitter: "gigue",
  log: "Journal",
  clearLog: "Effacer",
  noEvents: "Aucun événement",
  pressed: "appui",
  released: "relâché",
  connected: "connectée",
  disconnected: "déconnectée",
  battery: "Batterie",
  cable: "câble",
  reducedMode: "mode réduit",
  dongleIdle: "aucune manette appairée à l'adaptateur",
  leftStick: "Stick gauche",
  rightStick: "Stick droit",
  leftTrigger: "Gâchette gauche",
  rightTrigger: "Gâchette droite",
  rest: "Repos",
  restOffset: "Décalage au repos",
  restUnknown: "Relâchez le stick quelques instants",
  atRest: "au repos",
  moving: "en mouvement",
  drift: "Drift",
  driftOk: "Pas de drift",
  driftDetected: "Drift détecté",
  driftUnknown: "En attente d'un repos",
  driftThreshold: "Seuil de drift",
  suggestedDeadzone: "Zone morte conseillée",
  circularity: "Circularité",
  circularityHint: "Faites plusieurs tours complets du stick en butée.",
  coverage: "couverture",
  avgError: "écart moyen",
  maxRadius: "Rayon max",
  resetAnalysis: "Réinitialiser les mesures",
  clearTrace: "Effacer la trace",
  triggerRange: "Course",
  reachesZero: "Revient à 0",
  reachesFull: "Atteint 100 %",
  yes: "oui",
  no: "non",
  touchpadClick: "Clic du pavé",
  finger: "Doigt",
  inactive: "inactif",
  noTouchpad: "Cette manette n'a pas de pavé tactile.",
  touchpadReduced: "Pavé tactile indisponible en mode Bluetooth réduit.",
  noImu: "Cette manette n'a pas de capteurs de mouvement.",
  imuReduced: "Capteurs indisponibles en mode Bluetooth réduit.",
  gyro: "Gyroscope (°/s)",
  accel: "Accéléromètre (g)",
  orientation: "Orientation",
  pitch: "Tangage",
  roll: "Roulis",
  yaw: "Lacet",
  recalibrate: "Recalibrer le zéro",
  recalibrating: "Mesure du zéro… ne bougez pas la manette",
  recalibrateHint: "Posez la manette immobile sur une table avant de recalibrer.",
  factoryCalibration: "Calibration usine",
  nominalCalibration: "Calibration nominale (usine indisponible)",
  vibration: "Vibration",
  left: "Gauche (fort)",
  right: "Droite (faible)",
  both: "Les deux",
  intensity: "Intensité",
  duration: "Durée",
  rumbleUnsupportedOs: "Non supporté sur cet OS",
  rumbleNoHardware: "Non supporté par cette manette",
  triggerMotorsNote: "Les moteurs des gâchettes (Xbox) ne sont pas pris en charge.",
  lightbar: "Barre lumineuse",
  apply: "Appliquer",
  lightbarNoHardware: "Pas de barre lumineuse sur cette manette.",
  copy: "Copier",
  copied: "Copié",
  language: "Langue",
  tabReport: "Rapport",
  buttons: "Boutons",
  reportIntro:
    "Appuyez sur chaque bouton, faites plusieurs tours de chaque stick en butée, enfoncez les gâchettes à fond, posez deux doigts sur le pavé, secouez puis posez la manette, et testez les sorties. Tout est enregistré automatiquement.",
  newTest: "Nouveau test",
  elapsed: "Durée",
  progress: "Progression",
  checks: "Vérifications",
  stickEdge: "atteint le bord",
  stickRest: "repos mesuré",
  triggerFull: "course complète",
  fingerSeen: "doigt détecté",
  twoFingers: "deux doigts simultanés",
  imuMoved: "mouvement détecté",
  imuGravity: "gravité au repos",
  testOutput: "Tester",
  works: "Fonctionne",
  broken: "Ne fonctionne pas",
  exportPdf: "Exporter en PDF",
  exportHtml: "Exporter en HTML",
  exported: "Rapport enregistré",
  reveal: "Afficher dans le dossier",
  statusPass: "Réussi",
  statusFail: "Échec",
  statusUntested: "Non testé",
  statusUnavailable: "Indisponible",
  unavailableOs: "illisible sur cet OS",
} as const;

export type MessageKey = keyof typeof fr;

const en: Record<MessageKey, string> = {
  appSubtitle: "Controller tester",
  controllers: "Controllers",
  noController: "No controller detected",
  noControllerHint: "Plug a controller in over USB or pair it over Bluetooth.",
  tabOverview: "Overview",
  tabSticks: "Sticks",
  tabTouchpad: "Touchpad",
  tabMotion: "Motion",
  tabOutputs: "Outputs",
  reportRate: "Report rate",
  changesRate: "Changes",
  jitter: "jitter",
  log: "Event log",
  clearLog: "Clear",
  noEvents: "No events",
  pressed: "pressed",
  released: "released",
  connected: "connected",
  disconnected: "disconnected",
  battery: "Battery",
  cable: "cable",
  reducedMode: "reduced mode",
  dongleIdle: "no controller paired to the adaptor",
  leftStick: "Left stick",
  rightStick: "Right stick",
  leftTrigger: "Left trigger",
  rightTrigger: "Right trigger",
  rest: "Rest",
  restOffset: "Rest offset",
  restUnknown: "Let go of the stick for a moment",
  atRest: "at rest",
  moving: "moving",
  drift: "Drift",
  driftOk: "No drift",
  driftDetected: "Drift detected",
  driftUnknown: "Waiting for the stick to rest",
  driftThreshold: "Drift threshold",
  suggestedDeadzone: "Suggested dead zone",
  circularity: "Circularity",
  circularityHint: "Rotate the stick against its edge a few full turns.",
  coverage: "coverage",
  avgError: "average error",
  maxRadius: "Max radius",
  resetAnalysis: "Reset measurements",
  clearTrace: "Clear trace",
  triggerRange: "Travel",
  reachesZero: "Returns to 0",
  reachesFull: "Reaches 100%",
  yes: "yes",
  no: "no",
  touchpadClick: "Touchpad click",
  finger: "Finger",
  inactive: "inactive",
  noTouchpad: "This controller has no touchpad.",
  touchpadReduced: "Touchpad unavailable in Bluetooth reduced mode.",
  noImu: "This controller has no motion sensors.",
  imuReduced: "Sensors unavailable in Bluetooth reduced mode.",
  gyro: "Gyroscope (°/s)",
  accel: "Accelerometer (g)",
  orientation: "Orientation",
  pitch: "Pitch",
  roll: "Roll",
  yaw: "Yaw",
  recalibrate: "Recalibrate zero",
  recalibrating: "Measuring zero… keep the controller still",
  recalibrateHint: "Lay the controller still on a table before recalibrating.",
  factoryCalibration: "Factory calibration",
  nominalCalibration: "Nominal calibration (factory data unavailable)",
  vibration: "Vibration",
  left: "Left (strong)",
  right: "Right (weak)",
  both: "Both",
  intensity: "Intensity",
  duration: "Duration",
  rumbleUnsupportedOs: "Not supported on this OS",
  rumbleNoHardware: "Not supported by this controller",
  triggerMotorsNote: "Trigger motors (Xbox) are not supported.",
  lightbar: "Light bar",
  apply: "Apply",
  lightbarNoHardware: "This controller has no light bar.",
  copy: "Copy",
  copied: "Copied",
  language: "Language",
  tabReport: "Report",
  buttons: "Buttons",
  reportIntro:
    "Press every button, rotate each stick against its edge a few times, pull the triggers all the way, put two fingers on the touchpad, shake then lay the controller down, and test the outputs. Everything is recorded automatically.",
  newTest: "New test",
  elapsed: "Duration",
  progress: "Progress",
  checks: "Checks",
  stickEdge: "reaches the edge",
  stickRest: "rest measured",
  triggerFull: "full travel",
  fingerSeen: "finger detected",
  twoFingers: "two fingers at once",
  imuMoved: "movement detected",
  imuGravity: "gravity at rest",
  testOutput: "Test",
  works: "Works",
  broken: "Doesn't work",
  exportPdf: "Export as PDF",
  exportHtml: "Export as HTML",
  exported: "Report saved",
  reveal: "Show in folder",
  statusPass: "Passed",
  statusFail: "Failed",
  statusUntested: "Not tested",
  statusUnavailable: "Unavailable",
  unavailableOs: "not readable on this OS",
};

const notices: Record<Lang, Record<NoticeCode, string>> = {
  fr: {
    hidrawPermission:
      "Accès hidraw refusé : sans règle udev, la DualShock 4 n'est lue qu'en mode générique (sans pavé tactile, capteurs ni barre lumineuse). Installez la règle puis rebranchez la manette :",
    macHidDenied:
      "macOS a refusé l'accès à la manette. Vérifiez Réglages Système › Confidentialité et sécurité › Surveillance de l'entrée, puis relancez PatchPad.",
    hidOpenFailed: "Impossible d'ouvrir la manette HID.",
    hidUnavailable: "hidapi n'a pas pu démarrer : la DualShock 4 ne sera lue qu'en mode générique.",
    gilrsUnavailable: "Le backend générique (gilrs) n'a pas pu démarrer : les manettes Xbox ne seront pas détectées.",
    xboxUsbMacUnsupported:
      "Manette Xbox branchée en USB : macOS ne la prend pas en charge en filaire (protocole GIP, sans pilote). Débranchez-la et appairez-la en Bluetooth.",
    ds4ReducedMode:
      "La DualShock 4 reste en mode Bluetooth réduit (ni pavé tactile, ni capteurs, ni batterie). Reconnectez-la ou essayez en USB.",
  },
  en: {
    hidrawPermission:
      "hidraw access denied: without the udev rule, the DualShock 4 is only read in generic mode (no touchpad, sensors or light bar). Install the rule, then replug the controller:",
    macHidDenied:
      "macOS denied access to the controller. Check System Settings › Privacy & Security › Input Monitoring, then restart PatchPad.",
    hidOpenFailed: "Could not open the HID controller.",
    hidUnavailable: "hidapi failed to start: the DualShock 4 will only be read in generic mode.",
    gilrsUnavailable: "The generic backend (gilrs) failed to start: Xbox controllers will not be detected.",
    xboxUsbMacUnsupported:
      "Xbox controller plugged in over USB: macOS does not support it wired (GIP protocol, no driver). Unplug it and pair it over Bluetooth.",
    ds4ReducedMode:
      "The DualShock 4 is stuck in Bluetooth reduced mode (no touchpad, sensors or battery). Reconnect it or try USB.",
  },
};

const connections: Record<Lang, Record<Connection, string>> = {
  fr: { usb: "USB", bluetooth: "Bluetooth", wireless: "Sans fil", dongle: "Adaptateur", unknown: "Inconnue" },
  en: { usb: "USB", bluetooth: "Bluetooth", wireless: "Wireless", dongle: "Adaptor", unknown: "Unknown" },
};

const batteryStatus: Record<Lang, Record<BatteryStatus, string>> = {
  fr: { discharging: "", charging: "en charge", full: "pleine", wired: "filaire", unknown: "erreur de charge" },
  en: { discharging: "", charging: "charging", full: "full", wired: "wired", unknown: "charge error" },
};

const BUTTONS: Record<Family, Record<Button, string>> = {
  dualShock4: {
    south: "✕", east: "○", west: "□", north: "△",
    l1: "L1", r1: "R1", l2: "L2", r2: "R2",
    select: "Share", start: "Options", l3: "L3", r3: "R3", home: "PS", touchpad: "Touchpad",
    dpadUp: "↑", dpadDown: "↓", dpadLeft: "←", dpadRight: "→",
  },
  xbox: {
    south: "A", east: "B", west: "X", north: "Y",
    l1: "LB", r1: "RB", l2: "LT", r2: "RT",
    select: "View", start: "Menu", l3: "LS", r3: "RS", home: "Xbox", touchpad: "—",
    dpadUp: "↑", dpadDown: "↓", dpadLeft: "←", dpadRight: "→",
  },
  generic: {
    south: "South", east: "East", west: "West", north: "North",
    l1: "L1", r1: "R1", l2: "L2", r2: "R2",
    select: "Select", start: "Start", l3: "L3", r3: "R3", home: "Home", touchpad: "Touchpad",
    dpadUp: "↑", dpadDown: "↓", dpadLeft: "←", dpadRight: "→",
  },
};

export interface I18n {
  lang: Lang;
  t: (key: MessageKey) => string;
  notice: (code: NoticeCode) => string;
  connection: (c: Connection) => string;
  batteryStatus: (s: BatteryStatus) => string;
  button: (family: Family, b: Button) => string;
}

export function makeI18n(lang: Lang): I18n {
  const table: Record<MessageKey, string> = lang === "fr" ? fr : en;
  return {
    lang,
    t: (key) => table[key],
    notice: (code) => notices[lang][code],
    connection: (c) => connections[lang][c],
    batteryStatus: (s) => batteryStatus[lang][s],
    button: (family, b) => BUTTONS[family][b],
  };
}

export function systemLang(): Lang {
  return navigator.language.toLowerCase().startsWith("fr") ? "fr" : "en";
}

export const I18nContext = createContext<I18n>(makeI18n("en"));

export function useI18n(): I18n {
  return useContext(I18nContext);
}
