import { useMutation, useQueryClient } from '@tanstack/react-query'
import type { TFunction } from 'i18next'
import { ChevronRight, Pencil, Plus, Trash2 } from 'lucide-react'
import { useId, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Link, useNavigate } from 'react-router'
import { toast } from 'sonner'
import { ApiError, api } from '@/api/client'
import { useAllSeries, useCollections } from '@/api/queries'
import type { Collection, SeriesDetail, SeriesSummary } from '@/api/types'
import { useIsAdmin } from '@/auth/session'
import { BackLink, QueryState } from '@/browse/Parts'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { errorText, toastFailure } from '@/i18n/errors'
import { CoverCard, CoverGrid, CoverSkeletons, SeriesCard, Shelf } from '@/library/Covers'

// Séries par étagère d'accueil : au-delà, « Tout voir ».
const SHELF_SIZE = 20

/** Erreurs d'un nom de collection : les codes communs parlent de bibliothèques. */
function nameError(error: Error, t: TFunction) {
  if (error instanceof ApiError && (error.code === 'name_taken' || error.code === 'invalid_name')) {
    return t(`collections.errors.${error.code}`)
  }
  return errorText(error, t)
}

/** Séries d'une collection, par titre, d'après la liste des séries déjà chargée. */
function members(collection: Collection, all: SeriesSummary[]) {
  const ids = new Set(collection.series_ids)
  return all.filter((s) => ids.has(s.id)).sort((a, b) => a.name.localeCompare(b.name) || a.id - b.id)
}

/** Après un changement : la liste des collections, et le détail des séries (leurs puces). */
function useRefresh() {
  const queryClient = useQueryClient()
  return () => {
    void queryClient.invalidateQueries({ queryKey: ['collections'] })
    void queryClient.invalidateQueries({ queryKey: ['series'] })
  }
}

function CollectionCard({ collection, all, eager }: { collection: Collection; all: SeriesSummary[]; eager?: boolean }) {
  const { t } = useTranslation()
  const first = members(collection, all)[0]
  return (
    <CoverCard
      to={`/collections/${collection.id}`}
      title={collection.name}
      subtitle={t('counts.series', { count: collection.series_ids.length })}
      coverPageId={first?.cover_page_id ?? null}
      coverImageId={first?.cover_image_id}
      progress={null}
      eager={eager}
    />
  )
}

/** Étagère « Collections » de l'accueil ; absente sans collection visible. */
export function CollectionsShelf({ all }: { all: SeriesSummary[] }) {
  const { t } = useTranslation()
  const list = (useCollections().data ?? []).filter((c) => c.series_ids.length > 0)
  if (list.length === 0) return null
  return (
    <section aria-labelledby="home-collections" className="space-y-3">
      <div className="flex items-center justify-between gap-4">
        <h2 id="home-collections" className="text-lg font-semibold">
          {t('collections.title')}
        </h2>
        <Link
          to="/collections"
          aria-label={t('home.seeAllNamed', { name: t('collections.title') })}
          className="flex h-11 shrink-0 items-center gap-1 rounded-md px-2 text-sm text-muted-foreground transition-colors hover:text-foreground active:bg-accent"
        >
          {t('home.seeAll')}
          <ChevronRight aria-hidden className="size-4" />
        </Link>
      </div>
      <Shelf>
        {list.slice(0, SHELF_SIZE).map((c) => (
          <li key={c.id}>
            <CollectionCard collection={c} all={all} />
          </li>
        ))}
      </Shelf>
    </section>
  )
}

/** `/collections` : toutes les collections ; l'administrateur en crée. */
export function CollectionsPage() {
  const { t } = useTranslation()
  const collections = useCollections()
  const all = useAllSeries().data ?? []
  const admin = useIsAdmin()
  const navigate = useNavigate()
  const [creating, setCreating] = useState(false)
  return (
    <section className="space-y-4">
      <title>{`${t('collections.title')} · Shiori`}</title>
      <div className="flex flex-wrap items-center justify-between gap-4">
        <h1 className="text-2xl font-semibold">{t('collections.title')}</h1>
        {admin && (
          <Button onClick={() => setCreating(true)}>
            <Plus aria-hidden />
            {t('collections.new')}
          </Button>
        )}
      </div>
      <QueryState
        query={collections}
        isEmpty={(list) => list.length === 0}
        empty={<p className="rounded-md bg-card p-4 text-muted-foreground">{t(admin ? 'collections.emptyAdmin' : 'collections.empty')}</p>}
        loading={<CoverSkeletons grid />}
      >
        {(list) => (
          <CoverGrid>
            {list.map((c, i) => (
              <li key={c.id}>
                <CollectionCard collection={c} all={all} eager={i < 12} />
              </li>
            ))}
          </CoverGrid>
        )}
      </QueryState>
      <Dialog open={creating} onOpenChange={setCreating}>
        {creating && (
          <NameDialog
            title={t('collections.newTitle')}
            initial=""
            submit={api.createCollection}
            onDone={(c) => {
              setCreating(false)
              if (c) void navigate(`/collections/${c.id}`)
            }}
          />
        )}
      </Dialog>
    </section>
  )
}

