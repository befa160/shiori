import type { ReactNode } from 'react'

/** Champ de formulaire : libellé visible, aide facultative, erreur sous le champ. Le contrôle
 * reçoit `id`, et `aria-describedby` vers `${id}-help` / `${id}-error` quand ils existent. */
export function Field({ id, label, help, error, children }: { id: string; label: string; help?: string; error?: string | null; children: ReactNode }) {
  return (
    <div className="space-y-1">
      <label htmlFor={id} className="text-sm font-medium">
        {label}
      </label>
      {help && (
        <p id={`${id}-help`} className="text-sm text-muted-foreground">
          {help}
        </p>
      )}
      {children}
      {error && (
        <p id={`${id}-error`} role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}
    </div>
  )
}
