import { useEffect, useMemo, useRef, useState } from "react"
import { useInfiniteQuery, useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { GlobeIcon, PuzzleIcon, RefreshCwIcon } from "lucide-react"
import { toast } from "sonner"

import { useScopeServicesQuery } from "@/features/agents/queries"
import { PageSkeleton } from "@/components/shared/page-states"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Spinner } from "@/components/ui/spinner"
import {
  fillMarketServiceConfig,
  listFeaturedMarketServices,
  listRegistryMarketServices,
  type MarketServiceInput,
  type MarketServiceItem,
} from "@/lib/api/plugins"
import { addServiceFromConfig } from "@/lib/api/service-config"
import { queryKeys } from "@/lib/query-keys"
import { useI18n } from "@/lib/i18n-context"

const REGISTRY_PAGE_SIZE = 20

/** Debounce the shared search box before it reaches the (slow) registry. */
function useDebounced(value: string, ms: number) {
  const [debounced, setDebounced] = useState(value)
  useEffect(() => {
    const timer = setTimeout(() => setDebounced(value), ms)
    return () => clearTimeout(timer)
  }, [value, ms])
  return debounced
}

/** The Services tab: the featured catalogue first, then the official MCP
 * Registry paged in as the list is scrolled down. A registry failure never
 * takes the featured list down with it. */
