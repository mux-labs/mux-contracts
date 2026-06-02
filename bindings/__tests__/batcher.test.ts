/**
 * Unit tests for MuxBatcherClient binding shape and batcher-specific error mapping.
 */

import { MuxBatcherClient } from "../src/generated/mux-batcher";
import { contractErrorToHttp, ERROR_HTTP_MAP } from "../src/errors";

describe("MuxBatcherClient shape", () => {
  it("exposes executeBatch as a function", () => {
    expect(typeof MuxBatcherClient.prototype.executeBatch).toBe("function");
  });

  it("exposes simulateBatch as a function", () => {
    expect(typeof MuxBatcherClient.prototype.simulateBatch).toBe("function");
  });

  it("exposes maxBatchSize as a function", () => {
    expect(typeof MuxBatcherClient.prototype.maxBatchSize).toBe("function");
  });
});

describe("Batcher error HTTP mapping", () => {
  it("maps BatchTooLarge to 400", () => {
    expect(ERROR_HTTP_MAP.BatchTooLarge).toBe(400);
  });

  it("maps EmptyBatch to 400", () => {
    expect(ERROR_HTTP_MAP.EmptyBatch).toBe(400);
  });

  it("maps RequiredOperationFailed to 500", () => {
    expect(ERROR_HTTP_MAP.RequiredOperationFailed).toBe(500);
  });

  it("maps ReentrancyDetected to 409", () => {
    expect(ERROR_HTTP_MAP.ReentrancyDetected).toBe(409);
  });

  it("contractErrorToHttp returns correct shape for batcher errors", () => {
    const r = contractErrorToHttp("BatchTooLarge");
    expect(r.statusCode).toBe(400);
    expect(r.errorType).toBe("BatchTooLarge");
    expect(r.message).toBe("BatchTooLarge");
  });
});
