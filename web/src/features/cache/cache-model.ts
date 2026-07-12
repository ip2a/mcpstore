export type CacheLayerId = "entity" | "relations" | "state" | "event"

export const CACHE_LAYER_ORDER: CacheLayerId[] = ["entity", "relations", "state", "event"]

const LAYER_COUNTS_KEY: Record<CacheLayerId, string> = {
  entity: "entities",
  relations: "relations",
  state: "states",
  event: "events",
}

const LAYER_REPORT_FIELD: Record<CacheLayerId, string> = {
  entity: "entities",
  relations: "relations",
  state: "states",
  event: "events",
}

export const LAYER_I18N_KEY: Record<CacheLayerId, string> = {
  entity: "cacheEntities",
  relations: "cacheRelations",
  state: "cacheStates",
  event: "cacheEvents",
}

export type CacheKeyEntry = {
  id: string
  key: string
  type: string
  collection: string
  layer: CacheLayerId
  value: Record<string, unknown>
}

export type CacheCollectionNode = {
  namespace: string
  layer: CacheLayerId
  type: string
  collection: string
  keyCount: number
  keys: CacheKeyEntry[]
}

export type CacheLayerNode = {
  layer: CacheLayerId
  collections: CacheCollectionNode[]
  typeCount: number
  keyCount: number
}

export type CacheTree = {
  namespace: string
  backend?: string
  scope?: string
  layers: CacheLayerNode[]
  totalCollections: number
  totalKeys: number
  totalTypes: number
  requestMetrics?: Record<string, unknown>
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value)
}

function asStringArray(value: unknown): string[] {
  if (!Array.isArray(value)) return []
  return value.filter((item): item is string => typeof item === "string")
}

function isCacheLayerId(value: string): value is CacheLayerId {
  return CACHE_LAYER_ORDER.includes(value as CacheLayerId)
}

export function parseCollectionPath(collection: string): {
  namespace: string
  layer: CacheLayerId
  type: string
} | null {
  const parts = collection.split(":")
  if (parts.length < 3) return null
  const layer = parts[1]
  if (!isCacheLayerId(layer)) return null
  return {
    namespace: parts[0],
    layer,
    type: parts.slice(2).join(":"),
  }
}

export function buildCollectionPath(namespace: string, layer: CacheLayerId, type: string): string {
  return `${namespace}:${layer}:${type}`
}

function countForCollection(collection: string, counts: Record<string, unknown>): number | undefined {
  const parsed = parseCollectionPath(collection)
  if (!parsed) return undefined
  const typeCounts = counts[LAYER_COUNTS_KEY[parsed.layer]]
  if (!isRecord(typeCounts)) return undefined
  const count = typeCounts[parsed.type]
  return typeof count === "number" ? count : undefined
}

function stripCacheMeta(item: Record<string, unknown>): Record<string, unknown> {
  const value = { ...item }
  delete value._key
  delete value._type
  delete value._collection
  return value
}

function asCacheKeyEntries(items: unknown, layer: CacheLayerId): CacheKeyEntry[] {
  if (!Array.isArray(items)) return []

  const entries: CacheKeyEntry[] = []
  items.forEach((raw, index) => {
    if (!isRecord(raw)) return
    const key = typeof raw._key === "string" ? raw._key : `unknown-${index}`
    const type = typeof raw._type === "string" ? raw._type : "unknown"
    const collection =
      typeof raw._collection === "string" ? raw._collection : buildCollectionPath("unknown", layer, type)
    entries.push({
      id: `${collection}:${key}:${index}`,
      key,
      type,
      collection,
      layer,
      value: stripCacheMeta(raw),
    })
  })
  return entries
}

function emptyCollectionNode(
  namespace: string,
  layer: CacheLayerId,
  type: string,
  keyCount = 0,
): CacheCollectionNode {
  const collection = buildCollectionPath(namespace, layer, type)
  return { namespace, layer, type, collection, keyCount, keys: [] }
}

function buildLayerNode(layer: CacheLayerId, collectionMap: Map<string, CacheCollectionNode>): CacheLayerNode {
  const collections = [...collectionMap.values()]
    .filter((node) => node.layer === layer)
    .sort((a, b) => b.keyCount - a.keyCount || a.type.localeCompare(b.type) || a.collection.localeCompare(b.collection))
  const keyCount = collections.reduce((sum, node) => sum + node.keyCount, 0)
  return {
    layer,
    collections,
    typeCount: collections.length,
    keyCount,
  }
}

function buildLayersFromCollections(collectionMap: Map<string, CacheCollectionNode>): CacheLayerNode[] {
  return CACHE_LAYER_ORDER.map((layer) => buildLayerNode(layer, collectionMap)).filter(
    (layer) => layer.collections.length > 0,
  )
}

