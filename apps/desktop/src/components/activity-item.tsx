import { ReactNode } from "react";
import type { Activity } from "../types/activity";
import { useActivityStore } from "../stores/activity-store";

interface ActivityItemProps {
  activity: Activity;
  children?: ReactNode;
}

const TYPE_CONFIG: Record<
  Activity["type"],
  { icon: ReactNode; color: string; label: string }
> = {
  transcript: {
    label: "Transcript",
    color: "#6b7280", // gray
    icon: (
      <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
        <path strokeLinecap="round" strokeLinejoin="round" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
      </svg>
    ),
  },
  voice_command: {
    label: "Command",
    color: "#8b5cf6", // purple
    icon: (
      <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
        <path strokeLinecap="round" strokeLinejoin="round" d="M19 11a7 7 0 01-7 7m0 0a7 7 0 01-7-7m7 7v4m0 0H8m4 0h4m-4-8a3 3 0 01-3-3V5a3 3 0 116 0v6a3 3 0 01-3 3z" />
      </svg>
    ),
  },
  tool_result: {
    label: "Result",
    color: "#10b981", // green
    icon: (
      <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
        <path strokeLinecap="round" strokeLinejoin="round" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
      </svg>
    ),
  },
  tool_error: {
    label: "Error",
    color: "#ef4444", // red
    icon: (
      <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
        <path strokeLinecap="round" strokeLinejoin="round" d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
      </svg>
    ),
  },
  recording: {
    label: "Recording",
    color: "#ef4444", // red
    icon: (
      <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 24 24">
        <circle cx="12" cy="12" r="6" />
      </svg>
    ),
  },
  context_change: {
    label: "Mode",
    color: "#3b82f6", // blue
    icon: (
      <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
        <path strokeLinecap="round" strokeLinejoin="round" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
      </svg>
    ),
  },
};

const STATUS_INDICATOR: Record<Activity["status"], ReactNode> = {
  pending: (
    <span className="w-2 h-2 rounded-full bg-gray-400 animate-pulse" />
  ),
  running: (
    <span className="w-2 h-2 rounded-full bg-purple-500 animate-pulse" />
  ),
  completed: (
    <svg className="w-4 h-4 text-green-500" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
      <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
    </svg>
  ),
  error: (
    <svg className="w-4 h-4 text-red-500" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
      <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
    </svg>
  ),
};

function formatTime(timestamp: number): string {
  return new Date(timestamp).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });
}

