import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Billing and Invoices | Anyscape Gateway",
  description: "Manage your billing and invoices.",
};

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}
