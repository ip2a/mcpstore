import { useState } from "react"

import { type AppView, viewTitle } from "@/app/app-view"
import type { ServiceEntry } from "@/lib/api"

export function useAppView(services: ServiceEntry[]) {
  const [view, setView] = useState<AppView>({ name: "services" })
  const selectedService = view.name === "service" ? services.find((service) => service.name === view.serviceName) : undefined
  const pageTitle = viewTitle(view)

  return { pageTitle, selectedService, setView, view }
}
