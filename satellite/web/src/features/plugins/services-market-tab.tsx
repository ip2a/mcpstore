import { useEffect, useMemo, useState } from "react"
import { useInfiniteQuery, useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { GlobeIcon, RefreshCwIcon } from "lucide-react"
import { toast } from "sonner"

import { useScopeServicesQuery } from "@/features/agents/queries"
import { BasicCard, basicCardIconClassName } from "@/components/shared/basic-card"
import { DefaultServiceIcon } from "@/components/shared/default-service-icon"
import { PageSkeleton } from "@/components/shared/page-states"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { PaginationControls } from "@/components/shared/pagination-controls"
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
import { usePagination, getPaginationInfo } from "@/hooks/use-pagination"
import { cn } from "@/lib/utils"

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

/** The Services tab: curated featured catalogue, then the official MCP
 * Registry (registry.modelcontextprotocol.io) loaded on open and paged in.
 * A registry failure never takes the featured list down with it. */
export function ServicesMarketTab({ query }: { query: string }) {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  const [installing, setInstalling] = useState<MarketServiceItem | null>(null)
  const pagination = usePagination(20)

  const normalizedQuery = useDebounced(query.trim().toLowerCase(), 500)

  useEffect(() => {
    pagination.resetPage()
  }, [normalizedQuery])

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
  const registryTotal = registryQuery.data?.pages[0]?.total ?? 0
  const shownRegistryItems = useMemo(
    () =>
      registryItems.filter((item) => !featuredItems.some((featured) => featured.id === item.id)),
    [registryItems, featuredItems],
  )

  // Featured first, then registry — backend already drops featured keys from registry.
  const allItems = useMemo(
    () => [...featuredItems, ...shownRegistryItems],
    [featuredItems, shownRegistryItems],
  )
  // Once the registry answers, total = featured + registry (deduped server-side).
  const listTotal =
    registryQuery.isSuccess && !registryFailed
      ? featuredItems.length + registryTotal
      : allItems.length

  const paginatedItems = useMemo(() => {
    const start = pagination.offset
    const end = start + pagination.pageSize
    return allItems.slice(start, end)
  }, [allItems, pagination.offset, pagination.pageSize])

  const pagInfo = getPaginationInfo(
    pagination.offset,
    paginatedItems.length,
    listTotal,
    pagination.pageSize,
  )

  const hasResults = allItems.length > 0
  const registryFetching = registryQuery.isFetching
  const nextDisabled =
    pagination.page + 1 >= pagInfo.pageCount &&
    !registryQuery.hasNextPage &&
    !registryQuery.isFetchingNextPage

  async function handleNextPage() {
    const nextOffset = (pagination.page + 1) * pagination.pageSize
    if (nextOffset >= allItems.length && registryQuery.hasNextPage) {
      await registryQuery.fetchNextPage()
    }
    pagination.nextPage()
  }

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
    <>
      <ScrollPane className="min-h-0 flex-1" innerClassName="px-4 py-2">
        {paginatedItems.length > 0 ? (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(min(100%,16rem),16rem))] gap-4">
            {paginatedItems.map((item) => {
              const isFeatured = featuredItems.some((featured) => featured.id === item.id)
              return (
                <ServiceMarketCard
                  key={isFeatured ? `featured:${item.id}` : item.id}
                  busy={installingBusy}
                  installed={installedNames.has(item.name.toLowerCase())}
                  item={item}
                  onInstall={() => setInstalling(item)}
                />
              )
            })}
          </div>
        ) : registryFailed ? (
          <p className="mb-3 flex items-center gap-2 text-xs text-muted-foreground">
            <RefreshCwIcon className="size-3.5" aria-hidden />
            {t("serviceMarketRegistryFailed")}
          </p>
        ) : !registryFetching ? (
          <p className="text-xs text-muted-foreground">{t("serviceMarketNoResults")}</p>
        ) : null}

        {registryFetching ? (
          <p className="mt-3 flex items-center gap-2 text-xs text-muted-foreground">
            <Spinner className="size-3.5" />
            {t("serviceMarketLoading")}
          </p>
        ) : null}
        {!registryFetching &&
        !registryQuery.hasNextPage &&
        registryItems.length > 0 &&
        !registryFailed ? (
          <p className="mt-3 text-xs text-muted-foreground">{t("serviceMarketNoMore")}</p>
        ) : null}

        {!hasResults && !registryFetching && !registryFailed ? (
          <p className="py-8 text-center text-sm text-muted-foreground">
            {t("serviceMarketEmpty")}
          </p>
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

      {listTotal > 0 && (
        <PaginationControls
          from={pagInfo.from}
          to={pagInfo.to}
          total={listTotal}
          pageSize={pagination.pageSize}
          pageSizes={pagination.pageSizes}
          page={pagination.page}
          pageCount={pagInfo.pageCount}
          nextDisabled={nextDisabled}
          onPageSizeChange={pagination.changePageSize}
          onPrevPage={pagination.prevPage}
          onNextPage={() => void handleNextPage()}
        />
      )}
    </>
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
  // Market / MCP protocol icon when present; otherwise the panel's default MCP mark.
  const icon = item.icon?.trim() && !iconBroken ? item.icon.trim() : null

  const statusText = installed
    ? t("clientPluginInstalled")
    : item.publisher?.trim() || item.id

  return (
    <BasicCard
      icon={
        icon ? (
          <img
            src={icon}
            alt=""
            className={cn(basicCardIconClassName, "object-contain")}
            aria-hidden
            loading="lazy"
            onError={() => setIconBroken(true)}
          />
        ) : (
          <DefaultServiceIcon className={basicCardIconClassName} />
        )
      }
      title={item.title}
      subtitle={
        <>
          <span
            className={cn(
              "size-1.5 shrink-0 rounded-full",
              installed ? "bg-emerald-500" : "bg-muted-foreground/40",
            )}
          />
          <span className="truncate">{statusText}</span>
        </>
      }
      action={
        installed ? (
          <Button size="sm" variant="outline" className="shrink-0" disabled>
            {t("clientPluginInstalled")}
          </Button>
        ) : (
          <Button size="sm" className="shrink-0" disabled={busy} onClick={onInstall}>
            {t("clientPluginInstall")}
          </Button>
        )
      }
      body={<p className="line-clamp-2">{item.description?.trim() || t("noDescription")}</p>}
      footer={
        <div className="flex min-w-0 flex-wrap items-center gap-1.5">
          {item.matched ? (
            <Badge variant="outline" className="text-[10px]">
              <GlobeIcon aria-hidden />
              {t("serviceMarketAddressMatch")}
            </Badge>
          ) : null}
          {item.sign_in ? <span>{t("serviceMarketSignIn")}</span> : null}
          <span className="font-mono tabular-nums">{item.transport}</span>
        </div>
      }
    />
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
