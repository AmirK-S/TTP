// TTP - Talk To Paste
// The dictation trigger as the UI sees it: the shape Rust stores
// (`src-tauri/src/trigger.rs`) and how to name it for a person.

export type Trigger =
  | { kind: 'fn' }
  | { kind: 'modifier'; code: number }
  | { kind: 'key'; code: number; mods: number }
  | { kind: 'mouse'; button: number };

/** What `trigger-captured` carries. */
export type CaptureResult =
  | { status: 'captured'; trigger: Trigger }
  | { status: 'rejected'; reason: 'types_a_character' | 'caps_lock' }
  | { status: 'cancelled' | 'timed_out' | 'abandoned' };

// CGEventFlags, in the order macOS menus print them.
const MOD_SYMBOLS: [number, string][] = [
  [0x040000, '⌃'],
  [0x080000, '⌥'],
  [0x020000, '⇧'],
  [0x100000, '⌘'],
];

const MODIFIER_KEYS: Record<number, { symbol: string; side: 'left' | 'right' }> = {
  55: { symbol: '⌘', side: 'left' },
  54: { symbol: '⌘', side: 'right' },
  58: { symbol: '⌥', side: 'left' },
  61: { symbol: '⌥', side: 'right' },
  56: { symbol: '⇧', side: 'left' },
  60: { symbol: '⇧', side: 'right' },
  59: { symbol: '⌃', side: 'left' },
  62: { symbol: '⌃', side: 'right' },
};

/** Keys whose name is not the character they type. Translation keys under
 *  `trigger.keys`, or a literal glyph. */
const NAMED_KEYS: Record<number, string> = {
  49: 'trigger.keys.space',
  36: 'trigger.keys.return',
  48: 'trigger.keys.tab',
  51: '⌫',
  117: '⌦',
  53: 'Esc',
  123: '←',
  124: '→',
  125: '↓',
  126: '↑',
  115: '↖',
  119: '↘',
  116: '⇞',
  121: '⇟',
  114: 'Help',
  71: 'Clear',
  122: 'F1', 120: 'F2', 99: 'F3', 118: 'F4', 96: 'F5', 97: 'F6', 98: 'F7', 100: 'F8',
  101: 'F9', 109: 'F10', 103: 'F11', 111: 'F12', 105: 'F13', 107: 'F14', 113: 'F15',
  106: 'F16', 64: 'F17', 79: 'F18', 80: 'F19', 90: 'F20',
};

type T = (key: string, opts?: Record<string, unknown>) => string;

/**
 * A person-readable name for a trigger: "fn", "⌘ right", "⌥ Space", "F13",
 * "Mouse button 4". `chars` is what the key types on the current layout
 * (from `trigger_key_chars`), used for keys that have no fixed name.
 */
export function triggerLabel(trigger: Trigger, t: T, chars = ''): string {
  switch (trigger.kind) {
    case 'fn':
      return 'fn';
    case 'modifier': {
      const m = MODIFIER_KEYS[trigger.code];
      return m ? t(`trigger.side.${m.side}`, { key: m.symbol }) : `#${trigger.code}`;
    }
    case 'key': {
      const mods = MOD_SYMBOLS.filter(([bit]) => trigger.mods & bit).map(([, s]) => s).join('');
      const named = NAMED_KEYS[trigger.code];
      const name = named
        ? (named.startsWith('trigger.') ? t(named) : named)
        : (chars.trim() ? chars.toUpperCase() : `#${trigger.code}`);
      return mods ? `${mods} ${name}` : name;
    }
    case 'mouse':
      return trigger.button === 2
        ? t('trigger.mouseMiddle')
        : t('trigger.mouseButton', { n: trigger.button + 1 });
  }
}
