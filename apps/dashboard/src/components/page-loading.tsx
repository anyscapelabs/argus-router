import { FaSpinner } from "react-icons/fa";

export function PageLoading() {
  return (
    <div className="flex min-h-64 w-full items-center justify-center">
      <FaSpinner className="size-6 animate-spin text-primary/60" aria-label="Loading" />
    </div>
  );
}
