import { ChevronRightIcon } from "lucide-react"
import type { ReactNode } from "react"

import { cn } from "@/lib/utils"

export function WorkspacePageHeader(props: {
  className?: string
  subtitle?: string
  title: ReactNode
}) {
  return (
    <div className={cn("min-w-0 shrink-0 pl-4", props.className)}>
      {props.title}
      {props.subtitle ? (
        <p className="mt-1 truncate text-sm text-muted-foreground" title={props.subtitle}>
          {props.subtitle}
        </p>
      ) : null}
    </div>
  )
}

export function WorkspaceBreadcrumb(props: {
  ariaLabel: string
  className?: string
  currentLabel: string
  onParentClick: () => void
  parentLabel: string
  subtitle?: string
}) {
  return (
    <WorkspacePageHeader
      className={props.className}
      subtitle={props.subtitle}
      title={
        <nav aria-label={props.ariaLabel} className="min-w-0">
          <h1 className="flex min-w-0 items-center gap-1 text-lg font-semibold">
            <button
              type="button"
              onClick={props.onParentClick}
              className="shrink-0 text-muted-foreground underline-offset-4 transition-colors hover:text-foreground hover:underline"
            >
              {props.parentLabel}
            </button>
            <ChevronRightIcon className="size-4 shrink-0 text-muted-foreground" aria-hidden />
            <span className="min-w-0 truncate" title={props.currentLabel}>
              {props.currentLabel}
            </span>
          </h1>
        </nav>
      }
    />
  )
}
