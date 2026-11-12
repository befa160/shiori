import { Outlet } from 'react-router'

/** Cadre commun hors lecteur : thème sombre et zones sûres de la PWA. */
export function Layout() {
  return (
    <div className="min-h-dvh bg-background pt-safe pr-safe pb-safe pl-safe text-foreground">
      <main className="mx-auto max-w-5xl px-4 py-6">
        <Outlet />
      </main>
    </div>
  )
}
