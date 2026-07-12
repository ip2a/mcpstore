import { Field, FieldDescription, FieldLabel } from "@/components/ui/field"
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import {
  getRestartPolicyOption,
  getStartupPolicyOption,
  SERVICE_RESTART_POLICY_OPTIONS,
  SERVICE_STARTUP_POLICY_OPTIONS,
} from "@/features/services/service-lifecycle"
import { useI18n } from "@/lib/i18n-context"
import type { ServiceRestartPolicy, ServiceStartupPolicy } from "@/lib/api"

export function ServiceStartupPolicySelect({
  onChange,
  value,
}: {
  value: ServiceStartupPolicy
  onChange: (value: ServiceStartupPolicy) => void
}) {
  const { t } = useI18n()
  const selected = getStartupPolicyOption(value)

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
      <FieldDescription>{t(selected.descriptionKey)}</FieldDescription>
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
  const { t } = useI18n()
  const selected = getRestartPolicyOption(value, t)

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
      <FieldDescription>
        {"description" in selected && selected.description ? selected.description : t(selected.descriptionKey)}
      </FieldDescription>
    </Field>
  )
}
