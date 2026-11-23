"use client";

import { ConfirmDialog } from "@/components/confirm-dialog";

export type ApiKeyAction = "rename" | "revoke" | "delete";

export function ApiKeyActionDialog({
  action,
  keyName,
  nameValue = "",
  onNameChange,
  isPending,
  error,
  onConfirm,
  onCancel,
}: {
  action: ApiKeyAction | null;
  keyName?: string;
  nameValue?: string;
  onNameChange?: (value: string) => void;
  isPending?: boolean;
  error?: string | null;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const isDelete = action === "delete";
  const isRename = action === "rename";

  const description = isRename ? (
    <div>
      <p>
        Choose a new name for <span className="text-primary">{keyName}</span>.
      </p>
      <label htmlFor="api-key-name" className="mt-3 block text-xs text-primary/60">
        Key name
      </label>
      <input
        id="api-key-name"
        value={nameValue}
        onChange={(event) => onNameChange?.(event.target.value)}
        className="mt-1 w-full rounded-full border border-border-secondary bg-transparent px-3 py-2 text-sm text-primary outline-none focus:border-primary/40"
      />
    </div>
  ) : isDelete ? (
    <>
      This will revoke <span className="text-primary">{keyName}</span> and permanently
      delete its usage history. This cannot be undone.
    </>
  ) : (
    <>
      This will revoke <span className="text-primary">{keyName}</span>. The key will
      stop working immediately.
    </>
  );

  return (
    <ConfirmDialog
      open={action !== null}
      title={isRename ? "Rename API key" : isDelete ? "Delete API key" : "Revoke API key"}
      description={description}
      confirmLabel={isRename ? "Save" : isDelete ? "Delete key" : "Revoke key"}
      tone={isDelete ? "danger" : isRename ? "default" : "warning"}
      cardClassName={isRename ? "bg-background" : undefined}
      isPending={isPending}
      error={error}
      onConfirm={onConfirm}
      onCancel={onCancel}
    />
  );
}
