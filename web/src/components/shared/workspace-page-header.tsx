import { ChevronRightIcon } from "lucide-react"
import { Fragment, type ReactNode } from "react"

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

const crumbButtonClassName =
  "shrink-0 text-muted-foreground underline-offset-4 transition-colors hover:text-foreground hover:underline"

export type WorkspaceCrumb = {
  label: string
  onClick?: () => void
}

/** Uppercase Latin letters so scope/tab crumbs match TabsTrigger `uppercase`. */
export function breadcrumbUppercase(label: string): string {
  return /[A-Za-z]/.test(label) ? label.toUpperCase() : label
}

export function instanceScopeBreadcrumbLabel(
  scope: { type: string; agent_id?: string },
  labels: { store: string; all: string },
): string {
  if (scope.type === "store") return breadcrumbUppercase(labels.store)
  if (scope.type === "agent" && scope.agent_id) return breadcrumbUppercase(scope.agent_id)
  return breadcrumbUppercase(labels.all)
}

export function WorkspaceBreadcrumb(props: {
  ariaLabel: string
  className?: string
  crumbs: WorkspaceCrumb[]
  subtitle?: string
}) {
  const crumbs = props.crumbs.filter((crumb) => crumb.label.trim().length > 0)
  if (crumbs.length === 0) return null

  return (
    <WorkspacePageHeader
      className={props.className}
      subtitle={props.subtitle}
      title={
        <nav aria-label={props.ariaLabel} className="min-w-0">
          <h1 className="flex min-w-0 items-center gap-1 text-lg font-semibold">
            {crumbs.map((crumb, index) => {
              const isCurrent = index === crumbs.length - 1
              return (
                <Fragment key={`${crumb.label}-${index}`}>
                  {index > 0 ? (
                    <ChevronRightIcon
                      className="size-4 shrink-0 text-muted-foreground"
                      aria-hidden
                    />
                  ) : null}
                  {isCurrent || !crumb.onClick ? (
                    <span className="min-w-0 truncate" title={crumb.label}>
                      {crumb.label}
                    </span>
                  ) : (
                    <button type="button" onClick={crumb.onClick} className={crumbButtonClassName}>
                      {crumb.label}
                    </button>
                  )}
                </Fragment>
              )
            })}
          </h1>
        </nav>
      }
    />
  )
}
