import { useSyncExternalStore } from 'react'

function subscribe(onChange: () => void) {
  addEventListener('online', onChange)
  addEventListener('offline', onChange)
  return () => {
    removeEventListener('online', onChange)
    removeEventListener('offline', onChange)
  }
}

/** Le navigateur se croit en ligne (`navigator.onLine`) ; suivi en direct. */
export function useOnline() {
  return useSyncExternalStore(subscribe, () => navigator.onLine)
}