export function ServicesMarketTab({ query }: { query: string }) {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  const [registryVisible, setRegistryVisible] = useState(false)
  const sentinelRef = useRef<HTMLDivElement | null>(null)
  const [installing, setInstalling] = useState<MarketServiceItem | null>(null)

  const normalizedQuery = useDebounced(query.trim().toLowerCase(), 500)

  const featuredQuery = useQuery({
    queryKey: queryKeys.servicesMarketFeatured,
    queryFn: listFeaturedMarketServices,
  })
  const servicesQuery = useScopeServicesQuery({ type: "store" })
  const registryQuery = useInfiniteQuery({
    queryKey: queryKeys.servicesMarketRegistry(normalizedQuery),
    queryFn: ({ pageParam }) =>
      listRegistryMarketServices({
        q: normalizedQuery,
        offset: pageParam,
        limit: REGISTRY_PAGE_SIZE,
      }),
    initialPageParam: 0,
    getNextPageParam: (last) => {
      const next = last.offset + last.items.length
      return next < last.total ? next : undefined
    },
    // A search reaches out immediately; without one the registry waits for
    // the list to be scrolled past the featured catalogue.
    enabled: normalizedQuery !== "" || registryVisible,
  })
  const installMutation = useMutation({
    mutationFn: async ({
      item,
      values,
    }: {
      item: MarketServiceItem
      values: Record<string, string>
    }) => {
      await addServiceFromConfig({
        name: item.name,
        scope: { type: "store" },
        config: fillMarketServiceConfig(item, values),
      })
    },
    onSuccess: (_result, { item }) => {
      toast.success(t("serviceMarketInstallDone", { name: item.title }))
      void queryClient.invalidateQueries({ queryKey: queryKeys.scopeServices({ type: "store" }) })
    },
    onError: (error) => {
      toast.error(error instanceof Error ? error.message : String(error))
    },
  })

  // The registry section loads when its sentinel first shows in the window.
  useEffect(() => {
    const sentinel = sentinelRef.current
    if (!sentinel || registryVisible) return
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) setRegistryVisible(true)
      },
      { rootMargin: "400px" },
    )
    observer.observe(sentinel)
    return () => observer.disconnect()
  }, [registryVisible])

  const installingBusy = installMutation.isPending
  const installedNames = useMemo(
    () => new Set((servicesQuery.data ?? []).map((service) => service.service_name.toLowerCase())),
    [servicesQuery.data],
  )
  const featuredItems = useMemo(
    () => filterByQuery(featuredQuery.data?.items, normalizedQuery),
    [featuredQuery.data, normalizedQuery],
  )
  const registryItems = useMemo(
    () => registryQuery.data?.pages.flatMap((page) => page.items) ?? [],
    [registryQuery.data],
  )
  const registryFailed =
    registryQuery.isError || (registryQuery.data?.pages.some((page) => page.failed) ?? false)
  const shownRegistryItems = useMemo(
    () =>
      registryItems.filter((item) => !featuredItems.some((featured) => featured.id === item.id)),
    [registryItems, featuredItems],
  )
  const hasResults = featuredItems.length > 0 || shownRegistryItems.length > 0
  const registryFetching = registryQuery.isFetching

  if (featuredQuery.isPending && !featuredQuery.data) {
    return (
      <div className="px-4 py-2">
        <PageSkeleton />
      </div>
    )
  }
  if (featuredQuery.isError) {
    return (
      <div className="flex h-full items-center justify-center px-4 py-8 text-sm text-muted-foreground">
        {featuredQuery.error instanceof Error
          ? featuredQuery.error.message
          : String(featuredQuery.error)}
      </div>
    )
  }

  return (
    <ScrollPane className="min-h-0 flex-1" innerClassName="px-4 py-2">
      {featuredItems.length > 0 ? (
        <section>
          <h2 className="mb-3 text-sm font-semibold text-muted-foreground">
            {t("serviceMarketFeatured")}
          </h2>
          <div className="grid grid-cols-[repeat(auto-fill,minmax(min(100%,16rem),16rem))] gap-4">
            {featuredItems.map((item) => (
              <ServiceMarketCard
                key={`featured:${item.id}`}
                busy={installingBusy}
                installed={installedNames.has(item.name.toLowerCase())}
                item={item}
                onInstall={() => setInstalling(item)}
              />
            ))}
          </div>
        </section>
      ) : null}

      <section className={featuredItems.length > 0 ? "mt-4" : undefined}>
        <h2 className="mb-3 text-sm font-semibold text-muted-foreground">
          {t("serviceMarketRegistry")}
        </h2>
        {registryFailed ? (
          <p className="mb-3 flex items-center gap-2 text-xs text-muted-foreground">
            <RefreshCwIcon className="size-3.5" aria-hidden />
            {t("serviceMarketRegistryFailed")}
          </p>
        ) : null}
        {shownRegistryItems.length > 0 ? (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(min(100%,16rem),16rem))] gap-4">
            {shownRegistryItems.map((item) => (
              <ServiceMarketCard
                key={item.id}
                busy={installingBusy}
                installed={installedNames.has(item.name.toLowerCase())}
                item={item}
                onInstall={() => setInstalling(item)}
              />
            ))}
          </div>
        ) : !registryFailed && !registryFetching ? (
          <p className="text-xs text-muted-foreground">
            {registryQuery.data ? t("serviceMarketNoResults") : t("serviceMarketScrollHint")}
          </p>
        ) : null}
        <div ref={sentinelRef} className="h-px" aria-hidden />
        {registryFetching ? (
          <p className="mt-3 flex items-center gap-2 text-xs text-muted-foreground">
            <Spinner className="size-3.5" />
            {t("serviceMarketLoading")}
          </p>
        ) : null}
        {!registryQuery.hasNextPage && registryItems.length > 0 && !registryFailed ? (
          <p className="mt-3 text-xs text-muted-foreground">{t("serviceMarketNoMore")}</p>
        ) : null}
      </section>

      {!hasResults && !registryFetching && !registryFailed ? (
        <p className="py-8 text-center text-sm text-muted-foreground">{t("serviceMarketEmpty")}</p>
      ) : null}

      <InstallServiceDialog
        key={installing?.id ?? "idle"}
        busy={installingBusy}
        item={installing}
        onOpenChange={(open) => {
          if (!open) setInstalling(null)
        }}
        onInstalled={async (item, values) => {
          await installMutation.mutateAsync({ item, values })
          setInstalling(null)
        }}
      />
    </ScrollPane>
  )
}

function filterByQuery(items: MarketServiceItem[] | undefined, query: string) {
  const list = items ?? []
  if (!query) return list
  return list.filter((item) =>
    [item.title, item.id, item.name, item.publisher, item.description]
      .filter((field): field is string => Boolean(field))
      .some((field) => field.toLowerCase().includes(query)),
  )
}

