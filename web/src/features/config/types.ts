/** 配置重置目标：store 全局 / 某个 agent 作用域。 */
export type ResetTarget = { scope: "store" } | { scope: "agent"; agentId: string }
