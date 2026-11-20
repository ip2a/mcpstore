import { useEffect, useMemo, useRef, useState } from "react"

import { useQueryClient } from "@tanstack/react-query"
import { ChevronRightIcon, DownloadIcon, PlusIcon, RotateCwIcon } from "lucide-react"
import { useTheme } from "next-themes"
import { toast } from "sonner"

import { AsciiOceanBackground } from "@/components/shared/ascii-ocean-background"
import { FocusThemeSwitch } from "@/components/shared/focus-switch"
import { PanelCard } from "@/components/shared/panel-card"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible"
import {
  WORKSPACE_TABS_CLASSNAME,
  WORKSPACE_TOOLBAR_CLASSNAME,
} from "@/components/shared/workspace-toolbar"
import { WorkspaceTabsList } from "@/components/shared/workspace-tabs-list"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { PageError, PageSkeleton } from "@/components/shared/page-states"
import { McpstoreAsciiBanner } from "@/components/shared/mcpstore-ascii-banner"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Tabs, TabsContent, TabsTrigger } from "@/components/ui/tabs"
import { SettingsRow, SettingsSection } from "@/features/settings/settings-section"
import { Label } from "@/components/ui/label"
import { InputGroup, InputGroupInput, InputGroupTextarea } from "@/components/ui/input-group"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"
import { DiagnosticsSection } from "@/features/settings/diagnostics-section"
import {
  hostsToList,
  payloadFromDraft,
  sections,
  settingsDraft,
  type SectionId,
  type SettingsDraft,
} from "@/features/settings/model"
import {
  formatReleaseVersion,
  isDesktopShell,
  ResetWindowSizeButton,
  useAppUpdater,
  useAppVersionLabel,
  type UpdaterPhase,
} from "@/features/settings/updater"
import { useSettingsMetaQuery, useUpdateSettingsMutation } from "@/features/settings/queries"
import { useKvHealthQuery } from "@/features/kv/queries"
import { getHostStatus, type HostStatus as ApiHostStatus, type UiLanguage } from "@/lib/api"
import { syncApiBaseFromHosts } from "@/lib/api/backend"
import { validateHostsConfig } from "@/lib/api/hosts"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"
import { queryKeys } from "@/lib/query-keys"
import { useUiStore } from "@/stores/ui-store"

const compactInputGroupClass = "w-20 shrink-0"

