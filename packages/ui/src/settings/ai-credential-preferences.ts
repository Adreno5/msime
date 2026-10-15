import type { AiAssistantPreferences } from "../index";

/** AI 凭据配置保存和测试共同使用的服务偏好投影。 */
export type AiCredentialPreferences = Pick<
  AiAssistantPreferences,
  "provider" | "endpoint" | "model"
>;
