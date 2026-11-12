"use client";

import { useQuery } from "@tanstack/react-query";

import { apiFetch } from "@/api/http-client";
import { API_ROUTES } from "@/config/api";

export type AgentsPage = {
  owner: string;
  agents: AgentSummary[];
};

export type AgentSummary = {
  id: string;
  name: string;
  status: string;
};

export function useAgents() {
  return useQuery({
    queryKey: ["agents"] as const,
    queryFn: () => apiFetch<AgentsPage>(API_ROUTES.agents),
  });
}