export function SettingsView() {
  const { setLanguageOverride, t } = useI18n()
  const queryClient = useQueryClient()
  const [activeTab, setActiveTab] = useState<SectionId>("general")
  const [draft, setDraft] = useState<SettingsDraft | null>(null)
  const [configOpen, setConfigOpen] = useState(false)
  const metaQuery = useSettingsMetaQuery(true)
  const settingsMutation = useUpdateSettingsMutation()
  const {
    phase,
    progress,
    version: updateAvailable,
    checkForUpdate,
    restartToUpdate,
  } = useAppUpdater()
  const appVersion = useAppVersionLabel()
  const meta = metaQuery.data
  const loading = metaQuery.isFetching && !draft
  const error = metaQuery.error
  const errorMessage =
    error instanceof Error ? error.message : error ? String(error) : t("settingsUnavailable")
  const saveTimerRef = useRef<number | null>(null)
  const draftRef = useRef<SettingsDraft | null>(null)
  const persistInFlightRef = useRef(false)
  const persistAgainRef = useRef(false)

  const configFile = meta?.config_file
  const configContent = useMemo(() => configFile?.content || "", [configFile?.content])

  const headerLine = useMemo(() => {
    if (!draft) return t("settingsDescription")
    const parts = [draft.hosts.active]
    if (configFile?.path) parts.push(configFile.path)
    return parts.join(" · ")
  }, [configFile?.path, draft, t])

  useEffect(() => {
    const settings = meta?.settings
    if (!settings) return
    setDraft((current) => {
      if (current) return current
      const next = settingsDraft(settings)
      draftRef.current = next
      return next
    })
  }, [meta])

  useEffect(() => {
    return () => {
      if (saveTimerRef.current != null) window.clearTimeout(saveTimerRef.current)
    }
  }, [])

  async function persistDraft() {
    const next = draftRef.current
    if (!next) return
    const validationError = validateHostsConfig(next.hosts)
    if (validationError) {
      toast.error(validationError)
      return
    }
    if (persistInFlightRef.current) {
      persistAgainRef.current = true
      return
    }
    persistInFlightRef.current = true
    persistAgainRef.current = false
    try {
      await settingsMutation.mutateAsync(payloadFromDraft(next))
      setLanguageOverride(next.language)
      syncApiBaseFromHosts(next.hosts)
      await queryClient.invalidateQueries({ queryKey: queryKeys.meta })
    } catch (err) {
      toast.error(err instanceof Error ? err.message : t("saveFailed"))
    } finally {
      persistInFlightRef.current = false
      if (persistAgainRef.current) {
        persistAgainRef.current = false
        void persistDraft()
      }
    }
  }

  function patchDraft(patch: Partial<SettingsDraft>) {
    setDraft((current) => {
      if (!current) return current
      const next = { ...current, ...patch }
      draftRef.current = next
      if (saveTimerRef.current != null) window.clearTimeout(saveTimerRef.current)
      saveTimerRef.current = window.setTimeout(() => {
        saveTimerRef.current = null
        void persistDraft()
      }, 400)
      return next
    })
  }

  const stateGuard =
    error && !draft ? (
      <PageError
        title={t("settingsUnavailable")}
        message={errorMessage}
        onRefresh={() => void metaQuery.refetch()}
      />
    ) : loading && !draft ? (
      <PageSkeleton />
    ) : null

  return (
    <>
      <div className="min-w-0 shrink-0 pl-4">
        <h1 className="text-lg font-semibold">{t("settings")}</h1>
        <p className="mt-1 truncate font-mono text-sm text-muted-foreground" title={headerLine}>
          {headerLine}
        </p>
      </div>

      <PanelCard className="@container flex h-full min-h-0 flex-1 flex-col gap-0 overflow-hidden p-0">
        {stateGuard ? (
          <div className="p-4">{stateGuard}</div>
        ) : draft ? (
          <Tabs
            value={activeTab}
            onValueChange={(value) => setActiveTab(value as SectionId)}
            className={WORKSPACE_TABS_CLASSNAME}
          >
            <div className={WORKSPACE_TOOLBAR_CLASSNAME}>
              <WorkspaceTabsList value={activeTab} className="gap-1">
                {sections.map((item) => {
                  const label = t(item.labelKey)
                  return (
                    <TabsTrigger key={item.id} value={item.id} title={label} className="flex-none">
                      {label}
                    </TabsTrigger>
                  )
                })}
              </WorkspaceTabsList>
            </div>

            <TabsContent
              value="general"
              className="mt-0 flex min-h-0 flex-1 flex-col overflow-hidden"
            >
              <ScrollPane className="flex-1" innerClassName="px-4 py-4">
                <section className="flex flex-col gap-6">
                  <SettingsSection title={t("language")}>
                    <SettingsRow title={t("language")} description={t("chooseLanguage")}>
                      <Select
                        value={draft.language}
                        onValueChange={(value) => {
                          const language = value as UiLanguage
                          setLanguageOverride(language)
                          patchDraft({ language })
                        }}
                      >
                        <SelectTrigger className="w-44">
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectGroup>
                            <SelectItem value="auto">{t("auto")}</SelectItem>
                            <SelectItem value="zh">{t("chinese")}</SelectItem>
                            <SelectItem value="en">{t("english")}</SelectItem>
                          </SelectGroup>
                        </SelectContent>
                      </Select>
                    </SettingsRow>
                    <ThemeSettingsRow />
                    {isDesktopShell ? (
                      <SettingsRow
                        title={t("resetDefaultWindowSize")}
                        description={t("resetDefaultWindowSizeDescription")}
                      >
                        <ResetWindowSizeButton />
                      </SettingsRow>
                    ) : null}
                  </SettingsSection>

                  <SettingsSection title={t("serviceTabsPrefsLabel")}>
                    <SettingsRow
                      title={t("serviceTabsSort")}
                      description={t("serviceTabsSortDescription")}
                    >
                      <Select
                        value={draft.serviceTabs.sort}
                        onValueChange={(value) =>
                          patchDraft({
                            serviceTabs: {
                              ...draft.serviceTabs,
                              sort: value as SettingsDraft["serviceTabs"]["sort"],
                            },
                          })
                        }
                      >
                        <SelectTrigger className="w-56">
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectGroup>
                            <SelectItem value="name">{t("serviceTabsSortName")}</SelectItem>
                            <SelectItem value="added_newest">
                              {t("serviceTabsSortAddedNewest")}
                            </SelectItem>
                            <SelectItem value="added_oldest">
                              {t("serviceTabsSortAddedOldest")}
                            </SelectItem>
                          </SelectGroup>
                        </SelectContent>
                      </Select>
                    </SettingsRow>
                    <ToggleField
                      title={t("serviceTabsShowDisconnected")}
                      description={t("serviceTabsShowDisconnectedDescription")}
                      checked={draft.serviceTabs.showDisconnected}
                      onCheckedChange={(showDisconnected) =>
                        patchDraft({
                          serviceTabs: { ...draft.serviceTabs, showDisconnected },
                        })
                      }
                    />
                  </SettingsSection>

                  <SettingsSection title={t("configFile")}>
                    <SettingsRow
                      title={t("settingsConfigExists")}
                      description={configFile?.path || t("configFileMissing")}
                    >
                      <Button size="sm" variant="outline" onClick={() => setConfigOpen(true)}>
                        {t("view")}
                      </Button>
                    </SettingsRow>
                  </SettingsSection>
                </section>
              </ScrollPane>
            </TabsContent>

            <TabsContent
              value="connection"
              className="mt-0 flex min-h-0 flex-1 flex-col overflow-hidden"
            >
              <ScrollPane className="flex-1" innerClassName="px-4 py-4">
                <section className="flex flex-col gap-6">
                  <SettingsSection
                    title={t("coreBackendStartupTitle")}
                  >
                    <ListenPortFields draft={draft} patchDraft={patchDraft} />
                    <PanelAuthField draft={draft} patchDraft={patchDraft} />
                  </SettingsSection>
                  <ConnectionSection draft={draft} patchDraft={patchDraft} />
                </section>
              </ScrollPane>
            </TabsContent>

            <TabsContent
              value="diagnostics"
              className="mt-0 flex min-h-0 flex-1 flex-col overflow-hidden"
            >
              <ScrollPane className="flex-1" innerClassName="px-4 py-2">
                <DiagnosticsSection
                  value={draft.diagnostics}
                  onChange={(diagnostics) => patchDraft({ diagnostics })}
                />
              </ScrollPane>
            </TabsContent>

            {/*
                Advanced: KV switch + placeholder for a future standalone
                "mcpstore management MCP" (not a gateway feature).
              */}
            <TabsContent
              value="advanced"
              className="mt-0 flex min-h-0 flex-1 flex-col overflow-hidden"
            >
              <ScrollPane className="flex-1" innerClassName="px-4 py-4">
                <section className="flex flex-col gap-6">
                  <SettingsSection title="KvStore">
                    <KvSettingsSection />
                  </SettingsSection>

                  <SettingsSection title={t("managementMcp")}>
                    <ToggleField
                      title={t("managementMcpEnabled")}
                      description={t("managementMcpEnabledDescription")}
                      checked={false}
                      disabled
                      onCheckedChange={() => {}}
                    />
                  </SettingsSection>
                </section>
              </ScrollPane>
            </TabsContent>

            <TabsContent
              value="about"
              className="relative mt-0 flex min-h-0 flex-1 flex-col overflow-hidden bg-transparent"
            >
              <AsciiOceanBackground />
              <ScrollPane
                className="relative z-10 min-h-0 flex-1 bg-transparent"
                innerClassName="px-4 py-2"
              >
                <section className="flex flex-col gap-5">
                  <McpstoreAsciiBanner />
                  <div className="flex flex-col gap-2">
                    <div className="px-0.5">
                      <h3 className="text-sm font-medium text-foreground">
                        {t("aboutMcpstore")}
                      </h3>
                    </div>
                    <AboutVersionCard
                      version={appVersion}
                      phase={phase}
                      progress={progress}
                      updateAvailable={updateAvailable}
                      onCheck={() => void checkForUpdate()}
                      onRestart={restartToUpdate}
                    />
                  </div>
                </section>
              </ScrollPane>
              <div className="relative z-10 flex shrink-0 flex-col items-center gap-2 px-4 pb-3 pt-1">
                <p className="text-sm text-muted-foreground">
                  {t("poweredBy")}{" "}
                  <a
                    href="https://crates.io/crates/mcpstore"
                    target="_blank"
                    rel="noreferrer"
                    className="font-medium text-blue-600 hover:underline dark:text-blue-400"
                  >
                    mcpstore
                  </a>
                </p>
                <AboutSocialLinks />
              </div>
            </TabsContent>
          </Tabs>
        ) : (
          <div className="p-4">
            <PageSkeleton />
          </div>
        )}
      </PanelCard>

      <Dialog open={configOpen} onOpenChange={setConfigOpen}>
        <DialogContent className="sm:max-w-3xl">
          <DialogHeader>
            <DialogTitle>{t("configFile")}</DialogTitle>
            <DialogDescription className="font-mono text-xs">
              {configFile?.path || t("configFileMissing")}
            </DialogDescription>
          </DialogHeader>
          <InputGroup>
            <InputGroupTextarea
              className="min-h-80 max-h-[60vh] font-mono text-xs"
              readOnly
              value={configContent}
            />
          </InputGroup>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => setConfigOpen(false)}>
              {t("close")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  )
}

