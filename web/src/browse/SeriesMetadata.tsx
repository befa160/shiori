import { ExternalLink, Lock } from 'lucide-react'
import { type ReactNode, useId, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import type { Author, PublicationStatus, SeriesDetail, SeriesField, SeriesMetadata, SeriesPatch } from '@/api/types'
import { usePatchSeries, webLinks } from './series'
import { Button } from '@/components/ui/button'
import { DialogContent, DialogDescription, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Textarea } from '@/components/ui/textarea'
import { errorText } from '@/i18n/errors'
import { STATUS_FILTERS } from '@/library/filters'

// shortcut: replié d'après la longueur du texte, pas sa hauteur affichée ; mesurer le débordement si un résumé court sur un écran étroit se retrouve coupé sans bouton.
const LONG_SUMMARY = 240

/** Ce que l'on sait de la série : auteurs, repères, résumé repliable, genres et tags, autres titres, liens. */
export function SeriesAbout({ series }: { series: SeriesDetail }) {
  const { t } = useTranslation()
  const m = series.metadata
  const [open, setOpen] = useState(false)
  const facts = [
    m.year?.toString(),
    m.publisher,
    m.publication_status && t(`browse.status.${m.publication_status}`),
    m.age_rating,
  ].filter(Boolean)
  const chips = [...new Set([...m.genres, ...m.tags])]
  const links = webLinks(m.links)
  const summaryId = useId()
  const long = (m.summary?.length ?? 0) > LONG_SUMMARY
  return (
    <div className="max-w-prose space-y-3">
      {m.authors.length > 0 && <p className="text-muted-foreground">{m.authors.map((a) => a.name).join(', ')}</p>}
      {facts.length > 0 && <p className="text-sm text-muted-foreground">{facts.join(' · ')}</p>}
      {m.summary && (
        <div>
          <p id={summaryId} className={long && !open ? 'line-clamp-4 whitespace-pre-line' : 'whitespace-pre-line'}>
            {m.summary}
          </p>
          {long && (
            <Button variant="link" className="h-11 px-0" aria-expanded={open} aria-controls={summaryId} onClick={() => setOpen(!open)}>
              {t(open ? 'series.less' : 'series.more')}
            </Button>
          )}
        </div>
      )}
      {chips.length > 0 && (
        <ul aria-label={t('series.genresAndTags')} className="flex flex-wrap gap-2">
          {chips.map((chip) => (
            <li key={chip} className="max-w-full truncate rounded-full bg-card px-3 py-1 text-sm">
              {chip}
            </li>
          ))}
        </ul>
      )}
      {m.alt_titles.length > 0 && (
        <p className="text-sm break-words text-muted-foreground">{t('series.alsoKnown', { titles: m.alt_titles.join(' · ') })}</p>
      )}
      {links.length > 0 && (
        <ul className="flex flex-wrap gap-x-4">
          {links.map((url) => (
            <li key={url.href}>
              <a
                href={url.href}
                target="_blank"
                rel="noreferrer noopener"
                className="inline-flex min-h-11 items-center gap-1 text-sm underline-offset-4 hover:underline"
              >
                {url.hostname}
                <ExternalLink aria-hidden className="size-4" />
              </a>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}

/** Champs du formulaire, chacun en texte ; une liste par ligne, sauf genres et tags (virgules). */
type Form = Record<Exclude<SeriesField, 'cover_page_id' | 'cover_image_id'>, string>
const UNKNOWN = 'unknown'
const lines = (text: string) => text.split('\n').map((s) => s.trim()).filter(Boolean)
const commas = (text: string) => text.split(',').map((s) => s.trim()).filter(Boolean)

function formOf(m: SeriesMetadata): Form {
  return {
    title: m.title ?? '',
    alt_titles: m.alt_titles.join('\n'),
    summary: m.summary ?? '',
    authors: m.authors.map((a) => a.name).join('\n'),
    genres: m.genres.join(', '),
    tags: m.tags.join(', '),
    year: m.year?.toString() ?? '',
    publisher: m.publisher ?? '',
    age_rating: m.age_rating ?? '',
    publication_status: m.publication_status ?? UNKNOWN,
    links: m.links.join('\n'),
  }
}

/** Valeur envoyée au serveur pour un champ ; `undefined` : année invalide. */
function valueOf(field: keyof Form, text: string, authors: Author[]): SeriesPatch[keyof Form] | undefined {
  switch (field) {
    case 'alt_titles':
    case 'links':
      return lines(text)
    case 'genres':
    case 'tags':
      return commas(text)
    case 'authors':
      // Un nom déjà connu garde ses rôles (scénario, dessin) ; un nouveau nom n'en a pas.
      return lines(text).map((name) => authors.find((a) => a.name === name) ?? { name, story: false, art: false })
    case 'year': {
      if (text.trim() === '') return null
      const year = Number(text)
      return Number.isInteger(year) && year >= 1 && year <= 9999 ? year : undefined
    }
    case 'publication_status':
      return text === UNKNOWN ? null : (text as PublicationStatus)
    default:
      return text.trim() || null
  }
}

/** Fenêtre « Modifier » : un champ changé est envoyé (donc verrouillé), « Rétablir » le rend aux fichiers. */
export function EditSeriesDialog({ series, onDone }: { series: SeriesDetail; onDone: () => void }) {
  const { t } = useTranslation()
  const id = useId()
  const save = usePatchSeries(series.id)
  const unlock = usePatchSeries(series.id)
  // Valeurs de départ, mises à jour quand un champ est rétabli : seuls les champs changés partent.
  const [initial, setInitial] = useState(() => formOf(series.metadata))
  const [form, setForm] = useState(initial)
  const [error, setError] = useState<string | null>(null)
  const yearError = valueOf('year', form.year, []) === undefined
  const set = (field: keyof Form) => (value: string) => setForm((f) => ({ ...f, [field]: value }))

  const restore = (field: keyof Form) =>
    unlock.mutate(
      { unlock: [field] },
      {
        onSuccess: (detail) => {
          const value = formOf(detail.metadata)[field]
          setInitial((f) => ({ ...f, [field]: value }))
          setForm((f) => ({ ...f, [field]: value }))
        },
        onError: (e) => setError(errorText(e, t)),
      },
    )

  const submit = (event: React.FormEvent) => {
    event.preventDefault()
    if (yearError) return
    const patch: Record<string, unknown> = {}
    for (const field of Object.keys(form) as (keyof Form)[]) {
      if (form[field] !== initial[field]) patch[field] = valueOf(field, form[field], series.metadata.authors)
    }
    if (Object.keys(patch).length === 0) return onDone()
    setError(null)
    save.mutate(patch as SeriesPatch, {
      onSuccess: () => {
        toast.success(t('series.saved'))
        onDone()
      },
      onError: (e) => setError(errorText(e, t)),
    })
  }

  type ControlProps = { id: string; 'aria-describedby'?: string; 'aria-invalid'?: boolean }
  const field = (name: keyof Form, control: (props: ControlProps) => ReactNode, help?: string, problem?: string) => {
    const locked = series.locked.includes(name)
    const label = t(`series.fields.${name}`)
    const helpId = help ? `${id}-${name}-help` : undefined
    const errorId = problem ? `${id}-${name}-error` : undefined
    const describedBy = [helpId, errorId].filter(Boolean).join(' ') || undefined
    return (
      <div className="space-y-1.5">
        <div className="flex min-h-11 flex-wrap items-center justify-between gap-x-3">
          <label htmlFor={`${id}-${name}`} className="text-sm font-medium">
            {label}
          </label>
          {locked && (
            <span className="flex items-center gap-1 text-sm text-muted-foreground">
              <Lock aria-hidden className="size-4 text-primary" />
              {t('series.locked')}
              <Button
                type="button"
                variant="ghost"
                size="sm"
                className="min-h-11"
                aria-label={t('series.unlockLabel', { field: label })}
                disabled={unlock.isPending}
                onClick={() => restore(name)}
              >
                {t('series.unlock')}
              </Button>
            </span>
          )}
        </div>
        {control({ id: `${id}-${name}`, 'aria-describedby': describedBy, 'aria-invalid': problem ? true : undefined })}
        {help && (
          <p id={helpId} className="text-sm text-muted-foreground">
            {help}
          </p>
        )}
        {problem && (
          <p id={errorId} role="alert" className="text-sm text-destructive">
            {problem}
          </p>
        )}
      </div>
    )
  }
  const text = (name: keyof Form) => (props: ControlProps) => (
    <Input {...props} value={form[name]} onChange={(e) => set(name)(e.target.value)} />
  )
  const area = (name: keyof Form, rows = 3) => (props: ControlProps) => (
    <Textarea {...props} rows={rows} value={form[name]} onChange={(e) => set(name)(e.target.value)} />
  )

  return (
    <DialogContent closeLabel={t('common.close')} className="sm:max-w-2xl">
      <DialogHeader>
        <DialogTitle>{t('series.editTitle', { name: series.name })}</DialogTitle>
        <DialogDescription>{t('series.editDescription')}</DialogDescription>
      </DialogHeader>
      {/* `min-w-0` : un long texte (field-sizing) n'élargit pas la fenêtre au-delà de l'écran. */}
      <form onSubmit={submit} className="min-w-0 space-y-4" noValidate>
        {field('title', text('title'), t('series.titleHelp'))}
        {field('alt_titles', area('alt_titles'), t('series.onePerLine'))}
        {field('summary', area('summary', 6))}
        {field('authors', area('authors', 2), t('series.onePerLine'))}
        <div className="grid gap-4 sm:grid-cols-2">
          {field('genres', text('genres'), t('series.commaSeparated'))}
          {field('tags', text('tags'), t('series.commaSeparated'))}
          {field(
            'year',
            (props) => <Input {...props} inputMode="numeric" value={form.year} onChange={(e) => set('year')(e.target.value)} />,
            undefined,
            yearError ? t('series.yearInvalid') : undefined,
          )}
          {field('publisher', text('publisher'))}
          {field('age_rating', text('age_rating'))}
          {field('publication_status', (props) => (
            <Select value={form.publication_status} onValueChange={set('publication_status')}>
              <SelectTrigger id={props.id} className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent position="popper" align="start">
                {STATUS_FILTERS.map((v) => (
                  <SelectItem key={v} value={v}>
                    {t(`browse.status.${v}`)}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          ))}
        </div>
        {field('links', area('links', 2), t('series.linksHelp'))}
        {error && (
          <p role="alert" className="text-sm text-destructive">
            {t('series.notSaved')} · {error}
          </p>
        )}
        <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
          <Button type="button" variant="ghost" onClick={onDone}>
            {t('series.cancel')}
          </Button>
          <Button type="submit" disabled={save.isPending || yearError}>
            {t('series.save')}
          </Button>
        </div>
      </form>
    </DialogContent>
  )
}
