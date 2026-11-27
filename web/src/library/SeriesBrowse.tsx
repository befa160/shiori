import { ChevronDown, Search, X } from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useSearchParams } from 'react-router'
import { useAllSeries, useLibraries } from '@/api/queries'
import { QueryState } from '@/browse/Parts'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Input } from '@/components/ui/input'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { cn } from '@/lib/utils'
import { CoverGrid, CoverSkeletons, SeriesCard } from './Covers'
import { tagLabel, tagSpellings } from '@/i18n/tags'
import { allTags, applyFilters, fold, isFiltered, READ_FILTERS, readFilters, SORTS, BROWSE_STATUS, tagKey } from './filters'

const ALL = 'all'

/** Toutes les séries, avec recherche, filtres et tri dans l'adresse. `/search` : le champ de
 * recherche prend le focus à l'ouverture (onglet « Recherche »). */
export function SeriesBrowse({ search = false }: { search?: boolean }) {
  const { t, i18n } = useTranslation()
  const lang = i18n.language
  const [params, setParams] = useSearchParams()
  const filters = readFilters(params)
  const series = useAllSeries()
  const libraries = useLibraries().data ?? []
  const input = useRef<HTMLInputElement>(null)
  // Le texte tapé vit ici : React Router change l'adresse dans une transition, et un champ piloté
  // par une transition prend du retard ou perd son curseur sur un appareil lent.
  const [q, setQ] = useState(filters.q)
  useEffect(() => {
    if (search) input.current?.focus()
  }, [search])

  // `replace` : taper ou filtrer ne remplit pas l'historique ; « Retour » depuis une série
  // retrouve quand même les filtres, qui sont dans l'adresse.
  const update = (key: string, value: string | null) =>
    setParams(
      (current) => {
        const next = new URLSearchParams(current)
        if (value === null || value === '') next.delete(key)
        else next.set(key, value)
        return next
      },
      { replace: true },
    )
  const setTags = (tags: string[]) =>
    setParams(
      (current) => {
        const next = new URLSearchParams(current)
        next.delete('tag')
        for (const tag of tags) next.append('tag', tag)
        return next
      },
      { replace: true },
    )
  // Pas à chaque frappe : des milliers de séries à replier (accents, casse).
  const tags = useMemo(
    () => allTags(series.data ?? []).sort((a, b) => tagLabel(a.tag, lang).localeCompare(tagLabel(b.tag, lang), lang)),
    [series.data, lang],
  )
  const clear = () => {
    setQ('')
    setParams(filters.sort === 'name' ? {} : { sort: filters.sort }, { replace: true })
  }

  const title = t(search ? 'browse.searchTitle' : 'browse.title')
  return (
    <section className="space-y-4">
      <title>{`${title} · Shiori`}</title>
      <h1 className="text-2xl font-semibold">{title}</h1>
      <div className="max-w-xl space-y-1">
        <label htmlFor="series-search" className="text-sm text-muted-foreground">
          {t('browse.searchLabel')}
        </label>
        <div className="relative">
          <Search aria-hidden className="pointer-events-none absolute top-1/2 left-3 size-5 -translate-y-1/2 text-muted-foreground" />
          <Input
            ref={input}
            id="series-search"
            type="search"
            enterKeyHint="search"
            autoComplete="off"
            placeholder={t('browse.searchPlaceholder')}
            value={q}
            onChange={(e) => {
              setQ(e.target.value)
              update('q', e.target.value)
            }}
            className="pl-10"
          />
        </div>
      </div>
      {/* Deux colonnes sur mobile (deux lignes pour quatre listes), à la suite au-delà. */}
      <div role="group" aria-label={t('browse.filters')} className="grid grid-cols-2 gap-2 sm:flex sm:flex-wrap sm:items-center">
        <FilterSelect
          label={t('browse.readLabel')}
          value={filters.read ?? ALL}
          options={[[ALL, t('browse.readAll')], ...READ_FILTERS.map((v) => [v, t(`browse.read.${v}`)] as const)]}
          onChange={(v) => update('read', v === ALL ? null : v)}
        />
        <FilterSelect
          label={t('browse.statusLabel')}
          value={filters.status ?? ALL}
          options={[[ALL, t('browse.statusAll')], ...BROWSE_STATUS.map((v) => [v, t(`browse.status.${v}`)] as const)]}
          onChange={(v) => update('status', v === ALL ? null : v)}
        />
        {(tags.length > 0 || filters.tags.length > 0) && (
          <TagFilter tags={tags} selected={filters.tags} onChange={setTags} />
        )}
        {/* Une seule bibliothèque : rien à choisir. */}
        {libraries.length > 1 && (
          <FilterSelect
            label={t('browse.libraryLabel')}
            value={filters.library === null ? ALL : String(filters.library)}
            options={[[ALL, t('browse.libraryAll')], ...libraries.map((l) => [String(l.id), l.name] as const)]}
            onChange={(v) => update('library', v === ALL ? null : v)}
          />
        )}
        <FilterSelect
          label={t('browse.sortLabel')}
          value={filters.sort}
          options={SORTS.map((v) => [v, t(`browse.sort.${v}`)] as const)}
          onChange={(v) => update('sort', v === 'name' ? null : v)}
          neutral="name"
        />
        {isFiltered(filters) && (
          <Button variant="ghost" onClick={clear} className="col-span-2 justify-self-start">
            {t('browse.clear')}
          </Button>
        )}
      </div>
      {filters.tags.length > 0 && (
        <ul aria-label={t('browse.tagsChosen')} className="flex flex-wrap gap-2">
          {filters.tags.map((tag) => {
            // La graphie des séries plutôt que celle de l'adresse (`?tag=seinen` → « Seinen »), traduite.
            const name = tagLabel(tags.find((known) => tagKey(known.tag) === tagKey(tag))?.tag ?? tag, lang)
            return (
              <li key={tag} className="max-w-full">
                <Button
                  variant="secondary"
                  aria-label={t('browse.removeTag', { tag: name })}
                  onClick={() => setTags(filters.tags.filter((other) => other !== tag))}
                  className="max-w-full rounded-full border border-primary text-primary"
                >
                  <span className="truncate">{name}</span>
                  <X aria-hidden />
                </Button>
              </li>
            )
          })}
        </ul>
      )}
      <QueryState
        query={series}
        isEmpty={(list) => list.length === 0}
        empty={<p>{t('libraries.empty')}</p>}
        loading={<CoverSkeletons grid />}
      >
        {(all) => {
          const list = applyFilters(all, filters)
          return (
            <div className="space-y-3">
              {/* Annonce le nombre de résultats à la frappe, sans déplacer le focus. */}
              <p aria-live="polite" className="text-sm text-muted-foreground">
                {t('counts.series', { count: list.length })}
              </p>
              {list.length === 0 ? (
                <div className="space-y-3 rounded-md bg-card p-4">
                  <p>{t('browse.noResults')}</p>
                  <p className="text-sm text-muted-foreground">{t('browse.noResultsHelp')}</p>
                  <Button variant="secondary" onClick={clear}>
                    {t('browse.clear')}
                  </Button>
                </div>
              ) : (
                <CoverGrid>
                  {list.map((s, i) => (
                    <li key={s.id}>
                      <SeriesCard series={s} eager={i < 12} />
                    </li>
                  ))}
                </CoverGrid>
              )}
            </div>
          )
        }}
      </QueryState>
    </section>
  )
}

