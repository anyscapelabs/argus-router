"use client";

import { useState } from "react";

type WorkspaceAvatarProps = {
  /** Initials avatar from the API's avatar service, seeded with the workspace name. */
  image?: string | null;
  /** Text fallback while the workspace is still loading or the image fails. */
  initials?: string | null;
};

/**
 * Workspace chip in the top bar. The letters come from the backend
 * (`GET /api/v1/workspace`), so a renamed workspace changes its avatar instead
 * of staying stuck on one glyph.
 */
export function WorkspaceAvatar({ image, initials }: WorkspaceAvatarProps) {
  const [failedImage, setFailedImage] = useState<string | null>(null);
  const showImage = Boolean(image) && failedImage !== image;

  return (
    <div className="flex size-6 items-center justify-center overflow-hidden rounded-sm border border-border bg-tertiary text-xs text-primary">
      {showImage ? (
        <img
          src={image ?? undefined}
          alt=""
          className="size-full object-cover"
          referrerPolicy="no-referrer"
          onError={() => setFailedImage(image ?? null)}
        />
      ) : (
        initials ?? "?"
      )}
    </div>
  );
}
