export type AppView =
  | { name: "services" }
  | { name: "agents" }
  | { name: "tools" }
  | { name: "config" }
  | { name: "cache" }
  | { name: "add" }
  | { name: "service"; serviceName: string }

export const navItems: Array<{ view: AppView; label: string }> = [
  { view: { name: "services" }, label: "服务" },
  { view: { name: "agents" }, label: "Agent" },
  { view: { name: "tools" }, label: "工具" },
  { view: { name: "config" }, label: "配置" },
  { view: { name: "cache" }, label: "缓存" },
]

export function viewTitle(view: AppView): string {
  if (view.name === "service") return view.serviceName
  if (view.name === "add") return "添加服务"
  return navItems.find((item) => item.view.name === view.name)?.label || "服务"
}
