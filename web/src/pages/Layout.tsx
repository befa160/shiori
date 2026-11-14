import { BookCopy, House, Loader2, RefreshCw, Search, Settings } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Link, NavLink, Outlet } from 'react-router'
import { useLibraries } from '@/api/queries'
import type { Library } from '@/api/types'
import { useStartScan } from '@/admin/useStartScan'
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarInset,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarMenuSub,
  SidebarMenuSubButton,
  SidebarProvider,
  SidebarTrigger,
  useSidebar,
} from '@/components/ui/sidebar'
import { ScanWatcher } from '@/admin/scans'
import { useTheme } from '@/lib/theme'
import { LanguageSelect } from './LanguageSelect'

// Destinations principales : sidebar à partir de `lg`, barre d'onglets en dessous (4 au plus).
const NAV = [
  { to: '/', label: 'nav.home', icon: House },
  { to: '/series', label: 'nav.series', icon: BookCopy },
  { to: '/search', label: 'nav.search', icon: Search },
  { to: '/admin', label: 'nav.admin', icon: Settings },
] as const

/** Cadre commun hors lecteur (DESIGN_SYSTEM.md, « Cadre et breakpoints »). */
export function Layout() {
  const { t } = useTranslation()
  useTheme()
  return (
    <SidebarProvider className="bg-background text-foreground">
      <a
        href="#main"
        className="sr-only z-50 rounded-md bg-primary font-medium text-primary-foreground focus:not-sr-only focus:fixed focus:px-4 focus:py-3 focus:top-[max(0.5rem,env(safe-area-inset-top))] focus:left-[max(0.5rem,env(safe-area-inset-left))]"
      >
        {t('layout.skip')}
      </a>
      <AppSidebar />
      <SidebarInset>
        {/* Marges : la plus grande de la marge voulue et de la zone sûre ; en bas, la barre d'onglets en plus. */}
        <main
          id="main"
          tabIndex={-1}
          className="mx-auto w-full max-w-screen-2xl pt-[max(1.5rem,env(safe-area-inset-top))] pr-[max(1rem,env(safe-area-inset-right))] pb-[calc(var(--tabbar-height)+env(safe-area-inset-bottom)+1.5rem)] pl-[max(1rem,env(safe-area-inset-left))] outline-none lg:px-8"
        >
          <Outlet />
        </main>
      </SidebarInset>
      <TabBar />
      <ScanWatcher />
    </SidebarProvider>
  )
}

function AppSidebar() {
  const { t } = useTranslation()
  const { state } = useSidebar()
  const libraries = useLibraries()
  return (
    <Sidebar>
      <SidebarHeader className="flex-row items-center justify-between group-data-[collapsible=icon]:justify-center">
        <Link
          to="/"
          aria-label={t('layout.home')}
          className="flex items-center gap-2 rounded-md px-2 py-1 text-lg font-semibold group-data-[collapsible=icon]:hidden"
        >
          {/* Logo crème et ambre, pour le fond sombre ; décoratif, le lien a déjà son nom. */}
          {/* Logo crème sur les thèmes sombres, noir sur les thèmes clairs (classe `dark` de <html>). */}
          <img src="/shiori-mark.svg" alt="" width={28} height={28} className="hidden dark:block" />
          <img src="/shiori-mark-light.svg" alt="" width={28} height={28} className="dark:hidden" />
          Shiori
        </Link>
        <SidebarTrigger label={t(state === 'expanded' ? 'layout.collapse' : 'layout.expand')} />
      </SidebarHeader>
      <nav aria-label={t('layout.navigation')} className="contents">
        <SidebarContent>
          <SidebarGroup>
            <SidebarMenu>
              {NAV.slice(0, 3).map((item) => (
                <NavItem key={item.to} {...item} />
              ))}
            </SidebarMenu>
          </SidebarGroup>
          {/* Liste des bibliothèques : masquée une fois réduite (l'accueil les liste aussi) ; en
              chargement ou en erreur, rien ici, l'écran principal l'affiche déjà. */}
          {libraries.data && libraries.data.length > 0 && (
            <SidebarGroup className="group-data-[collapsible=icon]:hidden">
              <SidebarGroupLabel>{t('nav.libraries')}</SidebarGroupLabel>
              <SidebarMenuSub>
                {libraries.data.map((library) => (
                  <li key={library.id} className="flex items-center gap-1">
                    <SidebarMenuSubButton className="min-w-0 flex-1">
                      <NavLink to={`/libraries/${library.id}`} title={library.name}>
                        <span>{library.name}</span>
                      </NavLink>
                    </SidebarMenuSubButton>
                    <ScanButton library={library} />
                  </li>
                ))}
              </SidebarMenuSub>
            </SidebarGroup>
          )}
          <SidebarGroup className="mt-auto">
            <SidebarMenu>
              <NavItem {...NAV[3]} />
            </SidebarMenu>
          </SidebarGroup>
        </SidebarContent>
      </nav>
      <SidebarFooter className="group-data-[collapsible=icon]:hidden">
        <LanguageSelect />
      </SidebarFooter>
    </Sidebar>
  )
}

/** Rescanner une bibliothèque sans passer par l'administration ; la fin est annoncée par le suivi. */
function ScanButton({ library }: { library: Library }) {
  const { t } = useTranslation()
  const scan = useStartScan()
  return (
    <button
      type="button"
      aria-label={t('admin.scanNamed', { name: library.name })}
      title={t('admin.scanNamed', { name: library.name })}
      disabled={scan.isPending}
      onClick={() => scan.mutate({ library })}
      className="flex size-11 shrink-0 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-sidebar-accent hover:text-foreground disabled:opacity-50"
    >
      {scan.isPending ? <Loader2 aria-hidden className="size-4 animate-spin" /> : <RefreshCw aria-hidden className="size-4" />}
    </button>
  )
}

function NavItem({ to, label, icon: Icon }: (typeof NAV)[number]) {
  const { t } = useTranslation()
  return (
    <SidebarMenuItem>
      <SidebarMenuButton tooltip={t(label)}>
        <NavLink to={to} end={to === '/'}>
          <Icon aria-hidden />
          <span>{t(label)}</span>
        </NavLink>
      </SidebarMenuButton>
    </SidebarMenuItem>
  )
}

/** Barre d'onglets fixe en bas, sous `lg` : icône et libellé (jamais l'icône seule). */
function TabBar() {
  const { t } = useTranslation()
  return (
    <nav
      aria-label={t('layout.navigation')}
      className="fixed inset-x-0 bottom-0 z-20 border-t border-border bg-card pr-safe pb-safe pl-safe lg:hidden"
    >
      <ul className="flex h-(--tabbar-height)">
        {NAV.map(({ to, label, icon: Icon }) => (
          <li key={to} className="flex-1">
            <NavLink
              to={to}
              end={to === '/'}
              className="flex h-full flex-col items-center justify-center gap-1 text-xs text-muted-foreground transition-colors active:bg-accent aria-[current=page]:text-primary"
            >
              <Icon aria-hidden className="size-5" />
              {t(label)}
            </NavLink>
          </li>
        ))}
      </ul>
    </nav>
  )
}