export function getLayerNode(tree: CacheTree | null, layer: CacheLayerId): CacheLayerNode {
  return tree?.layers.find((node) => node.layer === layer) ?? { layer, collections: [], typeCount: 0, keyCount: 0 }
}

export function getAllLayerNodes(tree: CacheTree | null): CacheLayerNode[] {
  return CACHE_LAYER_ORDER.map((layer) => getLayerNode(tree, layer))
}

export function buildDisplayCacheTree(inspectReport: unknown, healthReport: unknown): CacheTree | null {
  return buildCacheTreeFromInspect(inspectReport) ?? buildCacheTreeFromHealth(healthReport)
}

function summarizeTree(
  namespace: string,
  layers: CacheLayerNode[],
  meta: Pick<CacheTree, "backend" | "scope" | "requestMetrics">,
): CacheTree {
  const totalCollections = layers.reduce((sum, layer) => sum + layer.typeCount, 0)
  const totalKeys = layers.reduce((sum, layer) => sum + layer.keyCount, 0)
  return {
    namespace,
    backend: meta.backend,
    scope: meta.scope,
    requestMetrics: meta.requestMetrics,
    layers,
    totalCollections,
    totalKeys,
    totalTypes: totalCollections,
  }
}

export function buildCacheTreeFromHealth(report: unknown): CacheTree | null {
  if (!isRecord(report)) return null

  const namespace = typeof report.namespace === "string" ? report.namespace : "default"
  const backend = typeof report.backend === "string" ? report.backend : undefined
  const collectionMap = new Map<string, CacheCollectionNode>()

  for (const layer of CACHE_LAYER_ORDER) {
    for (const type of asStringArray(report[LAYER_REPORT_FIELD[layer]])) {
      const collection = buildCollectionPath(namespace, layer, type)
      collectionMap.set(collection, emptyCollectionNode(namespace, layer, type))
    }
  }

  return summarizeTree(namespace, buildLayersFromCollections(collectionMap), { backend })
}

export function buildCacheTreeFromInspect(report: unknown): CacheTree | null {
  if (!isRecord(report)) return null

  const namespace = typeof report.namespace === "string" ? report.namespace : "default"
  const backend = typeof report.backend === "string" ? report.backend : undefined
  const scope = typeof report.scope === "string" ? report.scope : undefined
  const counts = isRecord(report.counts) ? report.counts : {}
  const requestMetrics = isRecord(report.request_metrics) ? report.request_metrics : undefined
  const collectionMap = new Map<string, CacheCollectionNode>()

  for (const collection of asStringArray(report.collections)) {
    const parsed = parseCollectionPath(collection)
    if (!parsed) continue
    collectionMap.set(
      collection,
      emptyCollectionNode(parsed.namespace, parsed.layer, parsed.type, countForCollection(collection, counts) ?? 0),
    )
  }

  for (const layer of CACHE_LAYER_ORDER) {
    for (const entry of asCacheKeyEntries(report[LAYER_REPORT_FIELD[layer]], layer)) {
      let node = collectionMap.get(entry.collection)
      if (!node) {
        const parsed = parseCollectionPath(entry.collection)
        node = emptyCollectionNode(
          parsed?.namespace ?? namespace,
          parsed?.layer ?? layer,
          parsed?.type ?? entry.type,
        )
        collectionMap.set(entry.collection, node)
      }
      node.keys.push(entry)
      node.keyCount = node.keys.length
    }
  }

  for (const node of collectionMap.values()) {
    node.keys.sort((a, b) => a.key.localeCompare(b.key))
    if (node.keys.length === 0 && node.keyCount === 0) {
      const count = countForCollection(node.collection, counts)
      if (typeof count === "number") node.keyCount = count
    }
  }

  return summarizeTree(namespace, buildLayersFromCollections(collectionMap), { backend, scope, requestMetrics })
}

export function countHealthActiveTypes(report: unknown): number {
  const tree = buildCacheTreeFromHealth(report)
  return tree?.totalTypes ?? 0
}

export function countInspectKeys(report: unknown): number {
  const tree = buildCacheTreeFromInspect(report)
  return tree?.totalKeys ?? 0
}

export function extractCacheNamespace(...reports: unknown[]): string | null {
  for (const report of reports) {
    if (isRecord(report) && typeof report.namespace === "string") return report.namespace
  }
  return null
}

export function findCacheKeyEntry(tree: CacheTree | null, keyId: string | null): CacheKeyEntry | null {
  if (!tree || !keyId) return null
  for (const layer of tree.layers) {
    for (const collection of layer.collections) {
      const match = collection.keys.find((entry) => entry.id === keyId)
      if (match) return match
    }
  }
  return null
}
