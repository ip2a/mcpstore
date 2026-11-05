import { cn } from "@/lib/utils"

type UiverseSearchInputProps = {
  id?: string
  placeholder: string
  value: string
  onChange: (value: string) => void
  className?: string
}

/** Uiverse.io satyamchaudharydev/slippery-parrot-22 — WhatsApp-style rotating search. */
export function UiverseSearchInput({
  id,
  placeholder,
  value,
  onChange,
  className,
}: UiverseSearchInputProps) {
  return (
    <form
      className={cn("uiverse-search", className)}
      onSubmit={(event) => event.preventDefault()}
      onReset={() => onChange("")}
    >
      <label htmlFor={id}>
        <input
          id={id}
          type="text"
          required
          autoComplete="off"
          placeholder={placeholder}
          value={value}
          onChange={(event) => onChange(event.target.value)}
        />
        <div className="icon" aria-hidden="true">
          <svg
            className="swap-on"
            strokeWidth="2"
            stroke="currentColor"
            viewBox="0 0 24 24"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
          >
            <path
              d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
              strokeLinejoin="round"
              strokeLinecap="round"
            />
          </svg>
          <svg
            className="swap-off"
            strokeWidth="2"
            stroke="currentColor"
            viewBox="0 0 24 24"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
          >
            <path
              d="M10 19l-7-7m0 0l7-7m-7 7h18"
              strokeLinejoin="round"
              strokeLinecap="round"
            />
          </svg>
        </div>
        <button type="reset" className="close-btn" aria-label="Clear search">
          <svg viewBox="0 0 20 20" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
            <path
              clipRule="evenodd"
              fillRule="evenodd"
              d="M4.293 4.293a1 1 0 011.414 0L10 8.586l4.293-4.293a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 01-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 01-1.414-1.414L8.586 10 4.293 5.707a1 1 0 010-1.414z"
            />
          </svg>
        </button>
      </label>
    </form>
  )
}
