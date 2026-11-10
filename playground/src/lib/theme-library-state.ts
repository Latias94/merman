import { create } from "zustand";
import { createThemeLibraryFilters, type ThemeLibraryFilters } from "./theme-library.ts";

interface ThemeLibraryState {
  readonly filters: ThemeLibraryFilters;
  readonly initialized: boolean;
  initialize(familyId: string | null): void;
  changeFilters(filters: ThemeLibraryFilters): void;
}

export const useThemeLibraryStore = create<ThemeLibraryState>((set, get) => ({
  filters: createThemeLibraryFilters(null),
  initialized: false,
  initialize(familyId) {
    if (get().initialized || familyId === null) return;
    set({ initialized: true, filters: createThemeLibraryFilters(familyId) });
  },
  changeFilters(filters) {
    set({ initialized: true, filters });
  },
}));
