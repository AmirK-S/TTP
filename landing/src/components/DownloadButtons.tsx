import { useEffect, useState } from "react";

interface DownloadButtonsProps {
  downloadLinks?: { mac: string; windows: string };
  translations?: { downloadFor: string };
  locale?: string;
}

type DetectedOS = "mac" | "windows" | "other";

function detectOS(): DetectedOS {
  if (typeof navigator === "undefined") return "other";

  const ua = navigator.userAgent.toLowerCase();
  const platform = (navigator.platform || "").toLowerCase();

  if (platform.includes("mac") || ua.includes("macintosh") || ua.includes("mac os")) {
    return "mac";
  }
  if (platform.includes("win") || ua.includes("windows")) {
    return "windows";
  }
  return "other";
}

const FALLBACK_URL = "https://github.com/AmirK-S/TTP/releases";

export function DownloadButtons({ downloadLinks, translations, locale }: DownloadButtonsProps) {
  const downloadForText = translations?.downloadFor ?? "Download for";

  const links = downloadLinks ?? { mac: FALLBACK_URL, windows: FALLBACK_URL };
  const [detectedOS, setDetectedOS] = useState<DetectedOS>("other");

  useEffect(() => {
    setDetectedOS(detectOS());
  }, []);

  const isMacHighlighted = detectedOS === "mac" || detectedOS === "other";
  const isWinHighlighted = detectedOS === "windows";

  return (
    <div className="flex flex-col items-center gap-4 sm:flex-row sm:gap-6">
      {/* macOS Button */}
      <a
        href={links.mac}
        download
        className={`group flex items-center gap-3 rounded-xl border px-6 py-4 transition-all ${
          isMacHighlighted
            ? "border-white/20 bg-white/10 text-white shadow-lg shadow-white/5 hover:bg-white/15"
            : "border-white/10 bg-transparent text-slate-400 hover:border-white/20 hover:text-white"
        }`}
      >
        {/* Apple icon */}
        <svg
          className="h-6 w-6 flex-shrink-0"
          viewBox="0 0 24 24"
          fill="currentColor"
        >
          <path d="M18.71 19.5c-.83 1.24-1.71 2.45-3.05 2.47-1.34.03-1.77-.79-3.29-.79-1.53 0-2 .77-3.27.82-1.31.05-2.3-1.32-3.14-2.53C4.25 17 2.94 12.45 4.7 9.39c.87-1.52 2.43-2.48 4.12-2.51 1.28-.02 2.5.87 3.29.87.78 0 2.26-1.07 3.8-.91.65.03 2.47.26 3.64 1.98-.09.06-2.17 1.28-2.15 3.81.03 3.02 2.65 4.03 2.68 4.04-.03.07-.42 1.44-1.38 2.83M13 3.5c.73-.83 1.94-1.46 2.94-1.5.13 1.17-.34 2.35-1.04 3.19-.69.85-1.83 1.51-2.95 1.42-.15-1.15.41-2.35 1.05-3.11z" />
        </svg>
        <div className="text-left">
          <div className="text-xs uppercase tracking-wide opacity-70">
            {downloadForText}
          </div>
          <div className="text-base font-semibold">macOS</div>
        </div>
      </a>

      {/* Windows Button */}
      <a
        href={links.windows}
        download
        className={`group flex items-center gap-3 rounded-xl border px-6 py-4 transition-all ${
          isWinHighlighted
            ? "border-white/20 bg-white/10 text-white shadow-lg shadow-white/5 hover:bg-white/15"
            : "border-white/10 bg-transparent text-slate-400 hover:border-white/20 hover:text-white"
        }`}
      >
        {/* Windows icon */}
        <svg
          className="h-6 w-6 flex-shrink-0"
          viewBox="0 0 24 24"
          fill="currentColor"
        >
          <path d="M3 12V6.5l8-1.1V12H3zm0 .5h8v6.6l-8-1.1V12.5zm9 0h9V3l-9 1.2V12.5zm0 .5v6.3L21 21v-8.5h-9z" />
        </svg>
        <div className="text-left">
          <div className="text-xs uppercase tracking-wide opacity-70">
            {downloadForText}
          </div>
          <div className="text-base font-semibold">Windows</div>
        </div>
      </a>
    </div>
  );
}