/** Tags à cocher, avec une recherche : AniList en donne des dizaines par série. */
function TagFilter({
  tags,
  selected,
  onChange,
}: {
  tags: { tag: string; count: number }[]
  selected: string[]
  onChange: (tags: string[]) => void
}) {
  const { t, i18n } = useTranslation()
  const [find, setFind] = useState('')
  const chosen = new Set(selected.map(tagKey))
  // Le tag se cherche dans les deux langues : « comédie » trouve « Comedy ».
  const shown = tags.filter(({ tag }) => tagSpellings(tag).some((s) => fold(s).includes(fold(find.trim()))))
  const toggle = (tag: string) =>
    onChange(chosen.has(tagKey(tag)) ? selected.filter((s) => tagKey(s) !== tagKey(tag)) : [...selected, tag])
  return (
    <Popover onOpenChange={(open) => open || setFind('')}>
      <PopoverTrigger asChild>
        <Button
          variant="outline"
          className={cn(
            'w-full min-w-0 justify-between font-normal sm:w-fit',
            selected.length > 0 && 'border-primary text-primary',
          )}
        >
          <span className="truncate">
            {selected.length > 0 ? t('browse.tagsSelected', { count: selected.length }) : t('browse.tagsLabel')}
          </span>
          <ChevronDown aria-hidden className="opacity-50" />
        </Button>
      </PopoverTrigger>
      <PopoverContent
        align="start"
        side="bottom"
        className="w-72 max-w-[calc(100vw-2rem)] p-2"
        // Au doigt, le clavier virtuel cacherait la liste : le champ ne prend le focus que sur demande.
        onOpenAutoFocus={(e) => matchMedia('(pointer: coarse)').matches && e.preventDefault()}
      >
        <Input
          type="search"
          autoComplete="off"
          aria-label={t('browse.tagsFind')}
          placeholder={t('browse.tagsFind')}
          value={find}
          onChange={(e) => setFind(e.target.value)}
        />
        {shown.length === 0 ? (
          <p className="px-2 py-3 text-sm text-muted-foreground">{t('browse.tagsNone')}</p>
        ) : (
          <ul aria-label={t('browse.tagsLabel')} className="mt-2 max-h-72 overflow-y-auto">
            {shown.map(({ tag, count }) => (
              <li key={tag}>
                <label className="flex min-h-11 cursor-pointer items-center gap-3 rounded-md px-2 hover:bg-accent">
                  <Checkbox checked={chosen.has(tagKey(tag))} onCheckedChange={() => toggle(tag)} />
                  <span className="min-w-0 flex-1 truncate">{tagLabel(tag, i18n.language)}</span>
                  <span className="text-sm text-muted-foreground">{t('browse.tagCount', { count })}</span>
                </label>
              </li>
            ))}
          </ul>
        )}
      </PopoverContent>
    </Popover>
  )
}

/** Liste déroulante de filtre ; ambre quand elle restreint la liste (état actif). */
function FilterSelect({
  label,
  value,
  options,
  onChange,
  neutral = ALL,
}: {
  label: string
  value: string
  options: (readonly [string, string])[]
  onChange: (value: string) => void
  neutral?: string
}) {
  return (
    <Select value={value} onValueChange={onChange}>
      <SelectTrigger aria-label={label} className={cn(
          // Libellé tronqué par des points de suspension quand la colonne est étroite.
          'w-full min-w-0 sm:w-fit *:data-[slot=select-value]:block *:data-[slot=select-value]:truncate',
          value !== neutral && 'border-primary text-primary',
        )}>
        <SelectValue />
      </SelectTrigger>
      <SelectContent position="popper" align="start">
        {options.map(([v, text]) => (
          <SelectItem key={v} value={v}>
            {text}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  )
}
