import { useState } from "react";
import { useI18n } from "../i18n";
import type { Notice, PlatformInfo } from "../types";

export function Notices({ notices, platform }: { notices: Notice[]; platform: PlatformInfo | null }) {
  const i18n = useI18n();
  // Un seul bandeau par type de problème, détails regroupés.
  const grouped = new Map<Notice["code"], string[]>();
  for (const notice of notices) {
    const details = grouped.get(notice.code) ?? [];
    if (notice.detail) details.push(notice.detail);
    grouped.set(notice.code, details);
  }
  if (grouped.size === 0) return null;
  return (
    <div className="notices">
      {[...grouped.entries()].map(([code, details]) => (
        <div key={code} className="notice" role="alert">
          <div>{i18n.notice(code)}</div>
          {code === "hidrawPermission" && platform?.udevInstall && <CopyBlock text={platform.udevInstall} />}
          {details.length > 0 && <div className="notice-detail">{details.join(" — ")}</div>}
        </div>
      ))}
    </div>
  );
}

function CopyBlock({ text }: { text: string }) {
  const i18n = useI18n();
  const [copied, setCopied] = useState(false);
  return (
    <div className="copy-block">
      <pre>{text}</pre>
      <button
        onClick={() => {
          navigator.clipboard
            .writeText(text)
            .then(() => {
              setCopied(true);
              setTimeout(() => setCopied(false), 1500);
            })
            .catch(() => setCopied(false));
        }}
      >
        {copied ? i18n.t("copied") : i18n.t("copy")}
      </button>
    </div>
  );
}
