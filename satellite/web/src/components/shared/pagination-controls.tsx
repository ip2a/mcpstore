import { ChevronLeftIcon, ChevronRightIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"

export interface PaginationControlsProps {
  from: number
  to: number
  total: number
  pageSize: number
  pageSizes: readonly number[]
  page: number
  pageCount: number
  onPageSizeChange: (value: string) => void
  onPrevPage: () => void
  onNextPage: () => void
  className?: string
}

export function PaginationControls({
  from,
  to,
  total,
  pageSize,
  pageSizes,
  page,
  pageCount,
  onPageSizeChange,
  onPrevPage,
  onNextPage,
  className,
}: PaginationControlsProps) {
  const { t } = useI18n()

  if (total === 0) return null

  return (
    <div
      className={cn(
        "flex flex-wrap items-center justify-between gap-2 shrink-0 border-t px-4 py-2",
        className,
      )}
    >
      <div className="flex items-center gap-2 text-xs text-muted-foreground">
        <span>{t("gatewayFramesPageSize")}</span>
        <Select value={String(pageSize)} onValueChange={onPageSizeChange}>
          <SelectTrigger size="sm" className="h-8 w-[72px]">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {pageSizes.map((size) => (
              <SelectItem key={size} value={String(size)}>
                {size}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <span>{t("gatewayFramesShowing", { from, to, total })}</span>
      </div>
      <div className="flex items-center gap-1">
        <Button disabled={page <= 0} size="sm" variant="outline" onClick={onPrevPage}>
          <ChevronLeftIcon className="size-3.5" />
          {t("gatewayFramesPrev")}
        </Button>
        <Button disabled={page + 1 >= pageCount} size="sm" variant="outline" onClick={onNextPage}>
          {t("gatewayFramesNext")}
          <ChevronRightIcon className="size-3.5" />
        </Button>
      </div>
    </div>
  )
}