function KvSettingsSection() {
  const { t } = useI18n()
  const setKvDialogOpen = useUiStore((state) => state.setKvDialogOpen)
  const healthQuery = useKvHealthQuery()
  const storeName =
    healthQuery.data &&
    typeof healthQuery.data === "object" &&
    healthQuery.data !== null &&
    "store" in healthQuery.data &&
    typeof (healthQuery.data as { store?: unknown }).store === "string"
      ? (healthQuery.data as { store: string }).store
      : t("unknown")

  return (
    <SettingsRow title={t("switchKvStorage")} description={t("currentKvStorage", { store: storeName })}>
      <Button size="sm" variant="outline" onClick={() => setKvDialogOpen(true)}>
        {t("switch")}
      </Button>
    </SettingsRow>
  )
}

function ThemeSettingsRow() {
  const { t } = useI18n()
  const { theme, setTheme, resolvedTheme } = useTheme()
  const [mounted, setMounted] = useState(false)

  useEffect(() => {
    setMounted(true)
  }, [])

  const dark =
    theme === "dark" || (theme !== "light" && resolvedTheme === "dark")

  return (
    <SettingsRow title={t("theme")} description={t("chooseTheme")}>
      {mounted ? (
        <FocusThemeSwitch
          checked={dark}
          onCheckedChange={(checked) => setTheme(checked ? "dark" : "light")}
          label={t("theme")}
        />
      ) : (
        <div className="h-9 w-[calc(3rem*1.6439)]" aria-hidden />
      )}
    </SettingsRow>
  )
}

