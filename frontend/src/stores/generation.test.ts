import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { useGenerationStore } from "./generation";

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

describe("generation store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.restoreAllMocks();
    FakeEventSource.instances = [];
    vi.stubGlobal("EventSource", FakeEventSource);
  });

  it("creates a task and subscribes to task events", async () => {
    vi.spyOn(globalThis, "fetch")
      .mockResolvedValueOnce(
        jsonResponse({
          success: true,
          data: {
            id: "task-1",
            request: {
              topic: "Rust workflow",
              platform: "web",
              reference_urls: [],
              reference_ratio: 0.3,
              template_category: "general",
              template_name: "magazine",
              save_output: true
            },
            status: "Pending",
            created_at: "2026-03-28T00:00:00Z",
            updated_at: "2026-03-28T00:00:00Z",
            events: []
          }
        })
      )
      .mockResolvedValueOnce(
        jsonResponse({
          success: true,
          data: [
            {
              id: "task-1",
              request: {
                topic: "Rust workflow",
                platform: "web",
                reference_urls: [],
                reference_ratio: 0.3,
                template_category: "general",
                template_name: "magazine",
                save_output: true
              },
              status: "Running",
              created_at: "2026-03-28T00:00:00Z",
              updated_at: "2026-03-28T00:00:00Z",
              events: []
            }
          ]
        })
      );

    const store = useGenerationStore();
    store.form.topic = "Rust workflow";

    await store.createTask();

    expect(store.current?.id).toBe("task-1");
    expect(FakeEventSource.instances[0]?.url).toContain("/api/generation/tasks/task-1/events");
  });
});
