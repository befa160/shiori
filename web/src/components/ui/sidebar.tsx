import * as React from "react"
import { PanelLeftIcon } from "lucide-react"
import { Slot } from "radix-ui"
import { cn } from "@/lib/utils"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip"

// Sidebar shadcn (MCP), réduite à ce que Shiori utilise (DESIGN_SYSTEM.md, « Navigation ») :
// affichée à partir de `lg` seulement (la barre d'onglets la remplace en dessous, d'où le
// retrait du panneau mobile), état réduit gardé dans localStorage (pas de rendu serveur, le
// cookie ne servait à rien), pas de Ctrl+B (déjà pris par les marque-pages de Firefox),
// entrées de 44 px et focus ambre global (sans l'anneau de shadcn).

const STORAGE_KEY = "shiori.sidebar"
const SIDEBAR_WIDTH = "16rem"
// Entrée de 44 px + 8 px de marge du groupe de chaque côté.
const SIDEBAR_WIDTH_ICON = "3.75rem"

type SidebarContextProps = {
  state: "expanded" | "collapsed"
  toggleSidebar: () => void
}

const SidebarContext = React.createContext<SidebarContextProps | null>(null)

function useSidebar() {
  const context = React.useContext(SidebarContext)
  if (!context) {
    throw new Error("useSidebar must be used within a SidebarProvider.")
  }
  return context
}

// localStorage peut être absent ou refusé (navigation privée) : la sidebar reste ouverte.
function storedOpen(): boolean {
  try {
    return localStorage.getItem(STORAGE_KEY) !== "collapsed"
  } catch {
    return true
  }
}

function SidebarProvider({ className, style, children, ...props }: React.ComponentProps<"div">) {
  const [open, setOpen] = React.useState(storedOpen)

  const toggleSidebar = React.useCallback(() => {
    setOpen((was) => {
      try {
        localStorage.setItem(STORAGE_KEY, was ? "collapsed" : "expanded")
      } catch {
        // Préférence non retenue, sans conséquence.
      }
      return !was
    })
  }, [])

  const contextValue = React.useMemo<SidebarContextProps>(
    () => ({ state: open ? "expanded" : "collapsed", toggleSidebar }),
    [open, toggleSidebar]
  )

  return (
    <SidebarContext.Provider value={contextValue}>
      <TooltipProvider delayDuration={0}>
        <div
          data-slot="sidebar-wrapper"
          style={
            {
              "--sidebar-width": SIDEBAR_WIDTH,
              "--sidebar-width-icon": SIDEBAR_WIDTH_ICON,
              ...style,
            } as React.CSSProperties
          }
          className={cn("group/sidebar-wrapper flex min-h-dvh w-full", className)}
          {...props}
        >
          {children}
        </div>
      </TooltipProvider>
    </SidebarContext.Provider>
  )
}

function Sidebar({ className, children, ...props }: React.ComponentProps<"div">) {
  const { state } = useSidebar()

  return (
    <div
      className="group peer hidden text-sidebar-foreground lg:block"
      data-state={state}
      data-collapsible={state === "collapsed" ? "icon" : ""}
      data-slot="sidebar"
    >
      {/* Réserve la largeur de la sidebar fixe dans le flux. */}
      <div
        data-slot="sidebar-gap"
        className="relative w-(--sidebar-width) bg-transparent transition-[width] duration-200 ease-out group-data-[collapsible=icon]:w-(--sidebar-width-icon)"
      />
      <div
        data-slot="sidebar-container"
        className={cn(
          "fixed inset-y-0 left-0 z-10 hidden h-dvh w-(--sidebar-width) border-r border-sidebar-border pt-safe pb-safe pl-safe transition-[width] duration-200 ease-out group-data-[collapsible=icon]:w-(--sidebar-width-icon) lg:flex",
          className
        )}
        {...props}
      >
        <div data-sidebar="sidebar" data-slot="sidebar-inner" className="flex h-full w-full flex-col bg-sidebar">
          {children}
        </div>
      </div>
    </div>
  )
}

function SidebarTrigger({
  className,
  onClick,
  label,
  ...props
}: React.ComponentProps<typeof Button> & { label: string }) {
  const { state, toggleSidebar } = useSidebar()

  return (
    <Button
      data-sidebar="trigger"
      data-slot="sidebar-trigger"
      variant="ghost"
      size="icon"
      aria-label={label}
      aria-expanded={state === "expanded"}
      className={cn("text-muted-foreground", className)}
      onClick={(event) => {
        onClick?.(event)
        toggleSidebar()
      }}
      {...props}
    >
      <PanelLeftIcon aria-hidden />
    </Button>
  )
}