function ToggleField({
  title,
  description,
  checked,
  disabled,
  onCheckedChange,
}: {
  title: string
  description?: string
  checked: boolean
  disabled?: boolean
  onCheckedChange: (checked: boolean) => void
}) {
  return (
    <SettingsRow title={title} description={description}>
      <Switch
        checked={checked}
        disabled={disabled}
        onCheckedChange={onCheckedChange}
        aria-label={title}
      />
    </SettingsRow>
  )
}

function AboutVersionCard({
  version,
  phase,
  progress,
  updateAvailable,
  onCheck,
  onRestart,
}: {
  version: string | null
  phase: UpdaterPhase
  progress: number
  updateAvailable: string | null
  onCheck: () => void
  onRestart: () => void
}) {
  const { t } = useI18n()
  const busy = phase === "checking" || phase === "downloading"
  const ready = phase === "ready"
  const latest = phase === "latest"

  const statusText =
    phase === "checking"
      ? t("checkingForUpdates")
      : phase === "downloading"
        ? t("downloadingUpdate", { progress: `${progress}%` })
        : ready
          ? t("updateReadyNotice")
          : latest
            ? t("upToDate")
            : updateAvailable
              ? t("updateAvailable", { version: formatReleaseVersion(updateAvailable) })
              : null

  const buttonLabel = ready
    ? t("updateNow")
    : phase === "downloading"
      ? t("downloadingUpdate", { progress: `${progress}%` })
      : phase === "checking"
        ? t("checkingForUpdates")
        : latest
          ? t("updateLatest")
          : t("checkUpdates")

  return (
    <div className="flex items-center justify-between gap-4 rounded-xl border border-border/60 bg-card px-8 py-3.5 shadow-xs">
      <div className="flex min-w-0 items-center gap-4">
        <img
          src="/logo.png"
          alt=""
          className="size-10 shrink-0 rounded-lg"
          draggable={false}
        />
        <div className="min-w-0 space-y-1">
          <div className="truncate text-sm font-semibold text-foreground">mcpstore</div>
          <p className="truncate text-xs text-muted-foreground">
            {t("currentVersion", { version: formatReleaseVersion(version) })}
          </p>
        </div>
      </div>
      <div className="flex shrink-0 items-center gap-3">
        {statusText ? (
          <span className="hidden text-xs text-muted-foreground sm:inline">{statusText}</span>
        ) : null}
        <Button
          type="button"
          size="sm"
          className="rounded-md bg-foreground px-4 text-background hover:bg-foreground/90"
          disabled={busy}
          onClick={ready ? onRestart : onCheck}
        >
          {busy ? <RotateCwIcon data-icon="inline-start" className="animate-spin" /> : null}
          {buttonLabel}
        </Button>
      </div>
    </div>
  )
}

const ABOUT_SOCIAL_LINKS = [
  {
    id: "github",
    href: "https://github.com/ip2a/mcpstore",
    label: "github.com/ip2a/mcpstore",
    icon: (
      <svg viewBox="0 0 24 24" aria-hidden="true" className="size-6 shrink-0 fill-[#333] dark:fill-foreground">
        <path d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z" />
      </svg>
    ),
  },
  {
    id: "rust",
    href: "https://crates.io/crates/mcpstore",
    label: "crates.io/crates/mcpstore",
    icon: (
      <svg viewBox="0 0 24 24" aria-hidden="true" className="size-6 shrink-0 fill-[#dea584]">
        <path d="M12.9 2.1 21 6.7v10.6l-8.1 4.6L4.8 17.3V6.7l8.1-4.6zm0 1.8L6.6 7.5v8.8l6.3 3.6 6.3-3.6V7.5l-6.3-3.6zm0 2.3 3.9 2.2v1.3l-3.9 2.2-3.9-2.2V8.4l3.9-2.2zm-3.2 4.4 3.2 1.8v3.6l-3.2-1.8v-3.6zm6.4 0v3.6l3.2 1.8v-3.6l-3.2-1.8z" />
      </svg>
    ),
  },
  {
    id: "python",
    href: "https://pypi.org/project/mcpstore/",
    label: "pypi.org/project/mcpstore",
    icon: (
      <svg viewBox="0 0 24 24" aria-hidden="true" className="size-6 shrink-0 fill-[#3776ab]">
        <path d="M12 0C5.83 0 5.39.22 5.39 5.02v1.84h6.73v.55H3.52C1.03 7.41 0 8.84 0 12.02c0 3.26 1.1 4.63 3.93 4.63h2.25V14.4c0-2.61 2.25-4.89 4.89-4.89h5.94c2.17 0 3.93-1.8 3.93-3.97V5.02C20.94.2 19.77 0 12 0zm-3.3 2.18c.68 0 1.23.55 1.23 1.23S9.38 4.64 8.7 4.64s-1.23-.55-1.23-1.23.55-1.23 1.23-1.23zM20.07 7.97v2.11c0 2.72-2.31 5.05-5.05 5.05H9.08c-2.17 0-3.93 1.85-3.93 4.02v2.83C5.15 23.8 6.72 24 12 24c6.2 0 6.61-.23 6.61-5.02v-1.84h-6.67v-.55h8.54c2.49 0 3.52-1.37 3.52-4.55 0-3.26-1.1-4.63-3.93-4.63zm-3.34 12.54c.68 0 1.23.55 1.23 1.23s-.55 1.23-1.23 1.23-1.23-.55-1.23-1.23.55-1.23 1.23-1.23z" />
      </svg>
    ),
  },
  {
    id: "npm",
    href: "https://www.npmjs.com/package/@ip2a/mcpstore",
    label: "@ip2a/mcpstore",
    icon: (
      <svg viewBox="0 0 24 24" aria-hidden="true" className="size-6 shrink-0 fill-[#cb3837]">
        <path d="M0 7.334v8h6.666v1.332H12v-1.332h12v-8zm6.666 6.664H5.334v-4H3.999v4H1.335V8.667h5.331zm4 0v1.336H8.001V8.667h5.334v5.331zm12 0h-1.33v-4h-1.336v4h-1.335v-4h-1.33v4h-2.671V8.667h8.002zM10.665 10H12v2.667h-1.335z" />
      </svg>
    ),
  },
] as const

