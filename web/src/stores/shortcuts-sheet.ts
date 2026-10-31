import { create } from "zustand"

type ShortcutsSheetState = {
  open: boolean
  setOpen: (open: boolean) => void
  toggle: () => void
}

export const useShortcutsSheet = create<ShortcutsSheetState>((set, get) => ({
  open: false,
  setOpen: (open) => set({ open }),
  toggle: () => set({ open: !get().open }),
}))
