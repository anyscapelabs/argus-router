const API_URL = process.env.NEXT_PUBLIC_API_URL;

if (!API_URL) {
  throw new Error("NEXT_PUBLIC_API_URL is not set — see .env.example");
}

export const API_ROUTES = {
  auth: `${API_URL}/api/v1/auth`,
  agents: `${API_URL}/api/v1/agents`,
  billing: `${API_URL}/api/v1/billing`,
  billingCheckout: `${API_URL}/api/v1/billing/checkout`,
  billingInvoices: `${API_URL}/api/v1/billing/invoices`,
  billingPortal: `${API_URL}/api/v1/billing/portal`,
  health: `${API_URL}/api/v1/health`,
  models: `${API_URL}/api/v1/models`,
  onboarding: `${API_URL}/api/v1/onboarding`,
  plans: `${API_URL}/api/v1/plans`,
  profile: `${API_URL}/api/v1/profile`,
  settings: `${API_URL}/api/v1/settings`,
  usage: `${API_URL}/api/v1/usage`,
  workspace: `${API_URL}/api/v1/workspace`,
} as const;

export type ApiRoute = (typeof API_ROUTES)[keyof typeof API_ROUTES];

export { API_URL };
