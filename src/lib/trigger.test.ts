import { describe, it, expect } from 'vitest';
import { triggerLabel } from './trigger';

const t = (key: string, opts?: Record<string, unknown>) =>
  ({
    'trigger.side.right': `${opts?.key} right`,
    'trigger.side.left': `${opts?.key} left`,
    'trigger.keys.space': 'Space',
    'trigger.mouseMiddle': 'Middle click',
    'trigger.mouseButton': `Mouse button ${opts?.n}`,
  })[key] ?? key;

describe('triggerLabel', () => {
  it('names fn and sided modifiers', () => {
    expect(triggerLabel({ kind: 'fn' }, t)).toBe('fn');
    expect(triggerLabel({ kind: 'modifier', code: 54 }, t)).toBe('⌘ right');
    expect(triggerLabel({ kind: 'modifier', code: 58 }, t)).toBe('⌥ left');
  });

  it('prints modifiers in menu order before the key', () => {
    expect(triggerLabel({ kind: 'key', code: 49, mods: 0x080000 }, t)).toBe('⌥ Space');
    expect(triggerLabel({ kind: 'key', code: 15, mods: 0x100000 | 0x020000 }, t, 'r')).toBe('⇧⌘ R');
  });

  it('uses the layout character for keys without a fixed name', () => {
    // Keycode 0 types "q" on AZERTY.
    expect(triggerLabel({ kind: 'key', code: 0, mods: 0x040000 }, t, 'q')).toBe('⌃ Q');
    expect(triggerLabel({ kind: 'key', code: 105, mods: 0 }, t)).toBe('F13');
  });

  it('numbers mouse buttons the way people count them', () => {
    expect(triggerLabel({ kind: 'mouse', button: 2 }, t)).toBe('Middle click');
    expect(triggerLabel({ kind: 'mouse', button: 3 }, t)).toBe('Mouse button 4');
  });
});
