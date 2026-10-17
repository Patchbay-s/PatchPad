import { useEffect, useMemo, useRef, useState } from "react";
import { api, onSnapshot } from "./api";
import { HistoryStore } from "./history";
import { I18nContext, makeI18n, systemLang, type Lang, type MessageKey } from "./i18n";
import type { LogEntry, PlatformInfo, Snapshot } from "./types";
import { ControllerList } from "./components/ControllerList";
import { Notices } from "./components/Notices";
import { Overview } from "./components/Overview";
import { SticksTab } from "./components/SticksTab";
import { TouchpadTab } from "./components/TouchpadTab";
import { MotionTab } from "./components/MotionTab";
import { OutputsTab } from "./components/OutputsTab";
import { Footer } from "./components/Footer";
import { ReportTab } from "./components/ReportTab";
import mark from "./assets/patchpad-mark.png";

type Tab = "overview" | "sticks" | "touchpad" | "motion" | "outputs" | "report";

const TABS: { id: Tab; label: MessageKey }[] = [
  { id: "overview", label: "tabOverview" },
  { id: "sticks", label: "tabSticks" },
  { id: "touchpad", label: "tabTouchpad" },
  { id: "motion", label: "tabMotion" },
  { id: "outputs", label: "tabOutputs" },
  { id: "report", label: "tabReport" },
];

const LOG_MAX = 200;
const LANG_KEY = "patchpad.lang";

function storedLang(): Lang {
  try {
    const value = localStorage.getItem(LANG_KEY);
    if (value === "fr" || value === "en") return value;
  } catch {
    // Stockage indisponible : on suit le système.
  }
  return systemLang();
}

export default function App() {
  const [lang, setLang] = useState<Lang>(storedLang);
  const i18n = useMemo(() => makeI18n(lang), [lang]);
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [log, setLog] = useState<LogEntry[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [tab, setTab] = useState<Tab>("overview");
  const [platform, setPlatform] = useState<PlatformInfo | null>(null);
  const history = useRef(new HistoryStore());

  useEffect(() => {
    document.documentElement.lang = lang;
    try {
      localStorage.setItem(LANG_KEY, lang);
    } catch {
      // Préférence non conservée, sans conséquence.
    }
  }, [lang]);

  useEffect(() => {
    api.platform().then(setPlatform).catch(() => setPlatform(null));
    let disposed = false;
    const unlisten = onSnapshot((next) => {
      if (disposed) return;
      history.current.record(next.controllers);
      setSnapshot(next);
      if (next.newLog.length > 0) {
        setLog((previous) => [...previous, ...next.newLog].slice(-LOG_MAX));
      }
    });
    return () => {
      disposed = true;
      unlisten.then((fn) => fn());
    };
  }, []);

  const controllers = snapshot?.controllers ?? [];
  // Sélection : celle de l'utilisateur si encore présente, sinon la première manette.
  const view = controllers.find((c) => c.info.id === selected) ?? controllers[0] ?? null;

  return (
    <I18nContext.Provider value={i18n}>
      <div className="app">
        <aside className="sidebar">
          <header className="brand">
            <img className="brand-mark" src={mark} alt="" />
            <div>
              <div className="brand-name">PatchPad</div>
              <div className="brand-sub">{i18n.t("appSubtitle")}</div>
            </div>
          </header>
          <ControllerList controllers={controllers} selected={view?.info.id ?? null} onSelect={setSelected} />
          <div className="sidebar-footer">
            <label>
              {i18n.t("language")}{" "}
              <select value={lang} onChange={(e) => setLang(e.target.value as Lang)}>
                <option value="fr">Français</option>
                <option value="en">English</option>
              </select>
            </label>
          </div>
        </aside>

        <main className="main">
          <Notices notices={snapshot?.notices ?? []} platform={platform} />
          {view ? (
            <>
              <nav className="tabs" role="tablist">
                {TABS.map(({ id, label }) => (
                  <button
                    key={id}
                    role="tab"
                    aria-selected={tab === id}
                    className={tab === id ? "tab active" : "tab"}
                    onClick={() => setTab(id)}
                  >
                    {i18n.t(label)}
                  </button>
                ))}
              </nav>
              <section className="tab-body">
                {tab === "overview" && <Overview view={view} history={history.current.get(view.info.id)} />}
                {tab === "sticks" && (
                  <SticksTab
                    view={view}
                    history={history.current.get(view.info.id)}
                    threshold={snapshot?.driftThreshold ?? 0.05}
                    onClear={() => history.current.clearSticks(view.info.id)}
                  />
                )}
                {tab === "touchpad" && (
                  <TouchpadTab
                    view={view}
                    history={history.current.get(view.info.id)}
                    onClear={() => history.current.clearTouch(view.info.id)}
                  />
                )}
                {tab === "motion" && <MotionTab view={view} history={history.current.get(view.info.id)} />}
                {tab === "outputs" && <OutputsTab view={view} />}
                {tab === "report" && (
                  <ReportTab
                    view={view}
                    onNewTest={() => {
                      history.current.clearSticks(view.info.id);
                      history.current.clearTouch(view.info.id);
                    }}
                  />
                )}
              </section>
            </>
          ) : (
            <div className="empty">
              <div className="empty-title">{i18n.t("noController")}</div>
              <div className="muted">{i18n.t("noControllerHint")}</div>
            </div>
          )}
          <Footer view={view} controllers={controllers} log={log} onClearLog={() => setLog([])} />
        </main>
      </div>
    </I18nContext.Provider>
  );
}
