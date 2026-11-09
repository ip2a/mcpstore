import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"

import {
  getClientVersion,
  getGatewayProjection,
  getGatewayRequests,
  getGatewayStatus,
  getWiredClients,
  listClients,
  unwireClient,
  wireClient,
  type WireOptions,
  type WireResult,
} from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"
import { toast } from "sonner"
import { useI18n } from "@/lib/i18n-context"

export function useClientsQuery() {
  return useQuery({ queryKey: queryKeys.clients, queryFn: listClients })
}

/** CLI semver probe; isolated from the clients list so listing stays fast. */
export function useClientVersionQuery(clientId: string, enabled: boolean) {
  return useQuery({
    queryKey: queryKeys.clientVersion(clientId),
    queryFn: () => getClientVersion(clientId),
    enabled,
    staleTime: 60_000,
    retry: false,
  })
}

export function useWiredClientsQuery() {
  return useQuery({ queryKey: queryKeys.wiredClients, queryFn: getWiredClients })
}

export function useGatewayStatusQuery() {
  return useQuery({
    queryKey: queryKeys.gatewayStatus,
    queryFn: getGatewayStatus,
    refetchInterval: 5_000,
  })
}

/** Live projection issues; only polled while the gateway child is running. */
export function useGatewayProjectionQuery(running: boolean) {
  return useQuery({
    queryKey: queryKeys.gatewayProjection,
    queryFn: getGatewayProjection,
    enabled: running,
    refetchInterval: 5_000,
  })
}

export function useWireClientMutation() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({
      client,
      options,
      force,
    }: {
      client: string
      options?: WireOptions
      force?: boolean
    }) => wireClient(client, options, force),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.clients })
      void queryClient.invalidateQueries({ queryKey: queryKeys.wiredClients })
      void queryClient.invalidateQueries({ queryKey: queryKeys.gatewayStatus })
    },
  })
}

export function useUnwireClientMutation() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: unwireClient,
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.clients })
      void queryClient.invalidateQueries({ queryKey: queryKeys.wiredClients })
      void queryClient.invalidateQueries({ queryKey: queryKeys.gatewayStatus })
    },
  })
}

/**
 * Client wire/unwire operations (toast + inventory invalidation). Wire with options overrides scope/expose config.
 * Returns the wire result; on outcome=conflict the caller shows the confirm-replace dialog — no toast here.
 */
export function useClientToggle() {
  const { t } = useI18n()
  const wire = useWireClientMutation()
  const unwire = useUnwireClientMutation()
  async function toggle(
    client: { id: string; label: string },
    wired: boolean,
    options?: WireOptions,
    force?: boolean,
  ): Promise<WireResult | undefined> {
    try {
      if (wired) {
        await unwire.mutateAsync(client.id)
        toast.success(t("clientsUnwireOk", { client: client.label }))
      } else {
        const result = await wire.mutateAsync({ client: client.id, options, force })
        if (result.outcome === "conflict") return result
        toast.success(t("clientsWireOk", { client: client.label }))
        return result
      }
    } catch (err) {
      toast.error(err instanceof Error ? err.message : String(err))
    }
  }
  return { toggle, busy: wire.isPending || unwire.isPending }
}
