/**
 * Activity model for the unified activity feed.
 * Mirrors the Rust DTO in crates/events/src/activity.rs
 */

export type ActivityType =
  | "transcript"
  | "voice_command"
  | "tool_result"
  | "tool_error"
  | "recording"
  | "context_change";

export type ActivityStatus = "pending" | "running" | "completed" | "error";

export interface ActivityContent {
  text?: string;
  tool?: string;
  args?: Record<string, unknown>;
  result?: Record<string, unknown>;
  error?: string;
  duration?: number;
  mode?: string;
  prevMode?: string;
  app?: string;
}

export interface Activity {
  id: string;
  type: ActivityType;
  timestamp: number;
  status: ActivityStatus;
  parentId?: string;
  content: ActivityContent;
  expanded?: boolean;
}

export type ActivityFilter = ActivityType | "all";
