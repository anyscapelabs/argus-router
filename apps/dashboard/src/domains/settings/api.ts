import { API_ROUTES } from "@/config/api";
import { apiFetch } from "@/api/http-client";

export type UserSettings = {
  allowFreeTraining: boolean;
  maxKeyLifetimeDays: number;
};

export type SettingsPatch = Partial<UserSettings>;

export async function getSettings(): Promise<UserSettings> {
  return apiFetch<UserSettings>(API_ROUTES.settings);
}

export async function updateSettings(
  patch: SettingsPatch,
): Promise<UserSettings> {
  return apiFetch<UserSettings>(API_ROUTES.settings, {
    method: "PATCH",
    body: patch,
  });
}
