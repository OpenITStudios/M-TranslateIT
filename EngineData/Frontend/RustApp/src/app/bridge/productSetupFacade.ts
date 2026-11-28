import { applicationRuntimeApi, type ApplicationIntentResult } from "./applicationRuntimeApi";
import { runtimeApi } from "./runtimeApi";
import { compact } from "../shared/state";

export type ProductSetupAction = "check-readiness" | "verify-models" | "retry-worker";
export type ProductRecoveryAction = "fix-setup";

export type ProductSetupResult = {
  message: string;
  receipt: ApplicationIntentResult | null;
};

export async function runProductSetupAction(action: ProductSetupAction): Promise<ProductSetupResult> {
  if (action === "retry-worker") {
    const result = await applicationRuntimeApi.dispatchIntent("retry_blocked_worker").catch(() => null);
    return {
      message: compact(
        result?.message,
        result?.ok
          ? "Worker restarted. Check Again to verify current capabilities."
          : "Worker recovery failed. Check Again for the current status.",
      ),
      receipt: result,
    };
  }
  if (action === "check-readiness") {
    const result = await applicationRuntimeApi.dispatchIntent("check_readiness").catch(() => null);
    // Surface the canonical native admission reason (including an unverified
    // Codex choice) instead of replacing every failure with a generic notice.
    // Unknown/failed bridge calls still use the conservative fallback.
    return {
      message: result?.ok
        ? "TranslateIT is ready."
        : compact(
            result?.message,
            "TranslateIT still needs attention. Try Check Again, then open Help if the problem continues.",
          ),
      receipt: result,
    };
  }

  const result = await runtimeApi.verifyModels().catch(() => null);
  const blockers = Array.isArray(result?.blockers) ? result.blockers.join("; ") : "";
  return {
    message: compact(
      result?.note ?? blockers,
      result?.ok ? "All required AI files are available." : "Some required AI files still need attention.",
    ),
    receipt: null,
  };
}

export async function runProductRecoveryAction(action: ProductRecoveryAction): Promise<ProductSetupResult> {
  if (action !== "fix-setup") return { message: "No product recovery action was selected.", receipt: null };

  const result = await applicationRuntimeApi.dispatchIntent("fix_setup");
  if (!result.ok) {
    return {
      message: compact(
        result.message,
        "Setup still needs attention. Try Check Again, then open Help if needed.",
      ),
      receipt: result,
    };
  }

  const hasBlockingProblem = result.snapshot.problems.some((problem) => problem.severity === "blocking");
  if (hasBlockingProblem) {
    return { message: "Setup still needs attention. Try Check Again, then open Help if needed.", receipt: result };
  }

  return { message: "TranslateIT is ready. Check Meeting again; the TranslateIT microphone may still need attention.", receipt: result };
}
