import { LazyMotion, domAnimation, m } from "framer-motion";
import type { ReleaseData } from "@/lib/github";

interface ChangelogProps {
  releases?: ReleaseData[];
  translations?: {
    heading: string;
    subheading: string;
    empty: string;
    emptyLink: string;
    viewAll: string;
  };
  locale?: string;
}

function formatDate(dateString: string, locale: string): string {
  const date = new Date(dateString);
  return new Intl.DateTimeFormat(locale, {
    month: "short",
    day: "numeric",
    year: "numeric",
  }).format(date);
}

/** Simple markdown-to-JSX renderer for release notes. */
function renderMarkdown(body: string): React.ReactNode[] {
  const lines = body.split("\n");
  const elements: React.ReactNode[] = [];
  let bulletBuffer: string[] = [];
  let key = 0;

  function flushBullets() {
    if (bulletBuffer.length > 0) {
      elements.push(
        <ul key={key++} className="ml-4 list-disc space-y-1 text-sm text-slate-400">
          {bulletBuffer.map((item, i) => (
            <li key={i}>{renderInline(item)}</li>
          ))}
        </ul>
      );
      bulletBuffer = [];
    }
  }

  function renderInline(text: string): React.ReactNode {
    // Handle **bold** text
    const parts = text.split(/\*\*(.*?)\*\*/g);
    if (parts.length === 1) return text;
    return parts.map((part, i) =>
      i % 2 === 1 ? (
        <strong key={i} className="font-medium text-slate-300">
          {part}
        </strong>
      ) : (
        part
      )
    );
  }

  for (const line of lines) {
    const trimmed = line.trim();

    // Skip empty lines
    if (!trimmed) {
      flushBullets();
      continue;
    }

    // Heading (## or ###)
    if (trimmed.startsWith("## ") || trimmed.startsWith("### ")) {
      flushBullets();
      const headingText = trimmed.replace(/^#{2,3}\s+/, "");
      elements.push(
        <h4 key={key++} className="mt-3 mb-1.5 text-sm font-semibold text-slate-300">
          {headingText}
        </h4>
      );
      continue;
    }

    // Bullet point (- or *)
    if (/^[-*]\s+/.test(trimmed)) {
      bulletBuffer.push(trimmed.replace(/^[-*]\s+/, ""));
      continue;
    }

    // Regular text
    flushBullets();
    elements.push(
      <p key={key++} className="text-sm text-slate-400">
        {renderInline(trimmed)}
      </p>
    );
  }

  flushBullets();
  return elements;
}

export function Changelog({ releases, translations, locale = "en" }: ChangelogProps) {
  const headingText = translations?.heading ?? "Changelog";
  const subheadingText = translations?.subheading ?? "Every improvement, documented.";
  const emptyText = translations?.empty ?? "No releases yet. Check back soon!";
  const emptyLinkText = translations?.emptyLink ?? "View project on GitHub";
  const viewAllText = translations?.viewAll ?? "View all releases on GitHub";

  const localeTag = locale === "fr" ? "fr-FR" : "en-US";

  const releaseList = releases ?? [];

  return (
    <LazyMotion features={domAnimation}>
    <m.section
      id="changelog"
      data-snap-section
      className="relative px-6 py-32 md:py-48"
      initial={{ opacity: 0, y: 20 }}
      whileInView={{ opacity: 1, y: 0 }}
      viewport={{ once: true, amount: 0.1 }}
      transition={{ duration: 0.6, ease: "easeOut" }}
    >
      <div className="mx-auto max-w-3xl">
        {/* Section heading */}
        <div className="mb-20 md:mb-24 text-center">
          <h2 className="text-heading font-bold tracking-tight">
            <span className="inline-block bg-gradient-to-r from-white to-slate-400 bg-clip-text text-transparent">
              {headingText}
            </span>
          </h2>
          <p className="mt-4 text-body-lg text-slate-400">
            {subheadingText}
          </p>
        </div>

        {/* Timeline */}
        <div className="relative">
          {/* Vertical line */}
          <div
            className="absolute left-[5px] top-2 bottom-0 w-px bg-white/10"
            aria-hidden="true"
          />

          {/* Empty state */}
          {releaseList.length === 0 && (
            <div className="py-12 text-center">
              <p className="text-slate-400">
                {emptyText}
              </p>
              <a
                href="https://github.com/AmirK-S/TTP"
                target="_blank"
                rel="noopener noreferrer"
                className="mt-2 inline-block text-sm text-slate-500 underline underline-offset-2 hover:text-slate-300"
              >
                {emptyLinkText}
              </a>
            </div>
          )}

          {/* Timeline entries */}
          {releaseList.length > 0 &&
            releaseList.map((release, index) => (
              <m.div
                key={release.tag_name}
                className={`relative flex gap-6 pl-8 ${
                  index < releaseList.length - 1 ? "pb-10" : "pb-0"
                }`}
                initial={{ opacity: 0, y: 15 }}
                whileInView={{ opacity: 1, y: 0 }}
                viewport={{ once: true, amount: 0.1 }}
                transition={{ duration: 0.4, delay: index * 0.1 }}
              >
                {/* Timeline dot */}
                <div className="absolute left-0 top-1.5 h-[11px] w-[11px] rounded-full border-2 border-slate-700 bg-slate-400" />

                {/* Content */}
                <div className="flex-1">
                  {/* Version badge + date */}
                  <div className="mb-2 flex flex-wrap items-center gap-3">
                    <span className="rounded-full bg-white/10 px-2.5 py-0.5 text-xs font-medium text-white">
                      {release.tag_name}
                    </span>
                    <span className="text-xs text-slate-500">
                      {formatDate(release.published_at, localeTag)}
                    </span>
                  </div>

                  {/* Release title */}
                  {release.name && (
                    <h3 className="mb-2 text-base font-semibold text-white">
                      {release.name}
                    </h3>
                  )}

                  {/* Release notes */}
                  {release.body && (
                    <div className="space-y-1">
                      {renderMarkdown(release.body)}
                    </div>
                  )}
                </div>
              </m.div>
            ))}

          {/* "View all" link if there are releases */}
          {releaseList.length > 0 && (
            <div className="mt-8 text-center">
              <a
                href="https://github.com/AmirK-S/TTP/releases"
                target="_blank"
                rel="noopener noreferrer"
                className="text-sm text-slate-500 underline underline-offset-2 transition-colors hover:text-slate-300"
              >
                {viewAllText}
              </a>
            </div>
          )}
        </div>
      </div>
    </m.section>
    </LazyMotion>
  );
}
