import { FolderSearchIcon, RefreshCwIcon } from "lucide-react"

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty"
import { Skeleton } from "@/components/ui/skeleton"
import { useI18n } from "@/lib/i18n-context"

export function PageSkeleton() {
  return (
    <div className="flex flex-col gap-4">
      <Skeleton className="h-8 w-64" />
      <Skeleton className="h-28 w-full" />
      <Skeleton className="h-48 w-full" />
    </div>
  )
}

export function PageEmpty({ title, description, onRefresh }: { title: string; description: string; onRefresh?: () => void }) {
  const { t } = useI18n()

  return (
    <Empty>
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <FolderSearchIcon />
        </EmptyMedia>
        <EmptyTitle>{title}</EmptyTitle>
        <EmptyDescription>{description}</EmptyDescription>
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

export function PageError({ title, message, onRefresh }: { title: string; message: string; onRefresh?: () => void }) {
  const { t } = useI18n()

  return (
    <Alert variant="destructive" className="flex flex-col gap-3">
      <div className="flex flex-col gap-1">
        <AlertTitle>{title}</AlertTitle>
        <AlertDescription>{message}</AlertDescription>
      </div>
      {onRefresh ? (
        <div>
          <Button variant="outline" onClick={onRefresh}>
            <RefreshCwIcon data-icon="inline-start" />
            {t("refresh")}
          </Button>
        </div>
      ) : null}
    </Alert>
  )
}
