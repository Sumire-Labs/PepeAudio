import { Link } from "@astryxdesign/core/Link";

import { primaryTrackPageUrl } from "../app/track-presentation";
import type { TrackProvenance } from "../app/types";

interface TrackTitleLinkProps {
  readonly title: string;
  readonly provenance: TrackProvenance | null | undefined;
  readonly type?: "body" | "inherit" | "label" | "large";
  readonly maxLines?: number;
}

export function TrackTitleLink({
  title,
  provenance,
  type = "inherit",
  maxLines
}: TrackTitleLinkProps) {
  const href = primaryTrackPageUrl(provenance);
  if (href === null) return title;
  return (
    <Link
      href={href}
      isExternalLink
      newTabLabel="（新しいタブで開きます）"
      color="primary"
      type={type}
      weight="bold"
      {...(maxLines === undefined ? {} : { maxLines })}
    >
      {title}
    </Link>
  );
}