function AboutSocialLinks() {
  return (
    <div className="flex items-center justify-center gap-1.5">
      {ABOUT_SOCIAL_LINKS.map((link) => (
        <a
          key={link.id}
          href={link.href}
          target="_blank"
          rel="noreferrer"
          title={link.label}
          className="group flex h-12 w-12 items-center justify-center overflow-hidden rounded-[10px] bg-transparent text-foreground transition-all duration-300 hover:w-[18rem] hover:rounded-md"
        >
          {link.icon}
          <span className="ml-0 w-0 overflow-hidden text-center text-sm font-medium whitespace-nowrap opacity-0 transition-all duration-300 group-hover:ml-1.5 group-hover:w-48 group-hover:opacity-100">
            {link.label}
          </span>
        </a>
      ))}
    </div>
  )
}

function ListenPortFields({
  draft,
  patchDraft,
}: {
  draft: SettingsDraft
  patchDraft: (patch: Partial<SettingsDraft>) => void
}) {
  const { t } = useI18n()
  return (
    <>
      <SettingsRow
        title={t("defaultBackendPort")}
      >
        <InputGroup className={compactInputGroupClass}>
          <InputGroupInput
            inputMode="numeric"
            value={draft.api.port}
            onChange={(event) =>
              patchDraft({
                api: { ...draft.api, port: Math.max(1, Number(event.target.value || 1)) },
              })
            }
          />
        </InputGroup>
      </SettingsRow>
      <SettingsRow
        title={t("defaultFrontendPort")}
      >
        <InputGroup className={compactInputGroupClass}>
          <InputGroupInput
            inputMode="numeric"
            value={draft.web.port}
            onChange={(event) =>
              patchDraft({
                web: { ...draft.web, port: Math.max(1, Number(event.target.value || 1)) },
              })
            }
          />
        </InputGroup>
      </SettingsRow>
    </>
  )
}

function PanelAuthField({
  draft,
  patchDraft,
}: {
  draft: SettingsDraft
  patchDraft: (patch: Partial<SettingsDraft>) => void
}) {
  const { t } = useI18n()
  return (
    <SettingsRow title={t("panelAuthSetting")}>
      <Switch
        checked={draft.web.auth}
        onCheckedChange={(checked) => patchDraft({ web: { ...draft.web, auth: checked } })}
      />
    </SettingsRow>
  )
}

function formatLatency(ms: number): string {
  if (ms < 1) return `${Math.max(1, Math.round(ms * 1000))}μs`
  return `${Math.round(ms)}ms`
}

type HostStatus = {
  online: boolean
  latencyMs: number | null
  version: string | null
  store: string | null
}

function toHostStatus(status: ApiHostStatus): HostStatus {
  return {
    online: status.online,
    latencyMs: status.latency_ms,
    version: status.version,
    store: status.store,
  }
}

async function probeHost(name: string): Promise<HostStatus> {
  try {
    return toHostStatus(await getHostStatus(name))
  } catch {
    return { online: false, latencyMs: null, version: null, store: null }
  }
}

function formatHostAddress(url: string): string {
  try {
    const parsed =
      url.startsWith("http://") || url.startsWith("https://")
        ? new URL(url)
        : new URL(url, window.location.origin)
    return parsed.port ? `${parsed.hostname}:${parsed.port}` : parsed.hostname
  } catch {
    return url
  }
}

function composeConnectionUrl(host: string, port: string, secure: boolean, path: string): string {
  const trimmedHost = host.trim()
  if (!trimmedHost) return ""
  const trimmedPort = port.trim()
  let trimmedPath = path.trim()
  if (trimmedPath && !trimmedPath.startsWith("/")) trimmedPath = `/${trimmedPath}`
  return `${secure ? "https" : "http"}://${trimmedHost}${trimmedPort ? `:${trimmedPort}` : ""}${trimmedPath}`
}

