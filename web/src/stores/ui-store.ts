import { create } from "zustand"

type UiState = {
  homeHeroCollapsed: boolean
  settingsDialogOpen: boolean
  selectedAgentId: string | null
  setHomeHeroCollapsed: (collapsed: boolean) => void
  setSettingsDialogOpen: (open: boolean) => void
  setSelectedAgentId: (agentId: string | null) => void
}

export const useUiStore = create<UiState>((set) => ({
  homeHeroCollapsed: false,
  settingsDialogOpen: false,
  selectedAgentId: null,
  setHomeHeroCollapsed: (homeHeroCollapsed) => set({ homeHeroCollapsed }),
  setSettingsDialogOpen: (settingsDialogOpen) => set({ settingsDialogOpen }),
  setSelectedAgentId: (selectedAgentId) => set({ selectedAgentId }),
}))
