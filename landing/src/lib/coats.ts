import type { ui, defaultLang } from '@/i18n/ui';
import type { FaceVarietyId } from '@/lib/face';

type Key = keyof (typeof ui)[typeof defaultLang];

export type CoatId = 'wild' | 'roan' | 'piebald' | 'merle' | 'tortie';

export interface Coat {
  id: CoatId;
  nameKey: Key;
  descKey: Key;
  /** Only `wild` ships with the free app; the other four arrive with the
   *  Companion. Said plainly rather than drawn as a padlock. */
  free: boolean;
}

export const COATS: readonly Coat[] = [
  { id: 'wild', nameKey: 'coat.wild.name', descKey: 'coat.wild.desc', free: true },
  { id: 'roan', nameKey: 'coat.roan.name', descKey: 'coat.roan.desc', free: false },
  { id: 'piebald', nameKey: 'coat.piebald.name', descKey: 'coat.piebald.desc', free: false },
  { id: 'merle', nameKey: 'coat.merle.name', descKey: 'coat.merle.desc', free: false },
  { id: 'tortie', nameKey: 'coat.tortie.name', descKey: 'coat.tortie.desc', free: false },
];

export interface FaceEntry {
  id: FaceVarietyId;
  nameKey: Key;
  descKey: Key;
}

export const FACES: readonly FaceEntry[] = [
  { id: 'house', nameKey: 'face.house.name', descKey: 'face.house.desc' },
  { id: 'shut', nameKey: 'face.shut.name', descKey: 'face.shut.desc' },
  { id: 'drowsy', nameKey: 'face.drowsy.name', descKey: 'face.drowsy.desc' },
  { id: 'quick', nameKey: 'face.quick.name', descKey: 'face.quick.desc' },
  { id: 'bead', nameKey: 'face.bead.name', descKey: 'face.bead.desc' },
];

export interface PackEntry {
  id: string;
  nameKey: Key;
  descKey: Key;
}

export const PACKS: readonly PackEntry[] = [
  { id: 'default', nameKey: 'pack.default.name', descKey: 'pack.default.desc' },
  { id: 'bowl', nameKey: 'pack.bowl.name', descKey: 'pack.bowl.desc' },
  { id: 'marimba', nameKey: 'pack.marimba.name', descKey: 'pack.marimba.desc' },
  { id: 'submarine', nameKey: 'pack.submarine.name', descKey: 'pack.submarine.desc' },
  { id: 'felt', nameKey: 'pack.felt.name', descKey: 'pack.felt.desc' },
  { id: 'bubble', nameKey: 'pack.bubble.name', descKey: 'pack.bubble.desc' },
];

export const BUY_URL =
  'https://amirks.lemonsqueezy.com/checkout/buy/23ded1c4-c862-4f8c-ada5-0bb3dc2e0060';
export const REPO_URL = 'https://github.com/AmirK-S/TTP';
export const GROQ_CONSOLE = 'https://console.groq.com';
