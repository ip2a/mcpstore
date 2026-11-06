import { CacheKeysTable } from "@/features/cache/cache-layer-browser"
import type { CacheLayerNode } from "@/features/cache/cache-model"

export function CacheLayerContent({ layers }: { layers: CacheLayerNode[] }) {
  return (
    <div className="flex h-full min-h-0 flex-1 flex-col overflow-hidden">
      <CacheKeysTable layers={layers} />
    </div>
  )
}
