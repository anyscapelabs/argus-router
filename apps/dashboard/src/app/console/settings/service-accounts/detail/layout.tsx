import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Service Account | Anyscape Gateway",
  description: "Service account details and logs.",
};

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}
