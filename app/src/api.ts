// Accès typé aux commandes et événements Tauri.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { Confirmation, OutputItem, PlatformInfo, Snapshot } from "./types";

export const api = {
  platform: () => invoke<PlatformInfo>("platform"),
  rumble: (id: string, strong: number, weak: number, durationMs: number) =>
    invoke<void>("rumble", { id, strong, weak, durationMs }),
  setLightbar: (id: string, rgb: [number, number, number]) => invoke<void>("set_lightbar", { id, rgb }),
  resetAnalysis: (id: string | null) => invoke<void>("reset_analysis", { id }),
  recalibrateImu: (id: string) => invoke<void>("recalibrate_imu", { id }),
  setDriftThreshold: (threshold: number) => invoke<void>("set_drift_threshold", { threshold }),
  startReport: (id: string) => invoke<void>("start_report", { id }),
  confirmOutput: (id: string, item: OutputItem, result: Confirmation) =>
    invoke<void>("confirm_output", { id, item, result }),
  exportReport: (id: string, format: "html" | "pdf", path: string, lang: "fr" | "en", generatedAt: string) =>
    invoke<string>("export_report", { id, format, path, lang, generatedAt }),
};

export function onSnapshot(handler: (snapshot: Snapshot) => void): Promise<UnlistenFn> {
  return listen<Snapshot>("pad://snapshot", (e) => handler(e.payload));
}

/// Message lisible d'une erreur de commande (les commandes rejettent avec une chaîne).
export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error);
}