function ServiceMarketCard({
  busy,
  installed,
  item,
  onInstall,
}: {
  busy: boolean
  installed: boolean
  item: MarketServiceItem
  onInstall: () => void
}) {
  const { t } = useI18n()
  const [iconBroken, setIconBroken] = useState(false)
  const icon = item.icon && !iconBroken ? item.icon : null

  return (
    <div className="flex flex-col rounded-lg border border-border/50 bg-card p-4 transition-colors hover:border-border hover:bg-muted/40">
      <div className="flex min-w-0 items-center gap-2">
        <span className="flex size-8 shrink-0 items-center justify-center rounded-md border border-border/50 bg-muted/60 text-muted-foreground">
          {icon ? (
            <img
              src={icon}
              alt=""
              className="size-5 object-contain"
              aria-hidden
              loading="lazy"
              onError={() => setIconBroken(true)}
            />
          ) : (
            <PuzzleIcon className="size-4" aria-hidden />
          )}
        </span>
        <span className="min-w-0 flex-1">
          <span className="line-clamp-1 text-sm font-semibold leading-snug">{item.title}</span>
          <span className="mt-0.5 flex min-w-0 items-center gap-1 text-xs text-muted-foreground">
            <span
              className={`size-1.5 shrink-0 rounded-full ${installed ? "bg-emerald-500" : "bg-muted-foreground/40"}`}
            />
            <span className="truncate">{item.publisher || item.id}</span>
          </span>
        </span>
        <Button size="sm" className="shrink-0" disabled={busy || installed} onClick={onInstall}>
          {installed ? t("clientPluginInstalled") : t("clientPluginInstall")}
        </Button>
      </div>

      <p className="mt-3 min-h-10 line-clamp-2 text-xs leading-5 text-muted-foreground">
        {item.description?.trim() || t("noDescription")}
      </p>

      <div className="mt-3 flex flex-wrap items-center gap-1.5">
        {item.matched ? (
          <Badge variant="outline" className="text-[10px]">
            <GlobeIcon aria-hidden />
            {t("serviceMarketAddressMatch")}
          </Badge>
        ) : null}
        {item.sign_in ? (
          <Badge variant="outline" className="text-[10px]">
            {t("serviceMarketSignIn")}
          </Badge>
        ) : null}
        <span className="font-mono text-[10px] tabular-nums text-muted-foreground">
          {item.transport}
        </span>
      </div>
    </div>
  )
}

/** Collects the values an item's inputs need before its config is complete. */
function InstallServiceDialog({
  busy,
  item,
  onInstalled,
  onOpenChange,
}: {
  busy: boolean
  item: MarketServiceItem | null
  onInstalled: (item: MarketServiceItem, values: Record<string, string>) => Promise<void>
  onOpenChange: (open: boolean) => void
}) {
  const { t } = useI18n()
  // Remounted per target item (keyed by the caller): state starts clean.
  const [values, setValues] = useState<Record<string, string>>({})
  const [submitting, setSubmitting] = useState(false)

  const requiredInputs = item?.inputs.filter((input) => input.required) ?? []
  const complete = requiredInputs.every((input) => (values[input.key] ?? "").trim() !== "")

  async function submit() {
    if (!item || !complete) return
    setSubmitting(true)
    try {
      await onInstalled(item, values)
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <Dialog open={item !== null} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>
            {t("serviceMarketInstallSettings", { name: item?.title ?? "" })}
          </DialogTitle>
        </DialogHeader>
        {item && item.inputs.length > 0 ? (
          <div className="flex flex-col gap-3">
            {item.inputs.map((input) => (
              <MarketInputField
                key={input.key}
                input={input}
                value={values[input.key] ?? ""}
                onChange={(value) => setValues((prev) => ({ ...prev, [input.key]: value }))}
              />
            ))}
          </div>
        ) : (
          <p className="text-sm text-muted-foreground">{t("serviceMarketInstallHint")}</p>
        )}
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            {t("cancel")}
          </Button>
          <Button disabled={busy || submitting || !complete} onClick={() => void submit()}>
            {submitting ? <Spinner className="size-4" /> : null}
            {t("clientPluginInstall")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

function MarketInputField({
  input,
  value,
  onChange,
}: {
  input: MarketServiceInput
  value: string
  onChange: (value: string) => void
}) {
  return (
    <div className="flex flex-col gap-1.5">
      <Label htmlFor={`market-input-${input.key}`}>
        {input.label}
        {input.required ? <span className="ms-1 text-destructive">*</span> : null}
      </Label>
      <Input
        id={`market-input-${input.key}`}
        type={input.secret ? "password" : "text"}
        placeholder={input.placeholder}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
      {input.description ? (
        <p className="text-xs text-muted-foreground">{input.description}</p>
      ) : null}
    </div>
  )
}
