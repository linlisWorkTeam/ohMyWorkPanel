import { partsToPlainText } from "../chat/messageContent";

export type MarketingMarker =
  | { kind: "campaign"; campaignId: string }
  | { kind: "internal"; campaignId: string; stage: string };

export function parseMarketingMarker(content: string): MarketingMarker | null {
  const plain = partsToPlainText(content).trim();
  const campaign = plain.match(/^\[\[MARKETING_CAMPAIGN:([^\]]+)\]\]$/);
  if (campaign) return { kind: "campaign", campaignId: campaign[1] };
  const internal = plain.match(/^\[\[MARKETING_INTERNAL:([^:\]]+):([^\]]+)\]\]$/);
  if (internal) return { kind: "internal", campaignId: internal[1], stage: internal[2] };
  return null;
}
