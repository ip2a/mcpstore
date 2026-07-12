import { useCallback } from "react"
import { useLocation, useNavigate } from "react-router-dom"

import { type AppView, useViewTitle } from "@/app/app-view"
import type { ServiceEntry } from "@/lib/api"

export function useAppView(services: ServiceEntry[]) {
  const location = useLocation()
  const navigate = useNavigate()
  const view = viewFromPath(location.pathname)
  const selectedService = view.name === "service" ? services.find((service) => service.name === view.serviceName) : undefined
  const pageTitle = useViewTitle(view)
  const setView = useCallback((nextView: AppView) => navigate(pathForView(nextView)), [navigate])
  const goBack = useCallback(() => {
    if (location.key !== "default") navigate(-1)
    else navigate("/services", { replace: true })
  }, [location.key, navigate])

  return { goBack, pageTitle, selectedService, setView, view }
}

function serviceNameFromPath(pathname: string) {
  const rawName = pathname.slice("/services/".length)
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
  if (pathname.startsWith("/services/")) return { name: "service", serviceName: serviceNameFromPath(pathname) }
  return { name: "services" }
}

function pathForView(view: AppView): string {
  if (view.name === "services") return "/services"
  if (view.name === "service") return `/services/${encodeURIComponent(view.serviceName)}`
  return `/${view.name}`
}