function SidebarInset({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="sidebar-inset"
      className={cn("relative flex w-full min-w-0 flex-1 flex-col bg-background", className)}
      {...props}
    />
  )
}

function SidebarHeader({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="sidebar-header"
      data-sidebar="header"
      className={cn("flex flex-col gap-2 p-2", className)}
      {...props}
    />
  )
}

function SidebarFooter({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="sidebar-footer"
      data-sidebar="footer"
      className={cn("flex flex-col gap-2 p-2", className)}
      {...props}
    />
  )
}

function SidebarContent({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="sidebar-content"
      data-sidebar="content"
      className={cn(
        "flex min-h-0 flex-1 flex-col gap-2 overflow-auto group-data-[collapsible=icon]:overflow-hidden",
        className
      )}
      {...props}
    />
  )
}

function SidebarGroup({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="sidebar-group"
      data-sidebar="group"
      className={cn("relative flex w-full min-w-0 flex-col p-2", className)}
      {...props}
    />
  )
}

function SidebarGroupLabel({ className, ...props }: React.ComponentProps<"h2">) {
  return (
    <h2
      data-slot="sidebar-group-label"
      data-sidebar="group-label"
      className={cn(
        "flex h-8 shrink-0 items-center rounded-md px-3 text-xs font-medium text-muted-foreground",
        "group-data-[collapsible=icon]:hidden",
        className
      )}
      {...props}
    />
  )
}

function SidebarMenu({ className, ...props }: React.ComponentProps<"ul">) {
  return (
    <ul
      data-slot="sidebar-menu"
      data-sidebar="menu"
      className={cn("flex w-full min-w-0 flex-col gap-1", className)}
      {...props}
    />
  )
}

function SidebarMenuItem({ className, ...props }: React.ComponentProps<"li">) {
  return (
    <li
      data-slot="sidebar-menu-item"
      data-sidebar="menu-item"
      className={cn("group/menu-item relative", className)}
      {...props}
    />
  )
}

// Entrée active : `aria-current="page"` (posé par le NavLink de React Router), en ambre.
const menuButtonClasses =
  "flex h-11 w-full items-center gap-3 overflow-hidden rounded-md px-3 text-left text-sm transition-colors hover:bg-sidebar-accent active:bg-sidebar-accent aria-[current=page]:bg-sidebar-accent aria-[current=page]:font-medium aria-[current=page]:text-sidebar-primary group-data-[collapsible=icon]:size-11 group-data-[collapsible=icon]:px-3.5 [&>span:last-child]:truncate [&>svg]:size-4 [&>svg]:shrink-0"

/** Entrée de menu ; réduite, son libellé passe dans une infobulle (et reste son nom accessible). */
function SidebarMenuButton({
  tooltip,
  className,
  ...props
}: React.ComponentProps<"a"> & { tooltip: string }) {
  const { state } = useSidebar()
  const button = (
    <Slot.Root
      data-slot="sidebar-menu-button"
      data-sidebar="menu-button"
      className={cn(menuButtonClasses, className)}
      {...props}
    />
  )
  // Dépliée, pas d'infobulle du tout : même masquée, Radix la relierait par `aria-describedby`
  // et les lecteurs d'écran liraient le libellé deux fois.
  if (state !== "collapsed") {
    return button
  }

  return (
    <Tooltip>
      <TooltipTrigger asChild>{button}</TooltipTrigger>
      <TooltipContent side="right" align="center">
        {tooltip}
      </TooltipContent>
    </Tooltip>
  )
}

function SidebarMenuSub({ className, ...props }: React.ComponentProps<"ul">) {
  return (
    <ul
      data-slot="sidebar-menu-sub"
      data-sidebar="menu-sub"
      className={cn(
        "mx-3.5 flex min-w-0 flex-col gap-1 border-l border-sidebar-border px-2.5 py-0.5",
        "group-data-[collapsible=icon]:hidden",
        className
      )}
      {...props}
    />
  )
}

function SidebarMenuSubButton({ className, ...props }: React.ComponentProps<"a">) {
  return (
    <Slot.Root
      data-slot="sidebar-menu-sub-button"
      data-sidebar="menu-sub-button"
      className={cn(
        "flex h-11 min-w-0 items-center rounded-md px-2 text-sm text-sidebar-foreground transition-colors hover:bg-sidebar-accent active:bg-sidebar-accent aria-[current=page]:bg-sidebar-accent aria-[current=page]:text-sidebar-primary [&>span:last-child]:truncate",
        className
      )}
      {...props}
    />
  )
}

export {
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
}
