import {
  BotIcon,
  DatabaseIcon,
  ServerIcon,
  SlidersHorizontalIcon,
  WrenchIcon,
  type LucideIcon,
} from "lucide-react"
import { useI18n } from "@/lib/i18n-context"

export type AppView =
  | { name: "services" }
  | { name: "agents" }
  | { name: "tools" }
  | { name: "config" }
  | { name: "cache" }
  | { name: "service"; serviceName: string }

export type NavItem = {
  view: Exclude<AppView, { name: "service" }>
  label: string
  icon: LucideIcon
}

export function useNavItems(): NavItem[] {
  const { t } = useI18n()
  return [
    { view: { name: "services" }, label: t("navServices"), icon: ServerIcon },
    { view: { name: "agents" }, label: t("navAgents"), icon: BotIcon },
    { view: { name: "tools" }, label: t("navTools"), icon: WrenchIcon },
    { view: { name: "config" }, label: t("navConfig"), icon: SlidersHorizontalIcon },
    { view: { name: "cache" }, label: t("navCache"), icon: DatabaseIcon },
  ]
}

export function useViewTitle(view: AppView): string {
  const { t } = useI18n()
  if (view.name === "service") return view.serviceName
  const items = useNavItems()
  return items.find((item) => item.view.name === view.name)?.label || t("navServices")
}
