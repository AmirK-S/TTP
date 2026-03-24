import { useState } from "react";

interface InstallGuideProps {
  version?: string;
  windowsDownloadUrl?: string;
  translations?: {
    heading: string;
    subheading: string;
    whyTitle: string;
    whyText: string;
    macTab: string;
    winTab: string;
    macAlreadyTitle: string;
    macAlreadyDesc: string;
    macOneLineTitle: string;
    macOneLineDesc: string;
    macAltTitle: string;
    macAltStep1: string;
    macAltStep2: string;
    macAltStep3: string;
    winDownloadTitle: string;
    winDownloadButton: string;
    winDownloadDesc: string;
    winSmartScreenTitle: string;
    winDesc: string;
    winStep1: string;
    winStep2: string;
    copied: string;
    clickToCopy: string;
  };
}

const MAC_ONE_LINER = `curl -sL ttp.amirks.eu/install.sh | sh`;

function CopyButton({ text, copiedLabel, copyLabel }: { text: string; copiedLabel: string; copyLabel: string }) {
  const [copied, setCopied] = useState(false);

  const handleCopy = () => {
    navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <button
      onClick={handleCopy}
      className="shrink-0 rounded-md border border-white/10 bg-white/5 px-3 py-1.5 text-xs text-slate-400 transition-all hover:bg-white/10 hover:text-white"
    >
      {copied ? copiedLabel : copyLabel}
    </button>
  );
}

const WINDOWS_FALLBACK_URL = "https://github.com/AmirK-S/TTP/releases";

