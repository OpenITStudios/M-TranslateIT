import { runCommand } from "../shared/tauriBridge";

export type CodexGatewayStatus = {
  state: string;
  blocker: string;
  process_owned: boolean;
  chatgpt_authenticated: boolean;
  included_usage_allowed: boolean;
  app_server_version: string | null;
  diagnostic_protocol_compatible: boolean;
  realtime_audio_verified: boolean;
  meeting_route_enabled: boolean;
  generation: number;
  runtime_claim: string;
};

function unavailable(): CodexGatewayStatus {
  return {
    state: "unavailable",
    blocker: "codex_gateway:frontend_bridge_unavailable",
    process_owned: false,
    chatgpt_authenticated: false,
    included_usage_allowed: false,
    app_server_version: null,
    diagnostic_protocol_compatible: false,
    realtime_audio_verified: false,
    meeting_route_enabled: false,
    generation: 0,
    runtime_claim: "codex_gateway:unverified",
  };
}

// Keep literal command names for the existing Rust <-> TypeScript bridge
// verifiers. Dynamic dispatch would conceal missing registration.
export const codexGatewayApi = {
  async status(): Promise<CodexGatewayStatus> {
    return await runCommand<CodexGatewayStatus>("get_codex_gateway_status") ?? unavailable();
  },
  async connect(): Promise<CodexGatewayStatus> {
    return await runCommand<CodexGatewayStatus>("connect_codex_gateway") ?? unavailable();
  },
  async disconnect(): Promise<CodexGatewayStatus> {
    return await runCommand<CodexGatewayStatus>("disconnect_codex_gateway") ?? unavailable();
  },
};
