import { LazyMotion, domAnimation, m } from "framer-motion";
import { DownloadButtons } from "./DownloadButtons";
import { NumberTicker } from "./NumberTicker";

interface DownloadSectionProps {
  downloadLinks?: { mac: string; windows: string };
  totalDownloads?: number;
  translations?: {
    heading: string;
    subheading: string;
    downloads: string;
    downloadFor: string;
  };
  locale?: string;
}

export function DownloadSection({ downloadLinks, totalDownloads, translations, locale = "en" }: DownloadSectionProps) {
  const heading = translations?.heading ?? "Get Started in Seconds";
  const subheading = translations?.subheading ?? "Available for macOS and Windows. Free forever.";
  const downloadsText = translations?.downloads ?? "downloads";
  const downloadForText = translations?.downloadFor ?? "Download for";

  const localeTag = locale === "fr" ? "fr-FR" : "en-US";

  return (
    <LazyMotion features={domAnimation}>
    <m.section
      id="download"
      data-snap-section
      className="relative flex flex-col items-center justify-center px-6 py-32 md:py-48"
      initial={{ opacity: 0, y: 20 }}
      whileInView={{ opacity: 1, y: 0 }}
      viewport={{ once: true, amount: 0.1 }}
      transition={{ duration: 0.6, ease: "easeOut" }}
    >
      {/* Section heading */}
      <h2 className="text-heading font-bold tracking-tight">
        <span className="inline-block bg-gradient-to-r from-white to-slate-400 bg-clip-text text-transparent">
          {heading}
        </span>
      </h2>
      <p className="mt-4 max-w-lg text-center text-body-lg text-slate-400">
        {subheading}
      </p>

      {/* Download buttons */}
      <div className="mt-10">
        <DownloadButtons downloadLinks={downloadLinks} translations={{ downloadFor: downloadForText }} locale={localeTag} />
      </div>

      {/* Download count */}
      {totalDownloads != null && totalDownloads > 0 && (
        <div className="mt-8 flex items-center gap-2 text-slate-500">
          {/* Download icon */}
          <svg
            className="h-4 w-4"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
            <polyline points="7 10 12 15 17 10" />
            <line x1="12" y1="15" x2="12" y2="3" />
          </svg>
          <span className="text-sm">
            <NumberTicker
              value={totalDownloads}
              className="font-semibold text-slate-400"
              locale={localeTag}
            />{" "}
            {downloadsText}
          </span>
        </div>
      )}
    </m.section>
    </LazyMotion>
  );
}
