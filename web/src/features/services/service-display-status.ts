import type { ServiceState } from "@/lib/api"

export type ServiceDisplayStatus = "connected" | "connecting" | "disconnected" | "error"

export function deriveServiceDisplayStatus(state: ServiceState): ServiceDisplayStatus {
  if (state.desired === "stopped") return "disconnected"
  if (state.recovery.status === "exhausted") return "error"
  if (state.phase === "starting" || state.recovery.status === "waiting" || state.recovery.status === "probing") {
    return "connecting"
  }
  if (state.health === "unhealthy") return "error"
  if (state.phase === "running") return "connected"
  return "disconnected"
}

export function isServiceConnected(state?: ServiceState) {
  return state ? deriveServiceDisplayStatus(state) === "connected" : false
}

export function isServiceConnecting(state?: ServiceState) {
  return state ? deriveServiceDisplayStatus(state) === "connecting" : false
}