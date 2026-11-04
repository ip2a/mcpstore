import { FolderSearchIcon, RefreshCwIcon, ServerOffIcon } from "lucide-react"

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty"
import { Skeleton } from "@/components/ui/skeleton"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"

export function PageSkeleton({ className }: { className?: string }) {
  return (
    <div className={cn("flex flex-col gap-4", className)}>
      <Skeleton className="h-8 w-64" />
      <Skeleton className="h-28 w-full" />
      <Skeleton className="h-48 w-full" />
    </div>
  )
}

export function PageEmpty({ className, title, description, onRefresh }: { className?: string; title: string; description?: string; onRefresh?: () => void }) {
  const { t } = useI18n()

  return (
    <Empty className={className}>
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <FolderSearchIcon />
        </EmptyMedia>
        <EmptyTitle>{title}</EmptyTitle>
        {description ? <EmptyDescription>{description}</EmptyDescription> : null}
      </EmptyHeader>
      {onRefresh ? (
        <EmptyContent>
          <Button variant="outline" onClick={onRefresh}>
            <RefreshCwIcon data-icon="inline-start" />
            {t("refresh")}
          </Button>
        </EmptyContent>
      ) : null}
    </Empty>
  )
}

export function PageError({ className, title, message, onRefresh }: { className?: string; title?: string; message?: string; onRefresh?: () => void }) {
  const { t } = useI18n()

  return (
    <Alert className={cn("border-border/80 bg-muted/30 text-foreground shadow-none [&>svg]:text-muted-foreground", className)}>
      <ServerOffIcon />
      <div className="flex min-w-0 flex-col gap-1">
        <AlertTitle>{title ?? "Display error"}</AlertTitle>
        <AlertDescription className="text-muted-foreground">{message ?? "Please check the logs."}</AlertDescription>
      </div>
      {onRefresh ? (
        <div className="mt-1">
          <Button variant="outline" size="sm" onClick={onRefresh}>
            <RefreshCwIcon data-icon="inline-start" />
            {t("refresh")}
          </Button>
        </div>
      ) : null}
    </Alert>
  )
}
