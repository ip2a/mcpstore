import {
  checkServices,
  connectService,
  disconnectService,
  removeService,
  restartService,
  type ServiceEntry,
} from "@/lib/api"

type RunAction = (
  label: string,
  action: () => Promise<unknown>,
  onSuccess?: () => Promise<void> | void,
) => Promise<void>

export function useServiceActions({
  refreshServiceQueries,
  runAction,
}: {
  refreshServiceQueries: (serviceName: string, agentId?: string) => Promise<void>
  runAction: RunAction
}) {
  function checkAllServices() {
    return runAction("check:services", checkServices)
  }

  function connectServiceEntry(service: ServiceEntry) {
    return runAction(`connect:${service.name}`, () => connectService(service.name), () => refreshServiceQueries(service.name, service.agent_id))
  }

  function disconnectServiceEntry(service: ServiceEntry) {
    return runAction(`disconnect:${service.name}`, () => disconnectService(service.name), () => refreshServiceQueries(service.name, service.agent_id))
  }

  function restartServiceEntry(service: ServiceEntry) {
    return runAction(`restart:${service.name}`, () => restartService(service.name), () => refreshServiceQueries(service.name, service.agent_id))
  }

  function removeServiceEntry(service: ServiceEntry) {
    return runAction(`delete:${service.name}`, () => removeService(service.name), () => refreshServiceQueries(service.name, service.agent_id))
  }

  return {
    checkAllServices,
    connectServiceEntry,
    disconnectServiceEntry,
    removeServiceEntry,
    restartServiceEntry,
  }
}
