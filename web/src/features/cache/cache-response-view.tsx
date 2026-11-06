import { CacheCollectionDetail } from "@/features/cache/cache-layer-browser"
import type { CacheCollectionNode } from "@/features/cache/cache-model"
import { useI18n } from "@/lib/i18n-context"

export function CacheLayerContent({ collection }: { collection: CacheCollectionNode | null }) {
  const { t } = useI18n()

  if (!collection) {
    return <p className="text-sm text-muted-foreground">{t("cacheSelectCollectionHint")}</p>
  }

  return (
    <div className="flex h-full min-h-0 flex-1 flex-col overflow-hidden">
      <CacheCollectionDetail collection={collection} />
    </div>
  )
}
