import { useCallback } from "react"
import { useLocation, useNavigate } from "react-router-dom"

import { type AppView, useViewTitle } from "@/app/app-view"
import { useServiceDetailQuery } from "@/features/services/queries"
import type { ScopeRef, ServiceAddress, ServiceInstance } from "@/lib/api"

const EMPTY_ADDR: ServiceAddress = { service_name: "", scope: { type: "store" } }

function scopeEquals(a: ScopeRef, b: ScopeRef): boolean {
  if (a.type !== b.type) return false
  if (a.type === "agent" && b.type === "agent") return a.agent_id === b.agent_id
  return true
}

function scopeFromSearch(search: string): ScopeRef {
  const params = new URLSearchParams(search)
  if (params.get("scope") === "agent") {
    return { type: "agent", agent_id: params.get("agent_id") || "" }
  }
  return { type: "store" }
}

export function useAppView(services: ServiceInstance[]) {
  const location = useLocation()
  const navigate = useNavigate()
  const view = viewFromPath(location.pathname, location.search)
  const listedService =
    view.name === "instance"
      ? services.find(
          (service) =>
            service.service_name === view.addr.service_name &&
            scopeEquals(service.scope, view.addr.scope),
        )
      : undefined
  const instanceQuery = useServiceDetailQuery(
    view.name === "instance" ? view.addr : EMPTY_ADDR,
    view.name === "instance" && !listedService,
  )
  const selectedService = listedService || instanceQuery.data
  const viewTitle = useViewTitle(view)
  const pageTitle = selectedService?.service_name || viewTitle
  const setView = useCallback((nextView: AppView) => navigate(pathForView(nextView)), [navigate])
  const goBack = useCallback(() => {
    if (view.name === "tool") {
      navigate("/tools", { replace: location.key === "default" })
      return
    }
    if (view.name === "resource") {
      navigate("/resources", { replace: location.key === "default" })
      return
    }
    if (view.name === "prompt") {
      navigate("/prompts", { replace: location.key === "default" })
      return
    }
    if (location.key !== "default") navigate(-1)
    else navigate("/services", { replace: true })
  }, [location.key, navigate, view.name])

  return {
    goBack,
    pageTitle,
    selectedService,
    selectedServiceError: instanceQuery.error,
    selectedServiceLoading: instanceQuery.isFetching,
    refreshSelectedService: instanceQuery.refetch,
    setView,
    view,
  }
}

function viewFromPath(pathname: string, search: string): AppView {
  if (pathname === "/" || pathname === "/home") return { name: "home" }
  if (pathname === "/services") return { name: "services" }
  if (pathname === "/scope") return { name: "scope" }
  if (pathname === "/tools") return { name: "tools" }
  if (pathname === "/resources") return { name: "resources" }
  if (pathname === "/prompts") return { name: "prompts" }
  if (pathname.startsWith("/tools/")) {
    const parsed = parseRegistryDetailPath(pathname.slice("/tools/".length))
    if (parsed) return { name: "tool", instanceId: parsed.instanceId, toolName: parsed.key }
  }
  if (pathname.startsWith("/resources/")) {
    const parsed = parseRegistryDetailPath(pathname.slice("/resources/".length))
    if (parsed) return { name: "resource", instanceId: parsed.instanceId, resourceUri: parsed.key }
  }
  if (pathname.startsWith("/prompts/")) {
    const parsed = parseRegistryDetailPath(pathname.slice("/prompts/".length))
    if (parsed) return { name: "prompt", instanceId: parsed.instanceId, promptName: parsed.key }
  }
  if (pathname === "/clients") return { name: "clients" }
  if (pathname === "/plugins") {
    const tab = new URLSearchParams(search).get("tab")
    if (tab === "installed" || tab === "services" || tab === "agents") {
      return { name: "plugins", tab }
    }
    return { name: "plugins", tab: "installed" }
  }
  if (pathname === "/gateway") return { name: "gateway" }
  // Legacy /history bookmarks land on Traces → History tab.
  if (pathname === "/history") return { name: "traces", tab: "history" }
  if (pathname === "/traces") {
    const tab = new URLSearchParams(search).get("tab")
    if (tab === "history" || tab === "table" || tab === "graph") {
      return { name: "traces", tab }
    }
    return { name: "traces" }
  }
  if (pathname === "/cache") return { name: "cache" }
  if (pathname === "/settings") return { name: "settings" }
  if (pathname.startsWith("/services/")) {
    const raw = pathname.slice("/services/".length)
    let service_name: string
    try {
      service_name = decodeURIComponent(raw)
    } catch {
      service_name = raw
    }
    return { name: "instance", addr: { service_name, scope: scopeFromSearch(search) } }
  }
  return { name: "home" }
}

function parseRegistryDetailPath(rest: string): { instanceId: string; key: string } | null {
  const slash = rest.indexOf("/")
  if (slash <= 0) return null
  try {
    return {
      instanceId: decodeURIComponent(rest.slice(0, slash)),
      key: decodeURIComponent(rest.slice(slash + 1)),
    }
  } catch {
    return {
      instanceId: rest.slice(0, slash),
      key: rest.slice(slash + 1),
    }
  }
}

function pathForView(view: AppView): string {
  if (view.name === "services") return "/services"
  if (view.name === "tool") {
    return `/tools/${encodeURIComponent(view.instanceId)}/${encodeURIComponent(view.toolName)}`
  }
  if (view.name === "resource") {
    return `/resources/${encodeURIComponent(view.instanceId)}/${encodeURIComponent(view.resourceUri)}`
  }
  if (view.name === "prompt") {
    return `/prompts/${encodeURIComponent(view.instanceId)}/${encodeURIComponent(view.promptName)}`
  }
  if (view.name === "instance") {
    const base = `/services/${encodeURIComponent(view.addr.service_name)}`
    const query =
      view.addr.scope.type === "agent"
        ? `?scope=agent&agent_id=${encodeURIComponent(view.addr.scope.agent_id)}`
        : "?scope=store"
    return `${base}${query}`
  }
  if (view.name === "plugins" && view.tab) return `/plugins?tab=${encodeURIComponent(view.tab)}`
  if (view.name === "traces" && view.tab) return `/traces?tab=${encodeURIComponent(view.tab)}`
  return `/${view.name}`
}
