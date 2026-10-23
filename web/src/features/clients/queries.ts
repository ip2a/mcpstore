import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"

import {
  getGatewayRequests,
  getGatewayStatus,
  getWiredClients,
  listClients,
  unwireClient,
  wireClient,
  type WireOptions,
} from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"
import { toast } from "sonner"
import { useI18n } from "@/lib/i18n-context"

export function useClientsQuery() {
  return useQuery({ queryKey: queryKeys.clients, queryFn: listClients })
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

export function useWireClientMutation() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ client, options }: { client: string; options?: WireOptions }) =>
      wireClient(client, options),
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

/** 客户端接线/断线操作（toast + 清单失效）。wire 带 options 时覆盖作用域/暴露配置。 */
export function useClientToggle() {
  const { t } = useI18n()
  const wire = useWireClientMutation()
  const unwire = useUnwireClientMutation()
  async function toggle(
    client: { id: string; label: string },
    wired: boolean,
    options?: WireOptions,
  ) {
    try {
      if (wired) {
        await unwire.mutateAsync(client.id)
        toast.success(t("clientsUnwireOk", { client: client.label }))
      } else {
        await wire.mutateAsync({ client: client.id, options })
        toast.success(t("clientsWireOk", { client: client.label }))
      }
    } catch (err) {
      toast.error(err instanceof Error ? err.message : String(err))
    }
  }
  return { toggle, busy: wire.isPending || unwire.isPending }
}
