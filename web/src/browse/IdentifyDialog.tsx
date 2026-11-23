import { useMutation } from '@tanstack/react-query'
import { ExternalLink, Search } from 'lucide-react'
import { useId, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { api, thumbnailUrl } from '@/api/client'
import type { SeriesDetail, SourceId, WebMetadata } from '@/api/types'
import { Button } from '@/components/ui/button'
import { DialogContent, DialogDescription, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Credits } from '@/admin/Metadata'
import { errorText, sourceErrorText, toastFailure } from '@/i18n/errors'
import { FallbackCover } from '@/library/Covers'
import { useSeriesAction } from './series'

const SOURCES: SourceId[] = ['mangadex', 'anilist', 'wikipedia']
/** Page de la fiche chez sa source ; un identifiant Wikipédia est `langue:Titre de l'article`. */
const page = (source: SourceId, id: string) => {
  if (source !== 'wikipedia') return `${source === 'anilist' ? 'https://anilist.co/manga/' : 'https://mangadex.org/title/'}${encodeURIComponent(id)}`
  const [lang, ...title] = id.split(':')
  return `https://${lang === 'en' ? 'en' : 'fr'}.wikipedia.org/wiki/${encodeURIComponent(title.join(':').replaceAll(' ', '_'))}`
}

/** Fenêtre « Identifier » façon Plex (lot 5.7b) : en tête, chaque source liée, à actualiser, dont
 * prendre la couverture ou à délier (lot 10.2) ; puis choisir une source, chercher, et choisir la
 * bonne fiche parmi des couvertures. Les vignettes sont relayées par le serveur : le navigateur ne
 * contacte pas les sources. */
export function IdentifyDialog({ series }: { series: SeriesDetail }) {
  const { t, i18n } = useTranslation()
  const id = useId()
  // D'abord une source qui a des fiches à confirmer, sinon celle de la langue (Q28).
  const pending = series.sources.find((s) => s.candidates.length > 0)?.source
  const [source, setSource] = useState<SourceId>(pending ?? (i18n.language === 'en' ? 'anilist' : 'mangadex'))
  const [query, setQuery] = useState(series.name)
  const [error, setError] = useState<string | null>(null)
  const state = series.sources.find((s) => s.source === source)
  const search = useMutation({
    mutationFn: () => api.searchMetadata(series.id, source, query.trim()),
    onMutate: () => setError(null),
    onError: (e) => setError(errorText(e, t)),
  })
  const failed = toastFailure(t('series.identifyFailed'), t)
  const link = useSeriesAction(series.id, (externalId: string) => api.linkSource(series.id, source, externalId))
  const refresh = useSeriesAction(series.id, () => api.refreshMetadata(series.id))
  const cover = useSeriesAction(series.id, (s: SourceId) => api.useSourceCover(series.id, s))
  const unlink = useSeriesAction(series.id, (s: SourceId) => api.unlinkSource(series.id, s))
  const linked = series.sources.filter((s) => s.external_id)
  const unlinked = series.sources.filter((s) => s.unlinked)
  const busy = link.isPending || refresh.isPending || cover.isPending || unlink.isPending
  // Résultats d'une recherche de cette source, sinon les fiches gardées à confirmer.
  const searched = search.isSuccess
  const results = search.data ?? state?.candidates ?? []

  return (
    <DialogContent closeLabel={t('common.close')} className="sm:max-w-3xl">
      <DialogHeader>
        <DialogTitle>{t('series.identifyTitle', { name: series.name })}</DialogTitle>
        <DialogDescription>{t('series.identifyDescription')}</DialogDescription>
      </DialogHeader>
      {linked.length > 0 && (
        <section aria-labelledby={`${id}-linked`} className="space-y-2">
          <h3 id={`${id}-linked`} className="text-sm font-medium">
            {t('series.identifyLinkedList')}
          </h3>
          <ul className="space-y-2">
            {linked.map((s) => (
              <li key={s.source} className="space-y-2 rounded-md bg-background p-3">
                <p className="text-sm">
                  <span className="font-medium">{t(`admin.sourceNames.${s.source}`)}</span> ·{' '}
                  {t(s.matched_by === 'user' ? 'series.identifyUser' : 'series.identifyAuto')}{' '}
                  <a
                    href={page(s.source, s.external_id ?? '')}
                    target="_blank"
                    rel="noreferrer noopener"
                    className="inline-flex min-h-11 items-center gap-1 underline underline-offset-4"
                  >
                    {t('series.identifyOpen')}
                    <ExternalLink aria-hidden className="size-4" />
                  </a>
                </p>
                {s.last_error && (
                  <p className="text-sm text-destructive">{t('series.identifyLastError', { error: sourceErrorText(s.last_error, t) })}</p>
                )}
                {/* Un groupe nommé : avec deux sources liées, « Délier » dit laquelle au lecteur d'écran. */}
                <div role="group" aria-label={t(`admin.sourceNames.${s.source}`)} className="flex flex-wrap gap-2">
                  <Button variant="secondary" disabled={busy} onClick={() => refresh.mutate(undefined, { onError: failed })}>
                    {t('series.identifyRefresh')}
                  </Button>
                  {/* Wikipédia n'a pas de couverture. */}
                  {s.source !== 'wikipedia' && (
                    <Button
                      variant="secondary"
                      disabled={busy}
                      onClick={() =>
                        cover.mutate(s.source, { onSuccess: () => toast.success(t('series.identifyCoverDone')), onError: failed })
                      }
                    >
                      {t('series.identifyUseCover')}
                    </Button>
                  )}
                  <Button variant="ghost" disabled={busy} onClick={() => unlink.mutate(s.source, { onError: failed })}>
                    {t('series.identifyUnlink')}
                  </Button>
                </div>
              </li>
            ))}
          </ul>
        </section>
      )}
      {unlinked.map((s) => (
        <p key={s.source} className="text-sm text-muted-foreground">
          {t('series.identifyUnlinked', { source: t(`admin.sourceNames.${s.source}`) })}
        </p>
      ))}

      <form
        className="grid min-w-0 gap-3 sm:grid-cols-[12rem_1fr_auto] sm:items-end"
        onSubmit={(e) => {
          e.preventDefault()
          if (query.trim()) search.mutate()
        }}
      >
        {/* `grid gap` plutôt que `space-y` : le <select> caché de Radix ne décale pas les libellés. */}
        <div className="grid gap-1.5">
          <label htmlFor={`${id}-source`} className="text-sm font-medium">
            {t('series.identifySource')}
          </label>
          <Select
            value={source}
            onValueChange={(v) => {
              setSource(v as SourceId)
              search.reset()
            }}
          >
            <SelectTrigger id={`${id}-source`} className="w-full">
              <SelectValue />
            </SelectTrigger>
            <SelectContent position="popper" align="start">
              {SOURCES.map((s) => (
                <SelectItem key={s} value={s}>
                  {t(`admin.sourceNames.${s}`)}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className="grid min-w-0 gap-1.5">
          <label htmlFor={`${id}-query`} className="text-sm font-medium">
            {t('series.identifyQuery')}
          </label>
          <Input id={`${id}-query`} value={query} onChange={(e) => setQuery(e.target.value)} />
        </div>
        <Button type="submit" variant="secondary" disabled={search.isPending || !query.trim()}>
          <Search aria-hidden />
          {t('series.identifySearch')}
        </Button>
      </form>
      {error && (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}

      {(searched || results.length > 0) && (
        <section aria-labelledby={`${id}-results`} className="min-w-0 space-y-2">
          <h3 id={`${id}-results`} className="text-sm font-medium">
            {t(searched ? 'series.identifyResults' : 'series.identifyToConfirm')}
          </h3>
          {results.length === 0 ? (
            <p className="rounded-md bg-background p-3 text-sm text-muted-foreground">{t('series.identifyNone')}</p>
          ) : (
            <ul className="grid grid-cols-[repeat(auto-fill,minmax(7.5rem,1fr))] gap-3">
              {results.map((fiche) => (
                <li key={fiche.id}>
                  <Candidate
                    fiche={fiche}
                    source={source}
                    linked={state?.external_id === fiche.id}
                    disabled={busy}
                    onChoose={() =>
                      link.mutate(fiche.id, { onSuccess: () => toast.success(t('series.identifyDone')), onError: failed })
                    }
                  />
                </li>
              ))}
            </ul>
          )}
        </section>
      )}

      <Credits />
    </DialogContent>
  )
}

/** Un candidat : sa couverture (vignette relayée), son titre, son année et ses auteurs ; le
 * bouton entier le choisit. */
function Candidate({
  fiche,
  source,
  linked,
  disabled,
  onChoose,
}: {
  fiche: WebMetadata
  source: SourceId
  linked: boolean
  disabled: boolean
  onChoose: () => void
}) {
  const { t } = useTranslation()
  const [failed, setFailed] = useState(false)
  const title = fiche.titles[0]?.text ?? fiche.id
  const label = [title, fiche.year].filter(Boolean).join(' · ')
  return (
    <button
      type="button"
      disabled={disabled}
      aria-pressed={linked}
      aria-label={t('series.identifyChoose', { title: label })}
      onClick={onChoose}
      className="group w-full space-y-1 rounded-md text-left disabled:opacity-60"
    >
      <div className="relative aspect-[2/3] overflow-hidden rounded-md bg-card ring-2 ring-transparent group-hover:ring-foreground/15 group-aria-pressed:ring-primary">
        {failed ? (
          <FallbackCover title={title} />
        ) : (
          <img
            src={thumbnailUrl(source, fiche.id)}
            alt=""
            loading="lazy"
            decoding="async"
            onError={() => setFailed(true)}
            className="size-full object-cover"
          />
        )}
      </div>
      <p className="line-clamp-2 text-sm font-medium break-words">{title}</p>
      <p className="line-clamp-1 text-sm text-muted-foreground">
        {[fiche.year, fiche.authors.map((a) => a.name).join(', ')].filter(Boolean).join(' · ')}
      </p>
    </button>
  )
}
