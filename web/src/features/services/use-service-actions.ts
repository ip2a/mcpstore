import { toast } from "sonner"

import {
  checkInstance,
  connectInstance,
  disconnectInstance,
  removeServiceScope,
  restartInstance,
  ApiError,
  type ServiceAddress,
  type ServiceInstance,
} from "@/lib/api"
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
    return runAction("check:instances", () => Promise.all(services.map((service) => checkInstance(service))))
  }

  function connectServiceEntry(service: ServiceInstance) {
    return runAction(
      `connect:${service.instance_id}`,
      async () => {
        try {
          await connectInstance(service)
        } catch (error) {
          if (error instanceof ApiError && error.code === "connection_auth_required") {
            toast.info(t("oauthConnectLoginRequired"))
          }
          throw error
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