function formatFullUrlBody(host: string, port: string, path: string): string {
  const trimmedHost = host.trim()
  if (!trimmedHost) return ""
  const trimmedPort = port.trim()
  let trimmedPath = path.trim()
  if (trimmedPath && !trimmedPath.startsWith("/")) trimmedPath = `/${trimmedPath}`
  return `${trimmedHost}${trimmedPort ? `:${trimmedPort}` : ""}${trimmedPath}`
}

function composeFullUrlFromBody(body: string, secure: boolean): string {
  const trimmed = body.trim().replace(/^https?:\/\//i, "")
  if (!trimmed) return ""
  return `${secure ? "https" : "http"}://${trimmed}`
}

function parseFullUrlBodyInput(value: string): { body: string; secure?: boolean } {
  const trimmed = value.trim()
  const match = trimmed.match(/^(https?):\/\/(.*)$/i)
  if (match) {
    return { body: match[2], secure: match[1].toLowerCase() === "https" }
  }
  return { body: trimmed.replace(/^https?:\/\//i, "") }
}

function parseConnectionUrl(
  url: string,
): { host: string; port: string; secure: boolean; path: string } | null {
  const trimmed = url.trim()
  if (!trimmed) return null
  try {
    const withScheme = /^https?:\/\//i.test(trimmed) ? trimmed : `http://${trimmed}`
    const parsed = new URL(withScheme)
    if (!parsed.hostname) return null
    return {
      host: parsed.hostname,
      port: parsed.port,
      secure: parsed.protocol === "https:",
      path: parsed.pathname === "/" ? "" : `${parsed.pathname}${parsed.search}`,
    }
  } catch {
    return null
  }
}

function ConnectionSection({
  draft,
  patchDraft,
}: {
  draft: SettingsDraft
  patchDraft: (patch: Partial<SettingsDraft>) => void
}) {
  const { t } = useI18n()
  const hosts = useMemo(() => hostsToList(draft.hosts), [draft.hosts])
  const [addingOpen, setAddingOpen] = useState(false)
  const [pendingName, setPendingName] = useState("")
  const [pendingHost, setPendingHost] = useState("")
  const [pendingPort, setPendingPort] = useState("")
  const [pendingSecure, setPendingSecure] = useState(false)
  const [pendingPath, setPendingPath] = useState("")
  const [pendingFullUrlMode, setPendingFullUrlMode] = useState(false)
  const [pendingFullUrlBody, setPendingFullUrlBody] = useState("")
  const [editingName, setEditingName] = useState<string | null>(null)
  const [editName, setEditName] = useState("")
  const [editUrls, setEditUrls] = useState("")
  const [statuses, setStatuses] = useState<Record<string, HostStatus>>({})
  const [checking, setChecking] = useState<Record<string, boolean>>({})
  const [hostsOpen, setHostsOpen] = useState(false)

  function patchHosts(
    nextEntries: Record<string, { urls: string[] }>,
    active = draft.hosts.active,
  ) {
    patchDraft({ hosts: { active, entries: nextEntries } })
  }

  async function refreshLatency(name: string, options?: { notify?: boolean }) {
    setChecking((current) => ({ ...current, [name]: true }))
    const status = await probeHost(name)
    setStatuses((current) => ({ ...current, [name]: status }))
    setChecking((current) => ({ ...current, [name]: false }))
    if (options?.notify) {
      if (status.online && status.latencyMs != null) {
        toast.success(t("connectionRefreshOk", { latency: formatLatency(status.latencyMs) }))
      } else {
        toast.error(t("connectionRefreshFailed"))
      }
    }
  }

  useEffect(() => {
    let cancelled = false

    async function measureAll() {
      await Promise.all(
        hosts.map(async (host) => {
          if (cancelled) return
          await refreshLatency(host.name)
        }),
      )
    }

    void measureAll()
    const timer = window.setInterval(() => void measureAll(), 5000)
    return () => {
      cancelled = true
      window.clearInterval(timer)
    }
  }, [hosts])

  const pendingUrl = composeConnectionUrl(pendingHost, pendingPort, pendingSecure, pendingPath)
  const pendingFullUrl = composeFullUrlFromBody(pendingFullUrlBody, pendingSecure)
  const parsedRawUrl = pendingFullUrlMode ? parseConnectionUrl(pendingFullUrl) : null
  const pendingUrlValue = pendingFullUrlMode ? pendingFullUrl : pendingUrl
  const pendingValid =
    pendingName.trim() !== "" &&
    !Object.hasOwn(draft.hosts.entries, pendingName.trim()) &&
    (pendingFullUrlMode ? parsedRawUrl != null : pendingUrl !== "")

  function resetPending() {
    setPendingName("")
    setPendingHost("")
    setPendingPort("")
    setPendingSecure(false)
    setPendingPath("")
    setPendingFullUrlMode(false)
    setPendingFullUrlBody("")
  }

  function handleAddDialogOpenChange(open: boolean) {
    if (!open) resetPending()
    setAddingOpen(open)
  }

  function addHost() {
    const name = pendingName.trim()
    const url = pendingUrlValue.trim()
    if (!pendingValid || !name || !url) return
    patchHosts({
      ...draft.hosts.entries,
      [name]: { urls: [url] },
    })
    resetPending()
    setAddingOpen(false)
  }

  function setFullUrlMode(enabled: boolean) {
    if (!enabled) {
      const parsed = parseConnectionUrl(pendingFullUrl)
      if (parsed) {
        setPendingHost(parsed.host)
        setPendingPort(parsed.port)
        setPendingSecure(parsed.secure)
        setPendingPath(parsed.path)
      }
      setPendingFullUrlMode(false)
      return
    }
    const parsed = parseConnectionUrl(pendingUrl)
    setPendingFullUrlBody(parsed ? formatFullUrlBody(parsed.host, parsed.port, parsed.path) : "")
    if (parsed) setPendingSecure(parsed.secure)
    setPendingFullUrlMode(true)
  }

  /** One URL per line; order is the failover preference. */
  function parseEditUrls(): string[] {
    return editUrls
      .split("\n")
      .map((url) => url.trim())
      .filter((url) => url !== "")
  }

  function saveEdit() {
    if (!editingName) return
    const name = editName.trim()
    const urls = parseEditUrls()
    if (!name || urls.length === 0) return
    if (name !== editingName && Object.hasOwn(draft.hosts.entries, name)) {
      toast.error(t("saveFailed"))
      return
    }
    const entries = { ...draft.hosts.entries }
    delete entries[editingName]
    entries[name] = { urls }
    const active = draft.hosts.active === editingName ? name : draft.hosts.active
    patchHosts(entries, active)
    setEditingName(null)
    setEditName("")
    setEditUrls("")
  }

  function removeHost(name: string) {
    if (hosts.length <= 1) return
    const entries = { ...draft.hosts.entries }
    delete entries[name]
    const remaining = Object.keys(entries)
    const active = draft.hosts.active === name ? remaining[0]! : draft.hosts.active
    patchHosts(entries, active)
    if (editingName === name) {
      setEditingName(null)
      setEditName("")
      setEditUrls("")
    }
    toast.success(t("connectionDeleted"))
  }

  function toggleEdit(host: { name: string; urls: string[] }) {
    if (editingName === host.name) {
      setEditingName(null)
      setEditName("")
      setEditUrls("")
      return
    }
    setEditingName(host.name)
    setEditName(host.name)
    setEditUrls(host.urls.join("\n"))
    setAddingOpen(false)
  }

  const editingHost = editingName ? hosts.find((host) => host.name === editingName) : null

  function openAddDialog() {
    setEditingName(null)
    setEditName("")
    setEditUrls("")
    resetPending()
    setAddingOpen(true)
  }

  return (
    <>
      <SettingsSection title={t("connection")}>
        {hosts.map((host) => {
          const isEditing = editingName === host.name
          const canDelete = hosts.length > 1
          const isChecking = checking[host.name] === true
          const status = statuses[host.name]
          const online = status?.online === true
          const address = formatHostAddress(host.urls[0] ?? "")
          const latencyLabel =
            online && status?.latencyMs != null
              ? formatLatency(status.latencyMs)
              : isChecking
                ? "…"
                : status == null
                  ? "…"
                  : "—"
          return (
            <SettingsRow
              key={host.name}
              title={host.name}
              description={address || undefined}
            >
              <div className="flex shrink-0 items-center gap-2">
                <span className="font-mono text-xs tabular-nums text-muted-foreground">
                  {latencyLabel}
                </span>
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  className="h-7 px-2 text-xs"
                  aria-pressed={isEditing}
                  onClick={() => toggleEdit(host)}
                >
                  {t("edit")}
                </Button>
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  className="h-7 px-2 text-xs text-destructive hover:text-destructive"
                  disabled={!canDelete}
                  title={!canDelete ? t("connectionDeleteLastHint") : undefined}
                  onClick={() => removeHost(host.name)}
                >
                  {t("connectionRemove")}
                </Button>
              </div>
            </SettingsRow>
          )
        })}
        <Collapsible open={hostsOpen} onOpenChange={setHostsOpen}>
          <CollapsibleTrigger className="flex w-full items-center gap-2 px-4 py-3.5 text-left text-sm font-medium text-foreground transition-colors hover:bg-muted/40">
            <ChevronRightIcon
              className={cn(
                "size-4 shrink-0 text-muted-foreground transition-transform",
                hostsOpen && "rotate-90",
              )}
            />
            {t("host")}
          </CollapsibleTrigger>
          <CollapsibleContent>
            <button
              type="button"
              className="flex w-full items-center border-t border-border/60 px-4 py-3.5 text-left text-sm font-medium text-foreground transition-colors hover:bg-muted/40"
              onClick={openAddDialog}
            >
              {t("addHost")}
            </button>
          </CollapsibleContent>
        </Collapsible>
      </SettingsSection>

      {editingHost ? (
        <div className="rounded-lg border border-border/50 bg-card p-4">
          <p className="mb-3 text-sm font-medium">{t("edit")}</p>
          <div className="flex flex-col gap-3">
            <div className="flex flex-col gap-1.5">
              <Label htmlFor={`host-name-${editingHost.name}`}>{t("host")}</Label>
              <InputGroup>
                <InputGroupInput
                  id={`host-name-${editingHost.name}`}
                  value={editName}
                  onChange={(event) => setEditName(event.target.value)}
                  placeholder={t("host")}
                />
              </InputGroup>
            </div>
            <div className="flex flex-col gap-3 sm:flex-row sm:items-end">
              <div className="flex min-w-0 flex-1 flex-col gap-1.5">
                <Label htmlFor={`host-url-${editingHost.name}`}>{t("coreBackendUrlLabel")}</Label>
                <InputGroup>
                  <InputGroupTextarea
                    id={`host-url-${editingHost.name}`}
                    className="min-h-20 font-mono text-xs"
                    value={editUrls}
                    onChange={(event) => setEditUrls(event.target.value)}
                    placeholder={t("coreBackendUrlPlaceholder")}
                  />
                </InputGroup>
                <p className="text-xs text-muted-foreground">{t("coreBackendUrlHint")}</p>
              </div>
              <div className="flex gap-2">
                <Button type="button" size="sm" onClick={saveEdit}>
                  {t("save")}
                </Button>
                <Button
                  type="button"
                  size="sm"
                  variant="outline"
                  onClick={() => setEditingName(null)}
                >
                  {t("cancel")}
                </Button>
              </div>
            </div>
          </div>
        </div>
      ) : null}

      <Dialog open={addingOpen} onOpenChange={handleAddDialogOpenChange}>
        <DialogContent className="sm:max-w-lg">
          <DialogHeader>
            <DialogTitle>{t("addHost")}</DialogTitle>
            <DialogDescription>{t("connectionDescription")}</DialogDescription>
          </DialogHeader>

          <div className="flex flex-col gap-4">
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="pending-host-name">{t("host")}</Label>
              <InputGroup>
                <InputGroupInput
                  id="pending-host-name"
                  autoFocus
                  value={pendingName}
                  onChange={(event) => setPendingName(event.target.value)}
                  placeholder={t("host")}
                  className={cn(
                    pendingName.trim() &&
                      Object.hasOwn(draft.hosts.entries, pendingName.trim()) &&
                      "border-destructive/50",
                  )}
                />
              </InputGroup>
            </div>

            {pendingFullUrlMode ? (
              <div className="flex flex-col gap-1.5">
                <Label htmlFor="connection-full-url">{t("fullUrl")}</Label>
                <div className="flex items-center gap-2">
                  <span className="shrink-0 font-mono text-sm text-muted-foreground">
                    {pendingSecure ? "https://" : "http://"}
                  </span>
                  <InputGroup className="min-w-0 flex-1">
                    <InputGroupInput
                      id="connection-full-url"
                      value={pendingFullUrlBody}
                      onChange={(event) => {
                        const next = parseFullUrlBodyInput(event.target.value)
                        setPendingFullUrlBody(next.body)
                        if (next.secure != null) setPendingSecure(next.secure)
                      }}
                      placeholder="127.0.0.1:1820/api"
                      className={cn(
                        pendingFullUrlBody.trim() && !parsedRawUrl && "border-destructive/50",
                      )}
                    />
                  </InputGroup>
                </div>
              </div>
            ) : (
              <div className="flex flex-col gap-4 sm:flex-row">
                <div className="flex min-w-0 flex-1 flex-col gap-1.5">
                  <Label htmlFor="connection-host">{t("coreBackendUrlLabel")}</Label>
                  <div className="flex items-center gap-2">
                    <span className="shrink-0 font-mono text-sm text-muted-foreground">
                      {pendingSecure ? "https://" : "http://"}
                    </span>
                    <InputGroup className="min-w-0 flex-1">
                      <InputGroupInput
                        id="connection-host"
                        value={pendingHost}
                        onChange={(event) => setPendingHost(event.target.value)}
                        placeholder="127.0.0.1"
                      />
                    </InputGroup>
                  </div>
                </div>
                <div className="flex w-full flex-col gap-1.5 sm:w-32">
                  <Label htmlFor="connection-port">{t("port")}</Label>
                  <InputGroup>
                    <InputGroupInput
                      id="connection-port"
                      inputMode="numeric"
                      value={pendingPort}
                      onChange={(event) => setPendingPort(event.target.value.replace(/\D/g, ""))}
                      placeholder="1820"
                    />
                  </InputGroup>
                </div>
              </div>
            )}

            <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
              <div className="flex items-center gap-2 text-sm">
                <Switch
                  checked={pendingSecure}
                  onCheckedChange={setPendingSecure}
                  aria-label={t("useHttps")}
                />
                {t("useHttps")}
              </div>
              <div className="flex items-center gap-2 text-sm">
                <Switch
                  checked={pendingFullUrlMode}
                  onCheckedChange={setFullUrlMode}
                  aria-label={t("useFullUrl")}
                />
                {t("useFullUrl")}
              </div>
            </div>
          </div>

          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => handleAddDialogOpenChange(false)}
            >
              {t("cancel")}
            </Button>
            <Button type="button" disabled={!pendingValid} onClick={addHost}>
              <PlusIcon data-icon="inline-start" />
              {t("addHost")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  )
}
