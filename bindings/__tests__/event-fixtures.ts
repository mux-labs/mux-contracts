import type { RawSorobanEvent } from "../src/factory-events";

export function makeRawEvent(
  tag: string,
  action: string,
  data: unknown,
): RawSorobanEvent {
  return { topic: [tag, action], value: data };
}
