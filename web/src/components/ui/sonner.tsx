import {
  CircleCheckIcon,
  InfoIcon,
  Loader2Icon,
  OctagonXIcon,
  TriangleAlertIcon,
} from "lucide-react"
import type { CSSProperties } from "react"
import { Toaster as Sonner, type ToasterProps } from "sonner"
import { useShownTheme } from "@/lib/theme"

// Thème de sonner d'après le thème affiché (pas de next-themes) : le fond suit nos jetons, mais
// sonner colore lui-même la description (gris clair en sombre, illisible sur fond clair).
// Notifications fermées seules au bout de 5 s (non critiques), au-dessus de la barre d'onglets
// et de la zone sûre du bas (PWA).
const BOTTOM = "calc(var(--tabbar-height) + env(safe-area-inset-bottom) + 16px)"
const Toaster = ({ ...props }: ToasterProps) => {
  const shown = useShownTheme()
  return (
    <Sonner
      theme={shown === "dark" || shown === "oled" ? "dark" : "light"}
      duration={5000}
      offset={{ bottom: BOTTOM }}
      mobileOffset={{ bottom: BOTTOM }}
      className="toaster group"
      icons={{
        success: <CircleCheckIcon className="size-4" />,
        info: <InfoIcon className="size-4" />,
        warning: <TriangleAlertIcon className="size-4" />,
        error: <OctagonXIcon className="size-4" />,
        loading: <Loader2Icon className="size-4 animate-spin" />,
      }}
      style={
        {
          "--normal-bg": "var(--popover)",
          "--normal-text": "var(--popover-foreground)",
          "--normal-border": "var(--border)",
          "--border-radius": "var(--radius)",
        } as CSSProperties
      }
      {...props}
    />
  )
}

export { Toaster }
