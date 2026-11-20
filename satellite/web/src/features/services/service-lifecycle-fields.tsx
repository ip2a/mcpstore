import { Field, FieldGroup, FieldLabel } from "@/components/ui/field"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import {
  getRestartPolicyOption,
  getStartupPolicyOption,
  SERVICE_RESTART_POLICY_OPTIONS,
  SERVICE_STARTUP_POLICY_OPTIONS,
} from "@/features/services/service-lifecycle"
import { useI18n } from "@/lib/i18n-context"
import type { ServiceRestartPolicy, ServiceStartupPolicy } from "@/lib/api"

const policyFieldClassName = "justify-between [&>[data-slot=field-label]]:flex-none"
const policyTriggerClassName = "w-44 max-w-[11rem]"

export function ServiceStartupPolicySelect({
  onChange,
  value,
}: {
  value: ServiceStartupPolicy
  onChange: (value: ServiceStartupPolicy) => void
}) {
  const { t } = useI18n()
  const option = getStartupPolicyOption(value)

  return (
    <FieldGroup className="gap-2">
      <Field orientation="horizontal" className={policyFieldClassName}>
        <FieldLabel>startup_policy</FieldLabel>
        <Select value={value} onValueChange={(next) => onChange(next as ServiceStartupPolicy)}>
          <SelectTrigger className={policyTriggerClassName}>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectGroup>
              {SERVICE_STARTUP_POLICY_OPTIONS.map((item) => (
                <SelectItem key={item.value} value={item.value}>
                  {item.label}
                </SelectItem>
              ))}
            </SelectGroup>
          </SelectContent>
        </Select>
      </Field>
      <span className="text-xs text-muted-foreground">{t(option.descriptionKey)}</span>
    </FieldGroup>
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
  const option = getRestartPolicyOption(value, t)
  const hint =
    "description" in option && typeof option.description === "string"
      ? option.description
      : t(option.descriptionKey)

  return (
    <FieldGroup className="gap-2">
      <Field orientation="horizontal" className={policyFieldClassName}>
        <FieldLabel>restart_policy</FieldLabel>
        <Select value={value} onValueChange={(next) => onChange(next as ServiceRestartPolicy)}>
          <SelectTrigger className={policyTriggerClassName}>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectGroup>
              {SERVICE_RESTART_POLICY_OPTIONS.map((item) => (
                <SelectItem key={item.value} value={item.value}>
                  {item.label}
                </SelectItem>
              ))}
            </SelectGroup>
          </SelectContent>
        </Select>
      </Field>
      <span className="text-xs text-muted-foreground">{hint}</span>
    </FieldGroup>
  )
}
