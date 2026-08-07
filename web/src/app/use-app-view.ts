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
  const listedService = view.name === "instance"
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
    if (location.key !== "default") navigate(-1)
    else navigate("/services", { replace: true })
  }, [location.key, navigate])

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
  if (pathname === "/" || pathname === "/services") return { name: "services" }
  if (pathname === "/scope") return { name: "agents" }
  if (pathname === "/tools") return { name: "tools" }
  if (pathname === "/config") return { name: "agents" }
  if (pathname === "/cache") return { name: "cache" }
  if (pathname === "/add") return { name: "services" }
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
  return { name: "services" }
}

function pathForView(view: AppView): string {
  if (view.name === "services") return "/services"
  if (view.name === "agents") return "/scope"
  if (view.name === "instance") {
    const base = `/services/${encodeURIComponent(view.addr.service_name)}`
    const query =
      view.addr.scope.type === "agent"
        ? `?scope=agent&agent_id=${encodeURIComponent(view.addr.scope.agent_id)}`
        : "?scope=store"
    return `${base}${query}`
  }
  return `/${view.name}`
}
