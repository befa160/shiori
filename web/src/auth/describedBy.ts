/** `aria-describedby` d'un contrôle de `Field`. */
export const describedBy = (id: string, help: boolean, error: boolean) =>
  [help && `${id}-help`, error && `${id}-error`].filter(Boolean).join(' ') || undefined
