import { ArrowRightIcon, MoreHorizontalIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { ServiceStatusBadge } from "@/components/shared/service-status-badge"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table"
import type { ServiceEntry } from "@/lib/api"

export function ServiceTable(props: {
  services: ServiceEntry[]
  agentMap: Map<string, string>
  busy: string | null
  onConnect: (service: ServiceEntry) => void
  onDelete: (service: ServiceEntry) => void
  onDisconnect: (service: ServiceEntry) => void
  onOpen: (service: ServiceEntry) => void
  onRestart: (service: ServiceEntry) => void
}) {
  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>Service</TableHead>
          <TableHead>Agent</TableHead>
          <TableHead>Transport</TableHead>
          <TableHead>Status</TableHead>
          <TableHead>Tools</TableHead>
          <TableHead className="text-right">Actions</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {props.services.map((service) => (
          <TableRow
            key={service.name}
            className="cursor-pointer"
            tabIndex={0}
            onClick={() => props.onOpen(service)}
            onKeyDown={(event) => {
              if (event.target !== event.currentTarget) return
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault()
                props.onOpen(service)
              }
            }}
          >
            <TableCell>
              <button
                className="flex max-w-96 flex-col gap-1 text-left"
                type="button"
                onClick={(event) => {
                  event.stopPropagation()
                  props.onOpen(service)
                }}
              >
                <span className="font-medium">{service.name}</span>
                <span className="truncate text-sm text-muted-foreground">{String(service.config?.description || "No description")}</span>
              </button>
            </TableCell>
            <TableCell>{props.agentMap.get(service.name) || service.agent_id || "store"}</TableCell>
            <TableCell>{service.transport || "-"}</TableCell>
            <TableCell><ServiceStatusBadge status={service.status} /></TableCell>
            <TableCell>{service.tools?.length || 0}</TableCell>
            <TableCell className="text-right">
              <div className="flex justify-end gap-2" onClick={(event) => event.stopPropagation()}>
                <Button variant="ghost" size="sm" onClick={() => props.onOpen(service)}>
                  Detail
                  <ArrowRightIcon data-icon="inline-end" />
                </Button>
                <DropdownMenu>
                  <DropdownMenuTrigger asChild>
                    <Button variant="ghost" size="icon" aria-label={`Actions for ${service.name}`}>
                      <MoreHorizontalIcon />
                    </Button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="end">
                    <DropdownMenuGroup>
                      {service.status === "Connected" ? (
                        <DropdownMenuItem onClick={() => props.onDisconnect(service)}>Disconnect</DropdownMenuItem>
                      ) : (
                        <DropdownMenuItem onClick={() => props.onConnect(service)}>Connect</DropdownMenuItem>
                      )}
                      <DropdownMenuItem onClick={() => props.onRestart(service)}>Restart</DropdownMenuItem>
                      <DropdownMenuItem variant="destructive" onClick={() => props.onDelete(service)}>Delete</DropdownMenuItem>
                    </DropdownMenuGroup>
                  </DropdownMenuContent>
                </DropdownMenu>
              </div>
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  )
}
