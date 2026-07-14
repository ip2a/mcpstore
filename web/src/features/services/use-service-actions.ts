import {
  checkInstance,
  connectInstance,
  disconnectInstance,
  removeServiceScope,
  restartInstance,
  type ServiceInstance,
} from "@/lib/api"

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
  refreshInstanceQueries: (instanceId: string, scope: ServiceInstance["scope"]) => Promise<void>
  runAction: RunAction
  services: ServiceInstance[]
}) {
  function checkAllServices() {
    return runAction("check:instances", () => Promise.all(services.map((service) => checkInstance(service.instance_id))))
  }

  function connectServiceEntry(service: ServiceInstance) {
    return runAction(
      `connect:${service.instance_id}`,
      () => connectInstance(service.instance_id),
      () => refreshInstanceQueries(service.instance_id, service.scope),
    )
  }

  function disconnectServiceEntry(service: ServiceInstance) {
    return runAction(
      `disconnect:${service.instance_id}`,
      () => disconnectInstance(service.instance_id),
      () => refreshInstanceQueries(service.instance_id, service.scope),
    )
  }

  function restartServiceEntry(service: ServiceInstance) {
    return runAction(
      `restart:${service.instance_id}`,
      () => restartInstance(service.instance_id),
      () => refreshInstanceQueries(service.instance_id, service.scope),
    )
  }

  function removeServiceEntry(service: ServiceInstance) {
    return runAction(
      `delete:${service.instance_id}`,
      () => removeServiceScope(service.service_name, service.scope),
      () => refreshInstanceQueries(service.instance_id, service.scope),
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
