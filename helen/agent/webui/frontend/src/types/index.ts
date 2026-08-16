/**
 * 附件类型（多模态支持）
 */
export interface Attachment {
  id: string;           // upload_id
  filename: string;
  mime_type: string;
  size: number;
  url: string;          // 预览 URL
}

/**
 * 消息类型
 */
export interface Message {
  id: number | string;
  session_id?: string;
  role: 'user' | 'assistant' | 'system' | 'thinking';
  content: string;
  timestamp: string;
  // 推理中提示注入 (hint injection)
  isHint?: boolean;                        // 用户消息是否为 hint（处理中发送）
  hintStatus?: 'queued' | 'injected' | 'processed';  // hint 生命周期
  client_id?: string;                      // 用于 hint ack 关联
  // 多模态附件（v6.2）
  attachments?: Attachment[];
}

/**
 * 会话类型(v6.1:从 Helen transcript 目录读取)
 */
export interface Session {
  session_id: string;
  created_at: number;   // Unix epoch
  modified_at: number;  // Unix epoch
  size_bytes: number;
  message_count: number;
  preview: string;      // 首条 user 消息截断
}

/**
 * Statusline 数据（ChatWindow 底部状态栏）
 * 由 Helen 通过 Python FFI (ui.status_emitter) 推送，关键节点更新
 */
export interface StatuslineData {
  hostname?: string;
  cwd?: string;
  user?: string;
  model?: string;
  usageRatio: number;  // 0.0 - 1.0+，前端 *100 显示百分比
}

/**
 * Agent 状态
 */
export interface AgentStatus {
  name: string;
  status: 'idle' | 'running' | 'completed' | 'error';
  last_task?: string;
  progress?: number;
}

/**
 * WebSocket 消息类型
 */
export type WSMessage =
  | { type: 'message'; content: string }
  | { type: 'llm_chunk'; data: { type: string; content?: string } }
  | { type: 'llm_complete' }
  | { type: 'error'; data: { message: string } };

/**
 * API 响应类型
 */
export interface ApiResponse<T> {
  status: 'ok' | 'error';
  data?: T;
  message?: string;
}
