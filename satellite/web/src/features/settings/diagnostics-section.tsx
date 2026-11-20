import type { ReactNode } from "react"

import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"
import { SettingsRow, SettingsSection } from "@/features/settings/settings-section"
import { useI18n } from "@/lib/i18n-context"
import type { DiagnosticsDraft } from "@/features/settings/model"

export function DiagnosticsSection({
  value,
  onChange,
}: {
  value: DiagnosticsDraft
  onChange: (value: DiagnosticsDraft) => void
}) {
  const { t } = useI18n()

  function patch<K extends keyof DiagnosticsDraft>(
    section: K,
    values: Partial<DiagnosticsDraft[K]>,
  ) {
    onChange({ ...value, [section]: { ...value[section], ...values } })
  }

  const enabled = (channel: string) => t("diagnosticsEnableChannel", { channel })
  const enabledDescription = (channel: string) =>
    t("diagnosticsEnableChannelDescription", { channel })

  return (
    <div className="flex flex-col gap-6 py-2">
      <Group title={t("diagnosticsRuntimeLog")}>
        <Toggle
          title={enabled(t("diagnosticsRuntimeLog"))}
          description={enabledDescription(t("diagnosticsRuntimeLog"))}
          checked={value.runtime_log.enabled}
          onCheckedChange={(enabled) => patch("runtime_log", { enabled })}
        />
        <SettingsRow title={t("diagnosticsLevel")}>
          <Select
            value={value.runtime_log.level}
            onValueChange={(level) => patch("runtime_log", { level })}
          >
            <SelectTrigger className="w-36">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectGroup>
                {(["trace", "debug", "info", "warn", "error"] as const).map((level) => (
                  <SelectItem key={level} value={level}>
                    {level}
                  </SelectItem>
                ))}
              </SelectGroup>
            </SelectContent>
          </Select>
        </SettingsRow>
        <NumberField
          title={t("diagnosticsMaxSizeBytes")}
          value={value.runtime_log.max_size_bytes}
          onChange={(max_size_bytes) => patch("runtime_log", { max_size_bytes })}
          suffix={t("diagnosticsBytes")}
        />
        <NumberField
          title={t("diagnosticsRetentionDays")}
          description={t("unlimited")}
          value={value.runtime_log.retention_days}
          onChange={(retention_days) => patch("runtime_log", { retention_days })}
          onClear={() => patch("runtime_log", { retention_days: null })}
          suffix={t("days")}
          nullable
        />
      </Group>

      <Group title={t("diagnosticsRequests")}>
        <Toggle
          title={enabled(t("diagnosticsRequests"))}
          description={enabledDescription(t("diagnosticsRequests"))}
          checked={value.requests.enabled}
          onCheckedChange={(enabled) => patch("requests", { enabled })}
        />
        <Toggle
          title={t("diagnosticsCaptureTool")}
          checked={value.requests.capture_tool}
          onCheckedChange={(capture_tool) => patch("requests", { capture_tool })}
        />
        <Toggle
          title={t("diagnosticsCaptureManagement")}
          checked={value.requests.capture_management}
          onCheckedChange={(capture_management) => patch("requests", { capture_management })}
        />
        <Toggle
          title={t("diagnosticsCaptureCli")}
          checked={value.requests.capture_cli}
          onCheckedChange={(capture_cli) => patch("requests", { capture_cli })}
        />
        <Toggle
          title={t("diagnosticsCapturePayload")}
          description={t("diagnosticsCapturePayloadDescription")}
          checked={value.requests.capture_payload}
          onCheckedChange={(capture_payload) => patch("requests", { capture_payload })}
        />
        <NumberField
          title={t("diagnosticsMaxPayloadBytes")}
          value={value.requests.max_payload_bytes}
          onChange={(max_payload_bytes) => patch("requests", { max_payload_bytes })}
          suffix={t("diagnosticsBytes")}
        />
        <NumberField
          title={t("diagnosticsRetentionDays")}
          description={t("unlimited")}
          value={value.requests.retention_days}
          onChange={(retention_days) => patch("requests", { retention_days })}
          onClear={() => patch("requests", { retention_days: null })}
          suffix={t("days")}
          nullable
        />
        <NumberField
          title={t("diagnosticsMaxRecords")}
          description={t("unlimited")}
          value={value.requests.max_records}
          onChange={(max_records) => patch("requests", { max_records })}
          onClear={() => patch("requests", { max_records: null })}
          suffix={t("diagnosticsRecords")}
          nullable
        />
      </Group>

      <Group title={t("diagnosticsGatewayCalls")}>
        <Toggle
          title={enabled(t("diagnosticsGatewayCalls"))}
          description={enabledDescription(t("diagnosticsGatewayCalls"))}
          checked={value.gateway_calls.enabled}
          onCheckedChange={(enabled) => patch("gateway_calls", { enabled })}
        />
        <Toggle
          title={t("diagnosticsPersistCalls")}
          description={t("diagnosticsPersistCallsDescription")}
          checked={value.gateway_calls.persist}
          onCheckedChange={(persist) => patch("gateway_calls", { persist })}
        />
        <NumberField
          title={t("diagnosticsMemoryCapacity")}
          value={value.gateway_calls.memory_capacity}
          onChange={(memory_capacity) => patch("gateway_calls", { memory_capacity })}
          suffix={t("diagnosticsCalls")}
        />
        <NumberField
          title={t("diagnosticsRetentionDays")}
          description={t("unlimited")}
          value={value.gateway_calls.retention_days}
          onChange={(retention_days) => patch("gateway_calls", { retention_days })}
          onClear={() => patch("gateway_calls", { retention_days: null })}
          suffix={t("days")}
          nullable
        />
        <NumberField
          title={t("diagnosticsMaxCalls")}
          description={t("unlimited")}
          value={value.gateway_calls.max_calls}
          onChange={(max_calls) => patch("gateway_calls", { max_calls })}
          onClear={() => patch("gateway_calls", { max_calls: null })}
          suffix={t("diagnosticsCalls")}
          nullable
        />
      </Group>

      <Group title={t("diagnosticsFrames")}>
        <Toggle
          title={enabled(t("diagnosticsFrames"))}
          description={enabledDescription(t("diagnosticsFrames"))}
          checked={value.frames.enabled}
          onCheckedChange={(enabled) => patch("frames", { enabled })}
        />
        <Toggle
          title={t("diagnosticsCaptureGateway")}
          checked={value.frames.capture_gateway}
          onCheckedChange={(capture_gateway) => patch("frames", { capture_gateway })}
        />
        <Toggle
          title={t("diagnosticsCaptureUpstream")}
          checked={value.frames.capture_upstream}
          onCheckedChange={(capture_upstream) => patch("frames", { capture_upstream })}
        />
        <Toggle
          title={t("diagnosticsCapturePayload")}
          description={t("diagnosticsCapturePayloadDescription")}
          checked={value.frames.capture_payload}
          onCheckedChange={(capture_payload) => patch("frames", { capture_payload })}
        />
        <NumberField
          title={t("diagnosticsMaxPayloadBytes")}
          value={value.frames.max_payload_bytes}
          onChange={(max_payload_bytes) => patch("frames", { max_payload_bytes })}
          suffix={t("diagnosticsBytes")}
        />
        <NumberField
          title={t("diagnosticsRetentionDays")}
          description={t("unlimited")}
          value={value.frames.retention_days}
          onChange={(retention_days) => patch("frames", { retention_days })}
          onClear={() => patch("frames", { retention_days: null })}
          suffix={t("days")}
          nullable
        />
        <NumberField
          title={t("diagnosticsMaxRecords")}
          description={t("unlimited")}
          value={value.frames.max_records}
          onChange={(max_records) => patch("frames", { max_records })}
          onClear={() => patch("frames", { max_records: null })}
          suffix={t("diagnosticsRecords")}
          nullable
        />
      </Group>

      <Group title={t("diagnosticsEventHistory")}>
        <Toggle
          title={enabled(t("diagnosticsEventHistory"))}
          description={enabledDescription(t("diagnosticsEventHistory"))}
          checked={value.event_history.enabled}
          onCheckedChange={(enabled) => patch("event_history", { enabled })}
        />
        <NumberField
          title={t("diagnosticsEventCapacity")}
          value={value.event_history.capacity}
          onChange={(capacity) => patch("event_history", { capacity })}
          suffix={t("diagnosticsRecords")}
        />
      </Group>

      <Group title={t("diagnosticsProcessOutput")}>
        <Toggle
          title={enabled(t("diagnosticsStdioStderr"))}
          description={enabledDescription(t("diagnosticsStdioStderr"))}
          checked={value.stdio_stderr.enabled}
          onCheckedChange={(enabled) => patch("stdio_stderr", { enabled })}
        />
        <NumberField
          title={t("diagnosticsTailLines")}
          description={t("diagnosticsTailLinesDescription")}
          value={value.stdio_stderr.tail_lines}
          onChange={(tail_lines) => patch("stdio_stderr", { tail_lines })}
          suffix={t("diagnosticsLines")}
          allowZero
        />
        <Toggle
          title={t("diagnosticsForwardStderr")}
          checked={value.stdio_stderr.forward_to_runtime_log}
          onCheckedChange={(forward_to_runtime_log) =>
            patch("stdio_stderr", { forward_to_runtime_log })
          }
        />
        <Toggle
          title={t("diagnosticsIncludeStderrErrors")}
          checked={value.stdio_stderr.include_in_error}
          onCheckedChange={(include_in_error) => patch("stdio_stderr", { include_in_error })}
        />
        <Toggle
          title={t("diagnosticsDaemonOutput")}
          description={t("diagnosticsDaemonOutputDescription")}
          checked={value.daemon_output.enabled}
          onCheckedChange={(enabled) => patch("daemon_output", { enabled })}
        />
      </Group>
    </div>
  )
}