/** Page d'une collection : ses séries par titre ; renommer et supprimer (administrateur). */
export function CollectionPage({ id }: { id: number }) {
  const { t } = useTranslation()
  const collections = useCollections()
  const all = useAllSeries().data ?? []
  const admin = useIsAdmin()
  const navigate = useNavigate()
  const refresh = useRefresh()
  const [renaming, setRenaming] = useState(false)
  const [deleting, setDeleting] = useState(false)
  const remove = useMutation({
    mutationFn: () => api.deleteCollection(id),
    onSuccess: () => {
      refresh()
      toast.success(t('collections.deleted'))
      void navigate('/collections')
    },
    onError: toastFailure(t('collections.failed'), t),
  })
  const collection = collections.data?.find((c) => c.id === id)
  const name = collection?.name ?? t('collections.title')
  return (
    <section className="space-y-4">
      <title>{`${name} · Shiori`}</title>
      <BackLink to="/collections" label={t('collections.title')} />
      <QueryState query={collections} isEmpty={() => false} empty={null}>
        {() =>
          collection === undefined ? (
            <p role="alert">{t('collections.notFound')}</p>
          ) : (
            <>
              <div className="flex flex-wrap items-center justify-between gap-4">
                <h1 title={collection.name} className="line-clamp-2 min-w-0 text-2xl font-semibold break-words">
                  {collection.name}
                </h1>
                {admin && (
                  <div className="flex gap-2">
                    <Button variant="secondary" onClick={() => setRenaming(true)}>
                      <Pencil aria-hidden />
                      {t('collections.rename')}
                    </Button>
                    <Button variant="secondary" onClick={() => setDeleting(true)}>
                      <Trash2 aria-hidden />
                      {t('collections.delete')}
                    </Button>
                  </div>
                )}
              </div>
              {collection.series_ids.length === 0 ? (
                <p className="rounded-md bg-card p-4 text-muted-foreground">{t(admin ? 'collections.noSeriesAdmin' : 'collections.noSeries')}</p>
              ) : (
                <CoverGrid>
                  {members(collection, all).map((s, i) => (
                    <li key={s.id}>
                      <SeriesCard series={s} eager={i < 12} />
                    </li>
                  ))}
                </CoverGrid>
              )}
              <Dialog open={renaming} onOpenChange={setRenaming}>
                {renaming && (
                  <NameDialog
                    title={t('collections.renameTitle', { name: collection.name })}
                    initial={collection.name}
                    submit={(next) => api.renameCollection(id, next)}
                    onDone={() => setRenaming(false)}
                  />
                )}
              </Dialog>
              <AlertDialog open={deleting} onOpenChange={setDeleting}>
                <AlertDialogContent>
                  <AlertDialogTitle>{t('collections.deleteTitle', { name: collection.name })}</AlertDialogTitle>
                  <AlertDialogDescription>{t('collections.deleteDescription')}</AlertDialogDescription>
                  <AlertDialogFooter>
                    <AlertDialogCancel>{t('series.cancel')}</AlertDialogCancel>
                    <AlertDialogAction onClick={() => remove.mutate()}>{t('collections.delete')}</AlertDialogAction>
                  </AlertDialogFooter>
                </AlertDialogContent>
              </AlertDialog>
            </>
          )
        }
      </QueryState>
    </section>
  )
}

/** Fenêtre d'un nom de collection (création, renommage). `onDone` reçoit la collection enregistrée,
 * ou rien si on annule. */