export function InstallGuide({ translations, windowsDownloadUrl }: InstallGuideProps) {
  const t = {
    heading: translations?.heading ?? "Installation",
    subheading: translations?.subheading ?? "Get TTP up and running in seconds.",
    whyTitle: translations?.whyTitle ?? "Why does this happen?",
    whyText: translations?.whyText ?? "TTP is free and open source. Apple and Microsoft charge developers hundreds of dollars per year for a certificate that removes security warnings. We chose to keep TTP free instead. The app is fully open source — you can inspect every line of code on GitHub.",
    macTab: translations?.macTab ?? "macOS",
    winTab: translations?.winTab ?? "Windows",
    macAlreadyTitle: translations?.macAlreadyTitle ?? "Already downloaded? Open Terminal and run:",
    macAlreadyDesc: translations?.macAlreadyDesc ?? "This removes Apple's quarantine flag. That's it — the app opens normally after this.",
    macOneLineTitle: translations?.macOneLineTitle ?? "Install with one command:",
    macOneLineDesc: translations?.macOneLineDesc ?? "Downloads TTP, installs it, removes the quarantine flag, and launches it. Works on Apple Silicon and Intel.",
    macAltTitle: translations?.macAltTitle ?? "Alternative: no Terminal needed",
    macAltStep1: translations?.macAltStep1 ?? "Open the app once (it will be blocked)",
    macAltStep2: translations?.macAltStep2 ?? 'Go to System Settings → Privacy & Security',
    macAltStep3: translations?.macAltStep3 ?? 'Click "Open Anyway" and enter your password',
    winDownloadTitle: translations?.winDownloadTitle ?? "Download for Windows:",
    winDownloadButton: translations?.winDownloadButton ?? "Download TTP for Windows",
    winDownloadDesc: translations?.winDownloadDesc ?? "Downloads the installer (.exe). Run it to install TTP.",
    winSmartScreenTitle: translations?.winSmartScreenTitle ?? "Windows SmartScreen warning",
    winDesc: translations?.winDesc ?? "Windows SmartScreen may show a warning on first launch.",
    winStep1: translations?.winStep1 ?? 'Click "More info"',
    winStep2: translations?.winStep2 ?? 'Click "Run anyway"',
    copied: translations?.copied ?? "Copied!",
    clickToCopy: translations?.clickToCopy ?? "Copy",
  };

  const [activeTab, setActiveTab] = useState<"mac" | "win">("mac");

  return (
    <section
      id="install"
      data-snap-section
      className="relative flex flex-col items-center justify-center px-6 py-32 md:py-48"
    >
      <h2 className="text-heading font-bold tracking-tight">
        <span className="inline-block bg-gradient-to-r from-white to-slate-400 bg-clip-text text-transparent">
          {t.heading}
        </span>
      </h2>
      <p className="mt-4 max-w-lg text-center text-body-lg text-slate-400">
        {t.subheading}
      </p>

      {/* OS Tabs */}
      <div className="mt-8 flex gap-2">
        <button
          onClick={() => setActiveTab("mac")}
          className={`rounded-lg px-5 py-2 text-sm font-medium transition-all ${
            activeTab === "mac"
              ? "bg-white/10 text-white"
              : "text-slate-500 hover:text-slate-300"
          }`}
        >
          {t.macTab}
        </button>
        <button
          onClick={() => setActiveTab("win")}
          className={`rounded-lg px-5 py-2 text-sm font-medium transition-all ${
            activeTab === "win"
              ? "bg-white/10 text-white"
              : "text-slate-500 hover:text-slate-300"
          }`}
        >
          {t.winTab}
        </button>
      </div>

      {/* macOS content */}
      {activeTab === "mac" && (
        <div className="mt-6 w-full max-w-2xl space-y-6">
          {/* Primary: One-liner install */}
          <div className="rounded-xl border border-emerald-500/20 bg-emerald-500/[0.03] p-6">
            <h3 className="text-sm font-semibold text-white">{t.macOneLineTitle}</h3>
            <div className="mt-3 flex items-start gap-3 rounded-lg bg-black/40 px-4 py-3">
              <code className="flex-1 overflow-x-auto text-sm text-emerald-400 whitespace-nowrap">
                {MAC_ONE_LINER}
              </code>
              <CopyButton text={MAC_ONE_LINER} copiedLabel={t.copied} copyLabel={t.clickToCopy} />
            </div>
            <p className="mt-2 text-xs text-slate-500">{t.macOneLineDesc}</p>
          </div>

          {/* Alternative: no terminal */}
          <div className="rounded-xl border border-white/10 bg-white/[0.02] p-6">
            <h3 className="text-sm font-semibold text-white">{t.macAltTitle}</h3>
            <ol className="mt-3 space-y-2 text-sm text-slate-400">
              <li className="flex gap-3">
                <span className="shrink-0 font-mono text-slate-600">1.</span>
                {t.macAltStep1}
              </li>
              <li className="flex gap-3">
                <span className="shrink-0 font-mono text-slate-600">2.</span>
                {t.macAltStep2}
              </li>
              <li className="flex gap-3">
                <span className="shrink-0 font-mono text-slate-600">3.</span>
                {t.macAltStep3}
              </li>
            </ol>
          </div>

          {/* Why explanation */}
          <div className="rounded-xl border border-white/10 bg-white/[0.02] p-6">
            <h3 className="text-sm font-semibold uppercase tracking-wide text-slate-300">
              {t.whyTitle}
            </h3>
            <p className="mt-2 text-sm leading-relaxed text-slate-400">
              {t.whyText}
            </p>
          </div>
        </div>
      )}

      {/* Windows content */}
      {activeTab === "win" && (
        <div className="mt-6 w-full max-w-2xl space-y-6">
          {/* Primary: Download button */}
          <div className="rounded-xl border border-emerald-500/20 bg-emerald-500/[0.03] p-6">
            <h3 className="text-sm font-semibold text-white">{t.winDownloadTitle}</h3>
            <div className="mt-3">
              <a
                href={windowsDownloadUrl || WINDOWS_FALLBACK_URL}
                download
                className="inline-flex items-center gap-3 rounded-lg border border-emerald-500/30 bg-emerald-500/10 px-6 py-3 text-sm font-semibold text-emerald-400 transition-all hover:bg-emerald-500/20 hover:text-emerald-300"
              >
                {/* Windows icon */}
                <svg
                  className="h-5 w-5 flex-shrink-0"
                  viewBox="0 0 24 24"
                  fill="currentColor"
                >
                  <path d="M3 12V6.5l8-1.1V12H3zm0 .5h8v6.6l-8-1.1V12.5zm9 0h9V3l-9 1.2V12.5zm0 .5v6.3L21 21v-8.5h-9z" />
                </svg>
                {t.winDownloadButton}
              </a>
            </div>
            <p className="mt-2 text-xs text-slate-500">{t.winDownloadDesc}</p>
          </div>

          {/* SmartScreen instructions */}
          <div className="rounded-xl border border-white/10 bg-white/[0.02] p-6">
            <h3 className="text-sm font-semibold text-white">{t.winSmartScreenTitle}</h3>
            <p className="mt-2 text-sm text-slate-400">{t.winDesc}</p>
            <ol className="mt-4 space-y-2 text-sm text-slate-400">
              <li className="flex gap-3">
                <span className="shrink-0 font-mono text-slate-600">1.</span>
                {t.winStep1}
              </li>
              <li className="flex gap-3">
                <span className="shrink-0 font-mono text-slate-600">2.</span>
                {t.winStep2}
              </li>
            </ol>
          </div>

          {/* Why explanation */}
          <div className="rounded-xl border border-white/10 bg-white/[0.02] p-6">
            <h3 className="text-sm font-semibold uppercase tracking-wide text-slate-300">
              {t.whyTitle}
            </h3>
            <p className="mt-2 text-sm leading-relaxed text-slate-400">
              {t.whyText}
            </p>
          </div>
        </div>
      )}
    </section>
  );
}
