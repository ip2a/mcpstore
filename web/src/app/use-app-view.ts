import { useCallback } from "react"
import { useLocation, useNavigate } from "react-router-dom"

import { type AppView, useViewTitle } from "@/app/app-view"
import { useServiceDetailQuery } from "@/features/services/queries"
import type { ServiceInstance } from "@/lib/api"

export function useAppView(services: ServiceInstance[]) {
  const location = useLocation()
  const navigate = useNavigate()
  const view = viewFromPath(location.pathname)
  const listedService = view.name === "instance" ? services.find((service) => service.instance_id === view.instanceId) : undefined
  const instanceQuery = useServiceDetailQuery(
    view.name === "instance" ? view.instanceId : "",
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

function instanceIdFromPath(pathname: string) {
  const rawName = pathname.slice("/instances/".length)
  try {
    return decodeURIComponent(rawName)
  } catch {
    return rawName
  }
}

function viewFromPath(pathname: string): AppView {
  if (pathname === "/" || pathname === "/services") return { name: "services" }
  if (pathname === "/agents") return { name: "agents" }
  if (pathname === "/tools") return { name: "tools" }
  if (pathname === "/config") return { name: "config" }
  if (pathname === "/cache") return { name: "cache" }
  if (pathname === "/add") return { name: "services" }
  if (pathname.startsWith("/instances/")) return { name: "instance", instanceId: instanceIdFromPath(pathname) }
  return { name: "services" }
}

function pathForView(view: AppView): string {
  if (view.name === "services") return "/services"
  if (view.name === "instance") return `/instances/${encodeURIComponent(view.instanceId)}`
  return `/${view.name}`
}
