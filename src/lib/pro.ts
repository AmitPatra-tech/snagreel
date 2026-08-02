// Pro / activation configuration.
//
// Flow: pay via Dodo Payments → email the payment screenshot to the support
// address → receive an activation key → enter it in Settings › Pro.

/** Support inbox that receives the payment screenshot. */
export const PRO_SUPPORT_EMAIL = "mikarmiaura@gmail.com";

/** Dodo Payments checkout link for the Pro upgrade. */
export const DODO_CHECKOUT_URL =
  "https://checkout.dodopayments.com/buy/pdt_0Njm11QwZVfXNtIQoIq5G";

/** Human-readable list of what Pro unlocks (shown on the paywall). */
export const PRO_FEATURES = [
  "Clip / cutout — download just a section of a long video",
  "Transcribe audio & video to text in 90+ languages (high accuracy)",
  "Extract audio from any file, and trim or convert your downloads",
] as const;
