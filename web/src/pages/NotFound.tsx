import { Link } from 'react-router'
import { Button } from '@/components/ui/button'

export function NotFound() {
  return (
    <section className="space-y-3">
      <h1 className="text-xl font-semibold">Page introuvable</h1>
      <Button asChild>
        <Link to="/">Retour à l'accueil</Link>
      </Button>
    </section>
  )
}
