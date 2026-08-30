// Rivun WASM Sandboxing & Linear Memory Execution Simulator
// Implements WASM guest sandbox simulator with fuel metering, memory limits, and ABI v1

import { blake3 } from './blake3.mjs';

export class WasmGuestSandbox {
  constructor({
    initialFuel = 100_000,
    maxMemoryPages = 16, // 16 pages * 64KiB = 1MiB
    epochTimeoutMs = 50,
  } = {}) {
    this.initialFuel = initialFuel;
    this.fuel = initialFuel;
    this.maxMemoryBytes = maxMemoryPages * 64 * 1024;
    this.memory = Buffer.alloc(Math.min(64 * 1024, this.maxMemoryBytes)); // start with 1 page
    this.allocated = new Map(); // ptr -> size
    this.nextPtr = 8; // start above null pointer (0)
    this.epochTimeoutMs = epochTimeoutMs;
  }

  alloc(len) {
    if (len <= 0) throw new Error('Invalid allocation length');
    if (this.fuel < 10) throw new Error('Out of fuel');
    this.fuel -= 10;

    const ptr = this.nextPtr;
    if (ptr + len > this.maxMemoryBytes) {
      throw new Error('LinearMemoryExceeded: Requested ' + (ptr + len) + 'B > limit ' + this.maxMemoryBytes + 'B');
    }
    if (ptr + len > this.memory.length) {
      const newSize = Math.min(this.maxMemoryBytes, Math.max(this.memory.length * 2, ptr + len));
      const newMem = Buffer.alloc(newSize);
      this.memory.copy(newMem, 0, 0, this.memory.length);
      this.memory = newMem;
    }
    this.allocated.set(ptr, len);
    this.nextPtr = (ptr + len + 7) & ~7; // 8-byte aligned
    return ptr;
  }

  dealloc(ptr, len) {
    if (this.fuel < 5) throw new Error('Out of fuel');
    this.fuel -= 5;
    this.allocated.delete(ptr);
  }

  writeMemory(ptr, data) {
    const buf = Buffer.isBuffer(data) ? data : Buffer.from(data);
    if (ptr + buf.length > this.memory.length) {
      throw new Error('Memory out of bounds write');
    }
    buf.copy(this.memory, ptr, 0, buf.length);
  }

  readMemory(ptr, len) {
    if (ptr + len > this.memory.length) {
      throw new Error('Memory out of bounds read');
    }
    return Buffer.from(this.memory.subarray(ptr, ptr + len));
  }

  execute(action, payload, driverLogic) {
    const startTime = Date.now();
    const actionBuf = Buffer.isBuffer(action) ? action : Buffer.from(action, 'utf8');
    const payloadBuf = Buffer.isBuffer(payload) ? payload : Buffer.from(payload);

    const actionPtr = this.alloc(actionBuf.length);
    this.writeMemory(actionPtr, actionBuf);

    const payloadPtr = this.alloc(payloadBuf.length);
    this.writeMemory(payloadPtr, payloadBuf);

    // Run guest execution logic
    const fuelCost = 50 + payloadBuf.length * 2;
    if (this.fuel < fuelCost) {
      throw new Error('DriverExecutionError: out of fuel (remaining: ' + this.fuel + ', required: ' + fuelCost + ')');
    }
    this.fuel -= fuelCost;

    if (Date.now() - startTime > this.epochTimeoutMs) {
      throw new Error('DriverExecutionError: epoch timeout expired (' + this.epochTimeoutMs + 'ms)');
    }

    const resultBuf = driverLogic ? driverLogic(actionBuf, payloadBuf) : payloadBuf;
    if (Date.now() - startTime >= this.epochTimeoutMs) {
      throw new Error('DriverExecutionError: epoch timeout expired (' + this.epochTimeoutMs + 'ms)');
    }

    const resultPtr = this.alloc(resultBuf.length);
    this.writeMemory(resultPtr, resultBuf);

    // Pack 64-bit return value: (resultPtr << 32) | resultLen
    const packed = (BigInt(resultPtr) << 32n) | BigInt(resultBuf.length);

    this.dealloc(actionPtr, actionBuf.length);
    this.dealloc(payloadPtr, payloadBuf.length);

    return {
      resultPtr,
      resultLen: resultBuf.length,
      packed,
      output: resultBuf,
      remainingFuel: this.fuel,
      executionTimeMs: Date.now() - startTime,
    };
  }
}

export class DriverPipeline {
  constructor(stages = []) {
    this.stages = stages; // array of { name, wasmModule, logic }
  }

  run(initialPayload) {
    let current = Buffer.isBuffer(initialPayload) ? initialPayload : Buffer.from(initialPayload);
    const executionTrace = [];
    const stepHashes = [];

    for (const stage of this.stages) {
      const sandbox = new WasmGuestSandbox();
      const res = sandbox.execute(stage.name, current, stage.logic);
      current = res.output;
      const stepHash = blake3(current);
      stepHashes.push(stepHash.toString('hex'));
      executionTrace.push({
        stage: stage.name,
        outputLen: current.length,
        remainingFuel: res.remainingFuel,
        stepHash: stepHash.toString('hex'),
      });
    }

    return {
      finalOutput: current,
      stepHashes,
      trace: executionTrace,
    };
  }
}
