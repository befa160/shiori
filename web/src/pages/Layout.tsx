import { House, Settings } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Link, NavLink, Outlet } from 'react-router'
import { useLibraries } from '@/api/queries'
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
import { LanguageSelect } from './LanguageSelect'

// Destinations principales : sidebar à partir de `lg`, barre d'onglets en dessous (4 au plus).
const NAV = [
  { to: '/', label: 'nav.home', icon: House },
  { to: '/admin', label: 'nav.admin', icon: Settings },
] as const

/** Cadre commun hors lecteur (DESIGN_SYSTEM.md, « Cadre et breakpoints »). */
export function Layout() {
  const { t } = useTranslation()
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
          {/* Variante claire du logo, pour le fond sombre ; décorative, le lien a déjà son nom. */}
          <img src="/shiori-mark-light.svg" alt="" width={28} height={28} />
          Shiori
        </Link>
        <SidebarTrigger label={t(state === 'expanded' ? 'layout.collapse' : 'layout.expand')} />
      </SidebarHeader>
      <nav aria-label={t('layout.navigation')} className="contents">
        <SidebarContent>
          <SidebarGroup>
            <SidebarMenu>
              <NavItem {...NAV[0]} />
            </SidebarMenu>
          </SidebarGroup>
          {/* Liste des bibliothèques : masquée une fois réduite (l'accueil les liste aussi) ; en
              chargement ou en erreur, rien ici, l'écran principal l'affiche déjà. */}
          {libraries.data && libraries.data.length > 0 && (
            <SidebarGroup className="group-data-[collapsible=icon]:hidden">
              <SidebarGroupLabel>{t('nav.libraries')}</SidebarGroupLabel>
              <SidebarMenuSub>
                {libraries.data.map((library) => (
                  <li key={library.id}>
                    <SidebarMenuSubButton>
                      <NavLink to={`/libraries/${library.id}`} title={library.name}>
                        <span>{library.name}</span>
                      </NavLink>
                    </SidebarMenuSubButton>
                  </li>
                ))}
              </SidebarMenuSub>
            </SidebarGroup>
          )}
          <SidebarGroup className="mt-auto">
            <SidebarMenu>
              <NavItem {...NAV[1]} />
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
