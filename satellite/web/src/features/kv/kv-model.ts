export type KvLayerId = "entity" | "relations" | "state" | "event"

export const KV_LAYER_ORDER: KvLayerId[] = ["entity", "relations", "state", "event"]

const LAYER_COUNTS_KEY: Record<KvLayerId, string> = {
  entity: "entities",
  relations: "relations",
  state: "states",
  event: "events",
}

const LAYER_REPORT_FIELD: Record<KvLayerId, string> = {
  entity: "entities",
  relations: "relations",
  state: "states",
  event: "events",
}

export const LAYER_I18N_KEY: Record<KvLayerId, string> = {
  entity: "kvEntities",
  relations: "kvRelations",
  state: "kvStates",
  event: "kvEvents",
}

export type KvKeyEntry = {
  id: string
  key: string
  type: string
  collection: string
  layer: KvLayerId
  value: Record<string, unknown>
}

export type KvCollectionNode = {
  namespace: string
  layer: KvLayerId
  type: string
  collection: string
  keyCount: number
  keys: KvKeyEntry[]
}

export type KvLayerNode = {
  layer: KvLayerId
  collections: KvCollectionNode[]
  typeCount: number
  keyCount: number
}

export type KvTree = {
  namespace: string
  kvStore?: string
  scope?: string
  layers: KvLayerNode[]
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

function isKvLayerId(value: string): value is KvLayerId {
  return KV_LAYER_ORDER.includes(value as KvLayerId)
}

export function parseCollectionPath(collection: string): {
  namespace: string
  layer: KvLayerId
  type: string
} | null {
  const parts = collection.split(":")
  if (parts.length < 3) return null
  const layer = parts[1]
  if (!isKvLayerId(layer)) return null
  return {
    namespace: parts[0],
    layer,
    type: parts.slice(2).join(":"),
  }
}

export function buildCollectionPath(namespace: string, layer: KvLayerId, type: string): string {
  return `${namespace}:${layer}:${type}`
}

function countForCollection(
  collection: string,
  counts: Record<string, unknown>,
): number | undefined {
  const parsed = parseCollectionPath(collection)
  if (!parsed) return undefined
  const typeCounts = counts[LAYER_COUNTS_KEY[parsed.layer]]
  if (!isRecord(typeCounts)) return undefined
  const count = typeCounts[parsed.type]
  return typeof count === "number" ? count : undefined
}

function stripKvMeta(item: Record<string, unknown>): Record<string, unknown> {
  const value = { ...item }
  delete value._key
  delete value._type
  delete value._collection
  return value
}

function asKvKeyEntries(items: unknown, layer: KvLayerId): KvKeyEntry[] {
  if (!Array.isArray(items)) return []

  const entries: KvKeyEntry[] = []
  items.forEach((raw, index) => {
    if (!isRecord(raw)) return
    const key = typeof raw._key === "string" ? raw._key : `unknown-${index}`
    const type = typeof raw._type === "string" ? raw._type : "unknown"
    const collection =
      typeof raw._collection === "string"
        ? raw._collection
        : buildCollectionPath("unknown", layer, type)
    entries.push({
      id: `${collection}:${key}:${index}`,
      key,
      type,
      collection,
      layer,
      value: stripKvMeta(raw),
    })
  })
  return entries
}

function emptyCollectionNode(
  namespace: string,
  layer: KvLayerId,
  type: string,
  keyCount = 0,
): KvCollectionNode {
  const collection = buildCollectionPath(namespace, layer, type)
  return { namespace, layer, type, collection, keyCount, keys: [] }
}

function buildLayerNode(
  layer: KvLayerId,
  collectionMap: Map<string, KvCollectionNode>,
): KvLayerNode {
  const collections = [...collectionMap.values()]
    .filter((node) => node.layer === layer)
    .sort(
      (a, b) =>
        b.keyCount - a.keyCount ||
        a.type.localeCompare(b.type) ||
        a.collection.localeCompare(b.collection),
    )
  const keyCount = collections.reduce((sum, node) => sum + node.keyCount, 0)
  return {
    layer,
    collections,
    typeCount: collections.length,
    keyCount,
  }
}

function buildLayersFromCollections(
  collectionMap: Map<string, KvCollectionNode>,
): KvLayerNode[] {
  return KV_LAYER_ORDER.map((layer) => buildLayerNode(layer, collectionMap)).filter(
    (layer) => layer.collections.length > 0,
  )
}

export function getLayerNode(tree: KvTree | null, layer: KvLayerId): KvLayerNode {
  return (
    tree?.layers.find((node) => node.layer === layer) ?? {
      layer,
      collections: [],
      typeCount: 0,
      keyCount: 0,
    }
  )
}

export function getAllLayerNodes(tree: KvTree | null): KvLayerNode[] {
  return KV_LAYER_ORDER.map((layer) => getLayerNode(tree, layer))
}

export function buildDisplayKvTree(
  inspectReport: unknown,
  healthReport: unknown,
): KvTree | null {
  return buildKvTreeFromInspect(inspectReport) ?? buildKvTreeFromHealth(healthReport)
}

function summarizeTree(
  namespace: string,
  layers: KvLayerNode[],
  meta: Pick<KvTree, "kvStore" | "scope" | "requestMetrics">,
): KvTree {
  const totalCollections = layers.reduce((sum, layer) => sum + layer.typeCount, 0)
  const totalKeys = layers.reduce((sum, layer) => sum + layer.keyCount, 0)
  return {
    namespace,
    kvStore: meta.kvStore,
    scope: meta.scope,
    requestMetrics: meta.requestMetrics,
    layers,
    totalCollections,
    totalKeys,
    totalTypes: totalCollections,
  }
}

export function buildKvTreeFromHealth(report: unknown): KvTree | null {
  if (!isRecord(report)) return null

  const namespace = typeof report.namespace === "string" ? report.namespace : "default"
  const kvStore = typeof report.kvStore === "string" ? report.kvStore : undefined
  const collectionMap = new Map<string, KvCollectionNode>()

  for (const layer of KV_LAYER_ORDER) {
    for (const type of asStringArray(report[LAYER_REPORT_FIELD[layer]])) {
      const collection = buildCollectionPath(namespace, layer, type)
      collectionMap.set(collection, emptyCollectionNode(namespace, layer, type))
    }
  }

  return summarizeTree(namespace, buildLayersFromCollections(collectionMap), { kvStore })
}

export function buildKvTreeFromInspect(report: unknown): KvTree | null {
  if (!isRecord(report)) return null

  const namespace = typeof report.namespace === "string" ? report.namespace : "default"
  const kvStore = typeof report.kvStore === "string" ? report.kvStore : undefined
  const scope = typeof report.scope === "string" ? report.scope : undefined
  const counts = isRecord(report.counts) ? report.counts : {}
  const requestMetrics = isRecord(report.request_metrics) ? report.request_metrics : undefined
  const collectionMap = new Map<string, KvCollectionNode>()

  for (const collection of asStringArray(report.collections)) {
    const parsed = parseCollectionPath(collection)
    if (!parsed) continue
    collectionMap.set(
      collection,
      emptyCollectionNode(
        parsed.namespace,
        parsed.layer,
        parsed.type,
        countForCollection(collection, counts) ?? 0,
      ),
    )
  }

  for (const layer of KV_LAYER_ORDER) {
    for (const entry of asKvKeyEntries(report[LAYER_REPORT_FIELD[layer]], layer)) {
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

  return summarizeTree(namespace, buildLayersFromCollections(collectionMap), {
    kvStore,
    scope,
    requestMetrics,
  })
}

export function countHealthActiveTypes(report: unknown): number {
  const tree = buildKvTreeFromHealth(report)
  return tree?.totalTypes ?? 0
}

export function countInspectKeys(report: unknown): number {
  const tree = buildKvTreeFromInspect(report)
  return tree?.totalKeys ?? 0
}

export function extractKvNamespace(...reports: unknown[]): string | null {
  for (const report of reports) {
    if (isRecord(report) && typeof report.namespace === "string") return report.namespace
  }
  return null
}

export function findKvKeyEntry(
  tree: KvTree | null,
  keyId: string | null,
): KvKeyEntry | null {
  if (!tree || !keyId) return null
  for (const layer of tree.layers) {
    for (const collection of layer.collections) {
      const match = collection.keys.find((entry) => entry.id === keyId)
      if (match) return match
    }
  }
  return null
}
