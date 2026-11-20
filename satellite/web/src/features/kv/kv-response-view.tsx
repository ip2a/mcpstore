import { KvKeysTable } from "@/features/kv/kv-layer-browser"
import type { KvLayerNode } from "@/features/kv/kv-model"

export function KvLayerContent({ layers, search }: { layers: KvLayerNode[]; search?: string }) {
  return (
    <div className="flex h-full min-h-0 flex-1 flex-col overflow-hidden">
      <KvKeysTable layers={layers} search={search} />
    </div>
  )
}
