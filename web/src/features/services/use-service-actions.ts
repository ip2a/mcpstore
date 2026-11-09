import { toast } from "sonner"

import {
  checkInstance,
  connectInstance,
  disconnectInstance,
  getServiceState,
  removeServiceScope,
  restartInstance,
  type ServiceAddress,
  type ServiceInstance,
} from "@/lib/api"
import type { ServiceState } from "@/lib/api/service"
import { useI18n } from "@/lib/i18n-context"

type RunAction = (
  label: string,
  action: () => Promise<unknown>,
  onSuccess?: () => Promise<void> | void,
) => Promise<void>

export function useServiceActions({
  refreshInstanceQueries,
  runAction,
  services,
}: {
  refreshInstanceQueries: (addr: ServiceAddress) => Promise<void>
  runAction: RunAction
  services: ServiceInstance[]
}) {
  const { t } = useI18n()

  function checkAllServices() {
    return runAction("check:instances", () =>
      Promise.all(services.map((service) => checkInstance(service))),
    )
  }

  function connectServiceEntry(service: ServiceInstance) {
    return runAction(
      `connect:${service.instance_id}`,
      async () => {
        // Connect intent recorded; response data is the state snapshot.
        // Connection failures surface as state (phase/health/recovery), not errors —
        // the supervisor retries in the background.
        const state = await connectInstance(service)
        if (state?.auth?.status === "unauthenticated") {
          toast.info(t("oauthConnectLoginRequired"))
        }
        // The connect itself runs in the background, so the snapshot above still
        // says "starting". Poll until the phase settles, then refresh the lists —
        // otherwise the card spins on "Connecting" until a manual reload.
        for (let i = 0; i < 20; i++) {
          await new Promise((resolve) => setTimeout(resolve, 1500))
          let settled: ServiceState | undefined
          try {
            settled = await getServiceState(service)
          } catch {
            break
          }
          if (!settled || settled.phase !== "starting") break
        }
      },
      () => refreshInstanceQueries(service),
    )
  }

  function disconnectServiceEntry(service: ServiceInstance) {
    return runAction(
      `disconnect:${service.instance_id}`,
      () => disconnectInstance(service),
      () => refreshInstanceQueries(service),
    )
  }

  function restartServiceEntry(service: ServiceInstance) {
    return runAction(
      `restart:${service.instance_id}`,
      () => restartInstance(service),
      () => refreshInstanceQueries(service),
    )
  }

  function removeServiceEntry(service: ServiceInstance) {
    return runAction(
      `delete:${service.instance_id}`,
      () => removeServiceScope(service.service_name, service.scope),
      () => refreshInstanceQueries(service),
    )
  }

  return {
    checkAllServices,
    connectServiceEntry,
    disconnectServiceEntry,
    removeServiceEntry,
    restartServiceEntry,
  }
}
