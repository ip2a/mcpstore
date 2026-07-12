import type { ServiceLifecycleConfig, ServiceRestartPolicy, ServiceStartupPolicy } from "@/lib/api"

export type LifecyclePolicyOption<T extends string> = {
  value: T
  label: string
  descriptionKey: string
}

export const SERVICE_STARTUP_POLICY_OPTIONS: Array<LifecyclePolicyOption<ServiceStartupPolicy>> = [
  { value: "lazy", label: "lazy", descriptionKey: "lifecycleStartupLazy" },
  { value: "manual", label: "manual", descriptionKey: "lifecycleStartupManual" },
  { value: "on-store-start", label: "on-store-start", descriptionKey: "lifecycleStartupOnStoreStart" },
]

export const SERVICE_RESTART_POLICY_OPTIONS: Array<LifecyclePolicyOption<ServiceRestartPolicy>> = [
  { value: "no", label: "no", descriptionKey: "lifecycleRestartNo" },
  { value: "on-failure", label: "on-failure", descriptionKey: "lifecycleRestartOnFailure" },
  { value: "on-failure:1", label: "on-failure:1", descriptionKey: "lifecycleRestartOnFailure1" },
  { value: "on-failure:3", label: "on-failure:3", descriptionKey: "lifecycleRestartOnFailure3" },
  { value: "on-failure:5", label: "on-failure:5", descriptionKey: "lifecycleRestartOnFailure5" },
  { value: "always", label: "always", descriptionKey: "lifecycleRestartAlways" },
  { value: "unless-stopped", label: "unless-stopped", descriptionKey: "lifecycleRestartUnlessStopped" },
]

export function getStartupPolicyOption(value: ServiceStartupPolicy) {
  return SERVICE_STARTUP_POLICY_OPTIONS.find((option) => option.value === value) || SERVICE_STARTUP_POLICY_OPTIONS[0]
}

export function getRestartPolicyOption(value: ServiceRestartPolicy, t: (key: string, vars?: Record<string, string | number | null | undefined>) => string) {
  const known = SERVICE_RESTART_POLICY_OPTIONS.find((option) => option.value === value)
  if (known) return known

  if (value.startsWith("on-failure:")) {
    const count = value.slice("on-failure:".length)
    return {
      value,
      label: value,
      descriptionKey: "lifecycleRestartOnFailureN",
      description: t("lifecycleRestartOnFailureN", { count }),
    }
  }

  return SERVICE_RESTART_POLICY_OPTIONS[0]
}

export function buildServiceLifecycleConfig({
  restartPolicy,
  startupPolicy,
}: {
  startupPolicy: ServiceStartupPolicy
  restartPolicy: ServiceRestartPolicy
}): ServiceLifecycleConfig {
  return {
    startup_policy: startupPolicy,
    restart_policy: restartPolicy,
  }
}

export function parseServiceLifecycleConfig(
  lifecycle?: ServiceLifecycleConfig | null,
  t?: (key: string, vars?: Record<string, string | number | null | undefined>) => string,
) {
  const startupPolicy: ServiceStartupPolicy = lifecycle?.startup_policy || "lazy"
  const restartPolicy: ServiceRestartPolicy = lifecycle?.restart_policy || "no"
  const translate = t || ((key: string) => key)

  return {
    startupPolicy,
    restartPolicy,
    startupOption: getStartupPolicyOption(startupPolicy),
    restartOption: getRestartPolicyOption(restartPolicy, translate),
  }
}
