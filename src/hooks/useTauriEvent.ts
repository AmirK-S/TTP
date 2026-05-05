// TTP - Talk To Paste
// Safe Tauri event listener hook.
//
// Why this exists:
// `listen()` returns Promise<UnlistenFn>. The naive cleanup pattern
//   `return () => { unlisten.then(fn => fn()); }`
// races on unmount: an event that fires after unmount but before the
// Promise resolves can crash inside the Tauri JS runtime with
// "undefined is not an object (evaluating 'listeners[eventId].handlerId')".
//
// This hook:
// 1. Re-registers only when `event` changes (callback is held in a ref).
// 2. Awaits the unlisten Promise; if the component unmounts mid-await,
//    the unlisten is invoked the moment it resolves.
// 3. Guards the user callback with an `isMounted` flag so late events
//    after unmount don't touch freed state.

import { useEffect, useRef } from 'react';
import { listen, UnlistenFn, EventCallback } from '@tauri-apps/api/event';

export function useTauriEvent<T = unknown>(event: string, callback: EventCallback<T>) {
  const cbRef = useRef(callback);
  cbRef.current = callback;

  useEffect(() => {
    let mounted = true;
    let unlisten: UnlistenFn | null = null;

    listen<T>(event, (e) => {
      if (mounted) cbRef.current(e);
    })
      .then((fn) => {
        if (mounted) unlisten = fn;
        else fn();
      })
      .catch((err) => console.error(`[useTauriEvent] failed to listen ${event}:`, err));

    return () => {
      mounted = false;
      unlisten?.();
    };
  }, [event]);
}
