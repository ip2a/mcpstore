export const getRequiredEnv = (name) => {
  const value = import.meta.env[name]
  if (value === undefined || value === null || value === '') {
    throw new Error(`[Config] Missing required environment variable: ${name}`)
  }
  return value
}

export const getRequiredEnvNumber = (name) => {
  const raw = getRequiredEnv(name)
  const num = Number(raw)
  if (!Number.isFinite(num)) {
    throw new Error(`[Config] Environment variable ${name} must be a finite number`)
  }
  return num
}


