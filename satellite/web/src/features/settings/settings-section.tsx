import type { ReactNode } from "react"

import { cn } from "@/lib/utils"

/** Section title above a bordered card of setting rows (Cursor-style prefs). */
export function SettingsSection({
  title,
  description,
  children,
  className,
}: {
  title?: string
  description?: string
  children: ReactNode
  className?: string
}) {
  return (
    <div className={cn("flex flex-col gap-2", className)}>
      {title || description ? (
        <div className="px-0.5">
          {title ? <h3 className="text-sm font-medium text-foreground">{title}</h3> : null}
          {description ? (
            <p className="mt-0.5 text-xs text-muted-foreground">{description}</p>
          ) : null}
        </div>
      ) : null}
      <div className="divide-y divide-border/60 overflow-hidden rounded-xl border border-border/60 bg-card">
        {children}
      </div>
    </div>
  )
}

/** One settings row: title + description on the left, control on the right. */
export function SettingsRow({
  title,
  description,
  children,
  className,
}: {
  title: string
  description?: string
  children: ReactNode
  className?: string
}) {
  return (
    <div className={cn("flex items-center justify-between gap-4 px-4 py-3.5", className)}>
      <div className="min-w-0 flex-1 space-y-0.5">
        <div className="text-sm leading-snug font-medium">{title}</div>
        {description ? (
          <p className="text-xs leading-normal text-muted-foreground">{description}</p>
        ) : null}
      </div>
      <div className="flex shrink-0 items-center">{children}</div>
    </div>
  )
}
