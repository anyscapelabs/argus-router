"use client";

import { useState } from "react";

type WorkspaceAvatarProps = {
  image?: string | null;
  initials?: string | null;
};

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
