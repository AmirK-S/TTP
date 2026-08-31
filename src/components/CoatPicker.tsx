// TTP - Talk To Paste
// The coat picker.
//
// Two rules shaped this component, both from docs/ttp-pro-design.md.
//
// 1. **A locked coat is shown, not hidden, and shown at full fidelity.** The
//    specimen for a coat you do not own renders exactly as brightly as the one
//    you do. A list that hides its own contents cannot tempt anybody, and a
//    greyed-out smudge is a worse advertisement than the real thing.
// 2. **No second ask.** There is exactly one discoverable mention of the
//    purchase in Settings and it does not move — it lives in the Support
//    section. So there is no link here, no button, no badge that pulses. A
//    locked coat says where it comes from in one flat line and stops talking.

import { useTranslation } from 'react-i18next';
import { cn } from '../lib/cn';
import { COATS } from '../lib/theme-coats';

/**
 * A live miniature of the app wearing one coat.
 *
 * Not a screenshot and not a hand-copied swatch of hex values — the tile
 * carries `data-ttp-theme`, and the coat layer resolves its tokens on any
 * element that does, so this is the real coat painting itself. It therefore
 * cannot drift from what you get when you pick it, it follows light and dark
 * on its own, and a new coat needs no artwork.
 *
 * What it deliberately shows, because a palette swap is not a coat: the canvas
 * against the panel, the corner radius, the display face, the border weight,
 * whether anything casts a shadow, how far apart the coat likes things
 * (`--ttp-spec-gap`), and the accent.
 *
 * The little floating pill at the bottom right is drawn with the pill tokens,
 * with the voice bars rather than a face. The bars are what every TTP has; the
 * face is a separate choice that this picker has no opinion about.
 */
function Specimen({ coat, wide }: { coat: string; wide?: boolean }) {
  return (
    <div
      data-ttp-theme={coat}
      aria-hidden
      className={cn(
        'relative w-full overflow-hidden rounded-app-md border border-app-border bg-app-bg bg-noise',
        wide ? 'h-[108px]' : 'h-[92px]',
      )}
    >
      <div className="absolute inset-0 flex">
        {/* The sidebar, so the canvas is visible next to the panel — some
            coats put most of their character in the gap between the two. */}
        <div className="flex w-[22%] shrink-0 flex-col gap-1 border-r border-app-border bg-app-dim px-1.5 py-1.5">
          <span className="h-[4px] w-full rounded-full bg-app-accent opacity-80" />
          <span className="h-[3px] w-4/5 rounded-full bg-app-muted opacity-25" />
          <span className="h-[3px] w-3/5 rounded-full bg-app-muted opacity-25" />
        </div>
        <div className="min-w-0 flex-1 p-1.5">
          <div
            className="flex h-full flex-col rounded-app-sm border border-app-border bg-app-surface p-1.5 shine-sm"
            style={{ gap: 'var(--ttp-spec-gap)' }}
          >
            {/* Real display type at a real size, so the serif coat reads as a
                serif coat and the monospace one as a monospace one. */}
            <span className="text-display-xs !text-[10px] !leading-none text-app-text">TTP</span>
            <span className="h-[3px] w-full rounded-full bg-app-muted opacity-30" />
            <span className="h-[3px] w-4/5 rounded-full bg-app-muted opacity-30" />
            {wide && <span className="h-[3px] w-[88%] rounded-full bg-app-muted opacity-30" />}
            <div className="mt-auto flex items-center gap-1.5">
              <span className="h-3.5 w-10 rounded-app-xs bg-app-accent" />
              <span className="h-3.5 w-7 rounded-app-xs border border-app-border-strong" />
            </div>
          </div>
        </div>
      </div>
      {/* The pill, as it floats over a desktop: its own tokens, not the
          window's, because that is how it actually renders. Voice bars, not a
          face — the bars are what every TTP has. */}
      <span
        className="absolute bottom-1.5 right-1.5 flex h-4 items-center gap-[2px] rounded-full px-1.5"
        style={{
          background: 'var(--ttp-pill)',
          boxShadow: '0 1px 3px rgba(0,0,0,0.35), inset 0 0 0 1px var(--ttp-pill-ring)',
        }}
      >
        {[3, 6, 9, 5, 2].map((h, i) => (
          <span
            key={i}
            style={{ height: `${h}px`, background: 'var(--ttp-pill-ink)', opacity: 0.85 }}
            className="w-[1.5px] rounded-full"
          />
        ))}
      </span>
    </div>
  );
}

export interface CoatPickerProps {
  /** Currently painted coat id. */
  value: string;
  /** True when the Companion is owned. Locked coats stay visible either way. */
  unlocked: boolean;
  onSelect: (id: string) => void;
  disabled?: boolean;
}

export function CoatPicker({ value, unlocked, onSelect, disabled }: CoatPickerProps) {
  const { t } = useTranslation();

  const free = COATS.filter((c) => c.free);
  const rest = COATS.filter((c) => !c.free);

  const tile = (coat: (typeof COATS)[number], wide: boolean) => {
    const locked = !coat.free && !unlocked;
    const selected = value === coat.id;
    return (
      <button
        key={coat.id}
        type="button"
        onClick={() => !locked && !disabled && onSelect(coat.id)}
        disabled={locked || disabled}
        aria-pressed={selected}
        className={cn(
          'group flex flex-col gap-2 rounded-app-lg border p-2.5 text-left',
          'transition-[border-color,background-color] duration-hover ease-app-out',
          selected ? 'border-app-accent bg-app-accent-tint' : 'border-app-border bg-app-surface',
          !locked && !disabled && 'hover:border-app-border-strong hover:bg-app-raised active:scale-[0.995]',
          // Locked tiles dim their *label*, never their specimen — the point of
          // showing a coat you do not own is that you can see it.
          locked && 'cursor-default',
        )}
      >
        <Specimen coat={coat.id} wide={wide} />
        <div className={cn('min-w-0 px-0.5', wide && 'flex items-baseline gap-2')}>
          <p
            className={cn(
              'text-[13px] font-medium shrink-0',
              selected ? 'text-app-accent' : 'text-app-text',
              locked && 'text-app-muted',
            )}
          >
            {t(`settings.appearance.coats.${coat.id}.name`)}
          </p>
          <p className={cn('text-[11px] leading-relaxed text-app-faint', !wide && 'mt-0.5')}>
            {t(`settings.appearance.coats.${coat.id}.desc`)}
          </p>
        </div>
      </button>
    );
  };

  return (
    <div>
      {/* The free coat gets the whole width and comes first. It is not the
          leftover option below the paid ones; it is the default, and most
          people will never change it. */}
      <div className="mb-3 flex flex-col">{free.map((c) => tile(c, true))}</div>
      {/* Said once, above the set, rather than stamped on each of the four
          tiles. They unlock together — there is one purchase and no drip
          feed — so there is one sentence. And it is a sentence, not a call to
          action: the single mention of the purchase lives in the Support
          section and does not move. */}
      {!unlocked && (
        <p className="mb-2 text-[12px] text-app-faint">{t('settings.appearance.lockedNote')}</p>
      )}
      <div className="grid grid-cols-2 gap-3">{rest.map((c) => tile(c, false))}</div>
      <p className="mt-3 text-[12px] leading-relaxed text-app-faint">
        {t('settings.appearance.footnote')}
      </p>
    </div>
  );
}
