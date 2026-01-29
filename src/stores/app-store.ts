// TTP - Talk To Paste
// Global application state store using Zustand

import { create } from 'zustand';

interface AppStore {
  /** Whether the user has configured an API key */
  hasApiKey: boolean;
  setHasApiKey: (value: boolean) => void;
}

export const useAppStore = create<AppStore>((set) => ({
  hasApiKey: false,
  setHasApiKey: (value) => set({ hasApiKey: value }),
}));
