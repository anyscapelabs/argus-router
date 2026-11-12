export function modelDisplayName(model: string): string {
  const slash = model.indexOf("/");
  return slash === -1 ? model : model.slice(slash + 1);
}

export function formatTokenCount(n: number): string {
  if (n < 1000) return n.toLocaleString();
  return new Intl.NumberFormat("en-US", {
    notation: "compact",
    maximumFractionDigits: 1,
  }).format(n);
}
