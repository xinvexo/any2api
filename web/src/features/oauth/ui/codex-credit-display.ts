import type { OAuthQuotaCredits, OAuthQuotaRateCard } from "../api/oauth-quota-contracts";

const DISPLAY_FRACTION_DIGITS = 4;
const DISPLAY_SCALE = 10n ** BigInt(DISPLAY_FRACTION_DIGITS);

export function formatCodexCreditsUsd(balance: string, creditsPerUsd: number) {
  const [whole, fraction = ""] = balance.split(".");
  const sourceScale = 10n ** BigInt(fraction.length);
  const credits = BigInt(`${whole}${fraction}`);
  const denominator = BigInt(creditsPerUsd) * sourceScale;
  const scaled = credits * DISPLAY_SCALE;
  const quotient = scaled / denominator;
  const remainder = scaled % denominator;
  const rounded = remainder * 2n >= denominator ? quotient + 1n : quotient;
  const dollars = rounded / DISPLAY_SCALE;
  const cents = (rounded % DISPLAY_SCALE)
    .toString()
    .padStart(DISPLAY_FRACTION_DIGITS, "0")
    .replace(/0+$/, "");
  return cents.length > 0 ? `$${dollars}.${cents}` : `$${dollars}`;
}

export function presentCodexCredits(
  credits: OAuthQuotaCredits,
  rateCard: OAuthQuotaRateCard | null,
) {
  if (credits.unlimited) return { value: "无限" };
  if (credits.hasCredits && credits.balance !== null && rateCard !== null) {
    return {
      value: formatCodexCreditsUsd(credits.balance, rateCard.creditsPerUsd),
      detail: `${credits.balance} Credits · ${rateCard.creditsPerUsd} Credits = $1 · ${rateCard.id}`,
    };
  }
  if (credits.hasCredits && credits.balance !== null) {
    return { value: `${credits.balance} Credits` };
  }
  return { value: credits.hasCredits ? "可用（上游未返回余额）" : "不可用" };
}