function NameDialog({
  title,
  initial,
  submit,
  onDone,
}: {
  title: string
  initial: string
  submit: (name: string) => Promise<Collection>
  onDone: (saved?: Collection) => void
}) {
  const { t } = useTranslation()
  const id = useId()
  const refresh = useRefresh()
  const [name, setName] = useState(initial)
  const [error, setError] = useState<string | null>(null)
  const save = useMutation({
    mutationFn: submit,
    onSuccess: (saved) => {
      refresh()
      toast.success(t('series.saved'))
      onDone(saved)
    },
    onError: (e) => setError(nameError(e, t)),
  })
  return (
    <DialogContent closeLabel={t('common.close')}>
      <DialogHeader>
        <DialogTitle>{title}</DialogTitle>
        <DialogDescription>{t('collections.nameHelp')}</DialogDescription>
      </DialogHeader>
      <form
        noValidate
        className="space-y-4"
        onSubmit={(event) => {
          event.preventDefault()
          setError(null)
          save.mutate(name)
        }}
      >
        <div className="space-y-1.5">
          <label htmlFor={`${id}-name`} className="text-sm font-medium">
            {t('collections.name')}
          </label>
          <Input id={`${id}-name`} value={name} maxLength={100} autoFocus onChange={(e) => setName(e.target.value)} />
        </div>
        {error && (
          <p role="alert" className="text-sm text-destructive">
            {error}
          </p>
        )}
        <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
          <Button type="button" variant="ghost" onClick={() => onDone()}>
            {t('series.cancel')}
          </Button>
          <Button type="submit" disabled={save.isPending}>
            {t('series.save')}
          </Button>
        </div>
      </form>
    </DialogContent>
  )
}

/** « Collections » d'une série (administrateur) : une case par collection, cochée = la série y
 * est ; chaque case s'enregistre tout de suite. Une collection se crée ici aussi, avec la série. */
export function SeriesCollectionsDialog({ series }: { series: SeriesDetail }) {
  const { t } = useTranslation()
  const id = useId()
  const collections = useCollections()
  const refresh = useRefresh()
  const [name, setName] = useState('')
  const [error, setError] = useState<string | null>(null)
  const toggle = useMutation({
    mutationFn: ({ collection, on }: { collection: number; on: boolean }) => api.collectionSeries(collection, series.id, on),
    onSuccess: refresh,
    onError: toastFailure(t('collections.failed'), t),
  })
  const create = useMutation({
    mutationFn: async (next: string) => api.collectionSeries((await api.createCollection(next)).id, series.id, true),
    onSuccess: () => {
      refresh()
      setName('')
    },
    onError: (e) => setError(nameError(e, t)),
  })
  return (
    <DialogContent closeLabel={t('common.close')}>
      <DialogHeader>
        <DialogTitle>{t('collections.seriesTitle', { name: series.name })}</DialogTitle>
        <DialogDescription>{t('collections.seriesHelp')}</DialogDescription>
      </DialogHeader>
      <QueryState query={collections} isEmpty={() => false} empty={null}>
        {(list) => (
          <ul className="max-h-72 space-y-1 overflow-y-auto">
            {list.map((c) => (
              <li key={c.id} className="flex min-h-11 items-center gap-3">
                <Checkbox
                  id={`${id}-${c.id}`}
                  checked={c.series_ids.includes(series.id)}
                  disabled={toggle.isPending}
                  onCheckedChange={(v) => toggle.mutate({ collection: c.id, on: v === true })}
                />
                <label htmlFor={`${id}-${c.id}`} className="min-w-0 flex-1 truncate text-sm" title={c.name}>
                  {c.name}
                </label>
              </li>
            ))}
          </ul>
        )}
      </QueryState>
      <form
        noValidate
        className="space-y-1.5"
        onSubmit={(event) => {
          event.preventDefault()
          setError(null)
          create.mutate(name)
        }}
      >
        <label htmlFor={`${id}-new`} className="text-sm font-medium">
          {t('collections.newName')}
        </label>
        <div className="flex gap-2">
          <Input id={`${id}-new`} value={name} maxLength={100} onChange={(e) => setName(e.target.value)} />
          <Button type="submit" variant="secondary" disabled={create.isPending}>
            <Plus aria-hidden />
            {t('collections.create')}
          </Button>
        </div>
        {error && (
          <p role="alert" className="text-sm text-destructive">
            {error}
          </p>
        )}
      </form>
    </DialogContent>
  )
}
