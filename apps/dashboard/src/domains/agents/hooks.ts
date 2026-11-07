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

/**
 * `GET /api/v1/agents` — session-protected list of the operator's agents.
 * Placeholder payload until the agents domain model is built.
 */
export function useAgents() {
  return useQuery({
    queryKey: ["agents"] as const,
    queryFn: () => apiFetch<AgentsPage>(API_ROUTES.agents),
  });
}
