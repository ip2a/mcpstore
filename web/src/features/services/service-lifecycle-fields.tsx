import { Field, FieldLabel } from "@/components/ui/field"
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { SERVICE_RESTART_POLICY_OPTIONS, SERVICE_STARTUP_POLICY_OPTIONS } from "@/features/services/service-lifecycle"
import type { ServiceRestartPolicy, ServiceStartupPolicy } from "@/lib/api"

export function ServiceStartupPolicySelect({
  onChange,
  value,
}: {
  value: ServiceStartupPolicy
  onChange: (value: ServiceStartupPolicy) => void
}) {
  return (
    <Field>
      <FieldLabel>startup_policy</FieldLabel>
      <Select value={value} onValueChange={(next) => onChange(next as ServiceStartupPolicy)}>
        <SelectTrigger>
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectGroup>
            {SERVICE_STARTUP_POLICY_OPTIONS.map((option) => (
              <SelectItem key={option.value} value={option.value}>
                {option.label}
              </SelectItem>
            ))}
          </SelectGroup>
        </SelectContent>
      </Select>
    </Field>
  )
}

export function ServiceRestartPolicySelect({
  onChange,
  value,
}: {
  value: ServiceRestartPolicy
  onChange: (value: ServiceRestartPolicy) => void
}) {
  return (
    <Field>
      <FieldLabel>restart_policy</FieldLabel>
      <Select value={value} onValueChange={(next) => onChange(next as ServiceRestartPolicy)}>
        <SelectTrigger>
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectGroup>
            {SERVICE_RESTART_POLICY_OPTIONS.map((option) => (
              <SelectItem key={option.value} value={option.value}>
                {option.label}
              </SelectItem>
            ))}
          </SelectGroup>
        </SelectContent>
      </Select>
    </Field>
  )
}
