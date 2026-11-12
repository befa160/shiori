import { useQuery } from '@tanstack/react-query'
import { ApiError, api } from '@/api/client'

export function Home() {
  const health = useQuery({ queryKey: ['health'], queryFn: ({ signal }) => api.health({ signal }) })
  return (
    <section className="space-y-2">
      <h1 className="text-2xl font-semibold">Shiori</h1>
      {health.isPending && <p className="text-muted-foreground">Connexion au serveur…</p>}
      {health.isError && (
        <p role="alert" className="text-destructive">
          {health.error instanceof ApiError && health.error.code === 'network'
            ? 'Serveur injoignable'
            : `Erreur : ${health.error.message}`}
        </p>
      )}
      {health.isSuccess && <p className="text-muted-foreground">{`Serveur Shiori ${health.data.version}`}</p>}
    </section>
  )
}
