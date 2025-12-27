import { useEffect, useRef } from "react";
import { useActivityStore } from "../stores/activity-store";
import { ThreadedActivityItem } from "./activity-item";

export function ActivityFeed() {
  const scrollRef = useRef<HTMLDivElement>(null);
  const getFilteredActivities = useActivityStore((s) => s.getFilteredActivities);
  const activities = getFilteredActivities();

  // Filter out child activities (they're rendered inline with their parent)
  const topLevelActivities = activities.filter((a) => !a.parentId);

  // Auto-scroll to top on new activity
  useEffect(() => {
    if (scrollRef.current && topLevelActivities.length > 0) {
      scrollRef.current.scrollTop = 0;
    }
  }, [topLevelActivities.length]);

  if (topLevelActivities.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="text-center px-4">
          <svg
            className="w-12 h-12 mx-auto mb-3 opacity-30"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            strokeWidth={1.5}
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M19 11a7 7 0 01-7 7m0 0a7 7 0 01-7-7m7 7v4m0 0H8m4 0h4m-4-8a3 3 0 01-3-3V5a3 3 0 116 0v6a3 3 0 01-3 3z"
            />
          </svg>
          <p className="text-sm" style={{ color: "var(--color-text-tertiary)" }}>
            No activity yet
          </p>
          <p className="text-xs mt-1" style={{ color: "var(--color-text-tertiary)" }}>
            Start speaking or use voice commands
          </p>
        </div>
      </div>
    );
  }

  return (
    <div
      ref={scrollRef}
      className="flex-1 overflow-y-auto divide-y"
      style={{ borderColor: "var(--color-border)" }}
    >
      {topLevelActivities.map((activity) => (
        <ThreadedActivityItem key={activity.id} activity={activity} />
      ))}
    </div>
  );
}