function formatDuration(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${minutes}:${secs.toString().padStart(2, "0")}`;
}

function renderToolResult(activity: Activity, isExpanded: boolean): ReactNode {
  const result = activity.content.result;
  const tool = activity.content.tool;

  // Handle search results (web_search tool)
  if (tool === "web_search" && result?.result) {
    const { title, summary, url, thumbnail_url } = result.result;
    return (
      <div className="space-y-2">
        <div className="flex items-start gap-3">
          {thumbnail_url && (
            <img
              src={thumbnail_url}
              alt=""
              className="w-12 h-12 rounded object-cover flex-shrink-0"
            />
          )}
          <div className="flex-1 min-w-0">
            <p className="text-sm font-medium" style={{ color: "var(--color-text-primary)" }}>
              {title}
            </p>
            <p className="text-xs mt-1" style={{ color: "var(--color-text-secondary)" }}>
              {summary}
            </p>
            {url && (
              <a
                href={url}
                target="_blank"
                rel="noopener noreferrer"
                className="text-xs mt-1 inline-block hover:underline"
                style={{ color: "var(--color-accent)" }}
                onClick={(e) => e.stopPropagation()}
              >
                Read more →
              </a>
            )}
          </div>
        </div>
      </div>
    );
  }

  // Default: show tool name with expandable JSON
  return (
    <div>
      <div className="flex items-center gap-2">
        <span className="text-xs" style={{ color: "var(--color-text-tertiary)" }}>
          {tool}
        </span>
        {STATUS_INDICATOR[activity.status]}
      </div>
      {isExpanded && result && (
        <pre className="mt-2 p-2 text-xs rounded overflow-auto max-h-32" style={{ background: "var(--color-bg-tertiary)" }}>
          {JSON.stringify(result, null, 2)}
        </pre>
      )}
    </div>
  );
}

export function ActivityItem({ activity, children }: ActivityItemProps) {
  const toggleExpanded = useActivityStore((s) => s.toggleExpanded);
  const config = TYPE_CONFIG[activity.type];
  const isExpanded = activity.expanded ?? false;

  const renderContent = () => {
    switch (activity.type) {
      case "transcript":
        return (
          <p className="text-sm" style={{ color: "var(--color-text-primary)" }}>
            {activity.content.text}
          </p>
        );

      case "voice_command":
        return (
          <div className="flex items-center gap-2">
            <span className="text-sm font-medium" style={{ color: config.color }}>
              "{activity.content.text}"
            </span>
            <span className="text-xs px-1.5 py-0.5 rounded" style={{ background: `${config.color}20`, color: config.color }}>
              {activity.content.tool}
            </span>
          </div>
        );

      case "tool_result":
        return renderToolResult(activity, isExpanded);

      case "tool_error":
        return (
          <div className="flex items-center gap-2">
            <span className="text-xs" style={{ color: "var(--color-text-tertiary)" }}>
              {activity.content.tool}
            </span>
            <span className="text-xs text-red-500">{activity.content.error}</span>
          </div>
        );

      case "recording":
        return (
          <div className="flex items-center gap-2">
            <span className="text-sm" style={{ color: "var(--color-text-secondary)" }}>
              Recording session
            </span>
            {activity.content.duration && (
              <span className="text-xs" style={{ color: "var(--color-text-tertiary)" }}>
                {formatDuration(activity.content.duration)}
              </span>
            )}
          </div>
        );

      case "context_change":
        return (
          <div className="flex items-center gap-2 text-xs" style={{ color: "var(--color-text-tertiary)" }}>
            <span>{activity.content.prevMode}</span>
            <svg className="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M13 7l5 5m0 0l-5 5m5-5H6" />
            </svg>
            <span style={{ color: config.color }}>{activity.content.mode}</span>
          </div>
        );
    }
  };

  // web_search shows content directly, other tool_results are expandable
  const hasExpandableContent =
    activity.type === "tool_result" &&
    activity.content.result &&
    activity.content.tool !== "web_search";

  return (
    <div
      className={`px-3 py-2 transition-colors ${hasExpandableContent ? "cursor-pointer hover:bg-[var(--color-bg-secondary)]" : ""}`}
      onClick={hasExpandableContent ? () => toggleExpanded(activity.id) : undefined}
    >
      <div className="flex items-start gap-2">
        <span style={{ color: config.color }}>{config.icon}</span>
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 mb-0.5">
            <span className="text-[10px]" style={{ color: "var(--color-text-tertiary)" }}>
              {formatTime(activity.timestamp)}
            </span>
            {hasExpandableContent && (
              <svg
                className={`w-3 h-3 transition-transform ${isExpanded ? "rotate-90" : ""}`}
                style={{ color: "var(--color-text-tertiary)" }}
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
                strokeWidth={2}
              >
                <path strokeLinecap="round" strokeLinejoin="round" d="M9 5l7 7-7 7" />
              </svg>
            )}
          </div>
          {renderContent()}
        </div>
      </div>
      {children}
    </div>
  );
}

interface ThreadedActivityItemProps {
  activity: Activity;
}

export function ThreadedActivityItem({ activity }: ThreadedActivityItemProps) {
  const getChildActivities = useActivityStore((s) => s.getChildActivities);
  const children = getChildActivities(activity.id);

  return (
    <div>
      <ActivityItem activity={activity} />
      {children.length > 0 && (
        <div className="ml-6 border-l-2" style={{ borderColor: "var(--color-border)" }}>
          {children.map((child) => (
            <ActivityItem key={child.id} activity={child} />
          ))}
        </div>
      )}
    </div>
  );
}
