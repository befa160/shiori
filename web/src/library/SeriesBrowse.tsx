import { Search } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useSearchParams } from 'react-router'
import { useAllSeries, useLibraries } from '@/api/queries'
import { QueryState } from '@/browse/parts'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { cn } from '@/lib/utils'
import { CoverGrid, CoverSkeletons, SeriesCard } from './covers'
import { applyFilters, isFiltered, READ_FILTERS, readFilters, SORTS, STATUS_FILTERS } from './filters'

const ALL = 'all'

/** Toutes les séries, avec recherche, filtres et tri dans l'adresse. `/search` : le champ de
 * recherche prend le focus à l'ouverture (onglet « Recherche »). */
export function SeriesBrowse({ search = false }: { search?: boolean }) {
  const { t } = useTranslation()
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
          options={[[ALL, t('browse.statusAll')], ...STATUS_FILTERS.map((v) => [v, t(`browse.status.${v}`)] as const)]}
          onChange={(v) => update('status', v === ALL ? null : v)}
        />
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
