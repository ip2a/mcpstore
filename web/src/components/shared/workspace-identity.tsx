import { PathText } from "@/components/shared/path-text"
import { workspaceName } from "@/components/shared/workspace-name"
import { cn } from "@/lib/utils"

type WorkspaceIdentityProps = {
  className?: string
  fallbackTitle?: string
  label?: string
  labelClassName?: string
  pathClassName?: string
  titleClassName?: string
  workspace: string | null | undefined
}

export function WorkspaceIdentity({
  className,
  fallbackTitle = "No workspace",
  label = "Workspace",
  labelClassName,
  pathClassName,
  titleClassName,
  workspace,
}: WorkspaceIdentityProps) {
  return (
    <div className={cn("flex min-w-0 flex-col gap-1", className)}>
      <span className={cn("font-mono text-xs uppercase text-muted-foreground", labelClassName)}>{label}</span>
      <strong className={cn("truncate text-lg font-semibold leading-tight", titleClassName)}>{workspaceName(workspace, fallbackTitle)}</strong>
      <PathText value={workspace} className={pathClassName} wrap="all" />
    </div>
  )
}
