import type { ResourceTemplateInfo } from "@/lib/api"
import { hasJsonContent } from "@/lib/tool-info"

export function resourceTemplateKey(template: ResourceTemplateInfo) {
  return resourceTemplateUri(template) || template.name
}

export function resourceTemplateUri(template: ResourceTemplateInfo) {
  return String(template.uriTemplate || template.uri_template || "").trim()
}

export function resourceTemplateMimeType(template: ResourceTemplateInfo) {
  return String(template.mimeType || template.mime_type || "").trim()
}

export function getResourceTemplateMeta(template: ResourceTemplateInfo) {
  return template.meta ?? template._meta ?? null
}

export function getResourceTemplateAnnotations(template: ResourceTemplateInfo) {
  return template.annotations ?? null
}

export function hasResourceTemplateMeta(template: ResourceTemplateInfo) {
  return hasJsonContent(getResourceTemplateMeta(template))
}

export function hasResourceTemplateAnnotations(template: ResourceTemplateInfo) {
  return hasJsonContent(getResourceTemplateAnnotations(template))
}
