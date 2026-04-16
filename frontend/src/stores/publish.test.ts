import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { usePublishStore } from "./publish";

function jsonResponse(data: unknown): Response {
  return new Response(JSON.stringify(data), {
    status: 200,
    headers: { "Content-Type": "application/json" }
  });
}

class FakeEventSource {
  static instances: FakeEventSource[] = [];

  url: string;
  onmessage: ((event: MessageEvent<string>) => void) | null = null;
  onerror: (() => void) | null = null;

  constructor(url: string) {
    this.url = url;
    FakeEventSource.instances.push(this);
  }

  close() {
    return undefined;
  }
}

describe("publish store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.restoreAllMocks();
    FakeEventSource.instances = [];
    vi.stubGlobal("EventSource", FakeEventSource);
  });

  it("loads running publish tasks and subscribes to events", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValueOnce(
      jsonResponse({
        success: true,
        data: [
          {
            id: "publish-task-1",
            request: {
              article_relative_path: "vibecoding.md",
              target_id: "publish-wechat-1",
              mode: "draft"
            },
            status: "Running",
            created_at: "2026-04-12T00:00:00Z",
            updated_at: "2026-04-12T00:00:01Z",
            events: []
          }
        ]
      })
    );

    const store = usePublishStore();
    await store.loadTasks();

    expect(store.current?.id).toBe("publish-task-1");
    expect(FakeEventSource.instances[0]?.url).toContain("/api/publish/tasks/publish-task-1/events");
  });

  it("retries a failed publish task without creating a new task id", async () => {
    vi.spyOn(globalThis, "fetch")
      .mockResolvedValueOnce(
        jsonResponse({
          success: true,
          data: {
            id: "publish-task-1",
            request: {
              article_relative_path: "vibecoding.md",
              target_id: "publish-wechat-1",
              mode: "draft"
            },
            status: "Pending",
            created_at: "2026-04-12T00:00:00Z",
            updated_at: "2026-04-12T00:00:02Z",
            events: []
          }
        })
      )
      .mockResolvedValueOnce(
        jsonResponse({
          success: true,
          data: [
            {
              id: "publish-task-1",
              request: {
                article_relative_path: "vibecoding.md",
                target_id: "publish-wechat-1",
                mode: "draft"
              },
              status: "Running",
              created_at: "2026-04-12T00:00:00Z",
              updated_at: "2026-04-12T00:00:02Z",
              events: []
            }
          ]
        })
      );

    const store = usePublishStore();
    store.tasks = [
      {
        id: "publish-task-1",
        request: {
          article_relative_path: "vibecoding.md",
          target_id: "publish-wechat-1",
          mode: "draft"
        },
        status: "Failed",
        created_at: "2026-04-12T00:00:00Z",
        updated_at: "2026-04-12T00:00:01Z",
        events: [],
        output: undefined,
        error: "boom"
      }
    ];

    const retried = await store.retryTask(store.tasks[0]);

    expect(retried?.id).toBe("publish-task-1");
    expect(fetch).toHaveBeenNthCalledWith(
      1,
      "/api/publish/tasks/publish-task-1/retry",
      expect.objectContaining({
        method: "POST"
      })
    );
    expect(FakeEventSource.instances[0]?.url).toContain("/api/publish/tasks/publish-task-1/events");
  });

  it("deletes completed publish tasks from local state", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValueOnce(
      jsonResponse({
        success: true,
        data: {
          task_id: "publish-task-1"
        }
      })
    );

    const store = usePublishStore();
    const task = {
      id: "publish-task-1",
      request: {
        article_relative_path: "vibecoding.md",
        target_id: "publish-wechat-1",
        mode: "draft"
      },
      status: "Failed" as const,
      created_at: "2026-04-12T00:00:00Z",
      updated_at: "2026-04-12T00:00:01Z",
      events: [],
      output: undefined,
      error: "boom"
    };
    store.tasks = [task];
    store.current = task;
    store.eventSource = new FakeEventSource("/api/publish/tasks/publish-task-1/events") as never;

    const deleted = await store.deleteTask("publish-task-1");

    expect(deleted).toBe(true);
    expect(fetch).toHaveBeenCalledWith(
      "/api/publish/tasks/publish-task-1",
      expect.objectContaining({
        method: "DELETE"
      })
    );
    expect(store.tasks).toHaveLength(0);
    expect(store.current).toBeNull();
    expect(store.eventSource).toBeNull();
  });
});