function Group({ title, children }: { title: string; children: ReactNode }) {
  return <SettingsSection title={title}>{children}</SettingsSection>
}

function Toggle({
  title,
  description,
  checked,
  onCheckedChange,
}: {
  title: string
  description?: string
  checked: boolean
  onCheckedChange: (checked: boolean) => void
}) {
  return (
    <SettingsRow title={title} description={description}>
      <Switch aria-label={title} checked={checked} onCheckedChange={onCheckedChange} />
    </SettingsRow>
  )
}

function NumberField({
  title,
  description,
  value,
  onChange,
  onClear,
  suffix,
  nullable = false,
  allowZero = false,
}: {
  title: string
  description?: string
  value: number | null
  onChange: (value: number) => void
  onClear?: () => void
  suffix?: string
  nullable?: boolean
  allowZero?: boolean
}) {
  const { t } = useI18n()
  const minimum = allowZero ? 0 : 1
  return (
    <SettingsRow title={title} description={description}>
      <InputGroup className="w-36 shrink-0">
        <InputGroupInput
          aria-label={title}
          type="number"
          min={minimum}
          step={1}
          placeholder={nullable ? t("unlimitedPlaceholder") : undefined}
          value={value ?? ""}
          onChange={(event) => {
            const raw = event.target.value
            if (nullable && raw === "") {
              onClear?.()
              return
            }
            const parsed = Number(raw)
            onChange(Number.isFinite(parsed) ? Math.max(minimum, Math.trunc(parsed)) : minimum)
          }}
        />
        {suffix ? <InputGroupAddon align="inline-end">{suffix}</InputGroupAddon> : null}
      </InputGroup>
    </SettingsRow>
  )
}
