import { Link } from 'react-router'

export function NotFound() {
  return (
    <section className="space-y-3">
      <h1 className="text-xl font-semibold">Page introuvable</h1>
      <Link to="/" className="text-primary underline-offset-4 hover:underline">
        Retour à l'accueil
      </Link>
    </section>
  )
}
