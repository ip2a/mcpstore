import { KvKeysTable } from "@/features/kv/kv-layer-browser"
import type { KvLayerNode } from "@/features/kv/kv-model"

export function KvLayerContent({ layers }: { layers: KvLayerNode[] }) {
  return (
    <div className="flex h-full min-h-0 flex-1 flex-col overflow-hidden">
      <KvKeysTable layers={layers} />
    </div>
  )
}
