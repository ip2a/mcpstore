import {
  CableIcon,
  DatabaseIcon,
  FileStackIcon,
  HouseIcon,
  ScrollTextIcon,
  ServerIcon,
  WaypointsIcon,
  WrenchIcon,
  type LucideIcon,
} from "lucide-react"
import type { ServiceAddress } from "@/lib/api"
import { useI18n } from "@/lib/i18n-context"

export type AppView =
  | { name: "home" }
  | { name: "services" }
  | { name: "clients" }
  | { name: "tools" }
  | { name: "tool"; instanceId: string; toolName: string }
  | { name: "resources" }
  | { name: "prompts" }
  | { name: "gateway" }
  | { name: "cache" }
  | { name: "settings" }
  | { name: "instance"; addr: ServiceAddress }

export type NavItem = {
  view: Exclude<AppView, { name: "instance" }>
  label: string
  icon: LucideIcon
}

export function useNavItems(): NavItem[] {
  const { t } = useI18n()
  return [
    { view: { name: "home" }, label: t("navHome"), icon: HouseIcon },
    { view: { name: "services" }, label: t("navServices"), icon: ServerIcon },
    { view: { name: "clients" }, label: t("navClients"), icon: CableIcon },
    { view: { name: "tools" }, label: t("navTools"), icon: WrenchIcon },
    { view: { name: "resources" }, label: t("navResources"), icon: FileStackIcon },
    { view: { name: "prompts" }, label: t("navPrompts"), icon: ScrollTextIcon },
    { view: { name: "gateway" }, label: t("gateway"), icon: WaypointsIcon },
    { view: { name: "cache" }, label: t("navCache"), icon: DatabaseIcon },
  ]
}

export function useViewTitle(view: AppView): string {
  const { t } = useI18n()
  const items = useNavItems()
  if (view.name === "instance") return view.addr.service_name
  if (view.name === "tool") return view.toolName
  if (view.name === "settings") return t("settings")
  return items.find((item) => item.view.name === view.name)?.label || t("navServices")
}
