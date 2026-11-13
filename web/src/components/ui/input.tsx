import * as React from "react"
import { cn } from "@/lib/utils"

// Input shadcn (MCP), adapté : 44 px (tablette), 16 px de texte partout (sinon iOS zoome au
// focus), focus ambre global de index.css au lieu de l'anneau pâle de shadcn.
function Input({ className, type, ...props }: React.ComponentProps<"input">) {
  return (
    <input
      type={type}
      data-slot="input"
      className={cn(
        "h-11 w-full min-w-0 rounded-md border border-input bg-transparent px-3 py-1 text-base shadow-xs transition-colors selection:bg-primary selection:text-primary-foreground placeholder:text-muted-foreground disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50 dark:bg-input/30",
        "aria-invalid:border-destructive",
        className
      )}
      {...props}
    />
  )
}

export { Input }
