export const BackpressurePolicy = {
  DropOldest: 'DropOldest',
  DropNewest: 'DropNewest',
  BlockWithTimeout: 'BlockWithTimeout',
  Error: 'Error',
};

export class SpscRingBuffer {
  constructor(capacity = 1024, policy = BackpressurePolicy.Error) {
    // Ensure capacity is a power of two
    let cap = 1;
    while (cap < capacity) {
      cap <<= 1;
    }
    this.capacity = cap;
    this.mask = cap - 1;
    this.policy = policy;
    this.buffer = Buffer.alloc(this.capacity);
    this.head = 0; // write index
    this.tail = 0; // read index
  }

  availableRead() {
    return this.head - this.tail;
  }

  availableWrite() {
    return this.capacity - (this.head - this.tail);
  }

  isFull() {
    return this.availableWrite() === 0;
  }

  isEmpty() {
    return this.availableRead() === 0;
  }

  write(data) {
    const buf = Buffer.isBuffer(data) ? data : Buffer.from(data);
    if (buf.length > this.capacity) {
      throw new Error('Write length ' + buf.length + ' exceeds total buffer capacity ' + this.capacity);
    }

    if (this.availableWrite() < buf.length) {
      switch (this.policy) {
        case BackpressurePolicy.Error:
          throw new Error('BufferError: Full');
        case BackpressurePolicy.DropNewest:
          return 0; // Discard write
        case BackpressurePolicy.DropOldest:
          // Advance tail to free enough space
          const needed = buf.length - this.availableWrite();
          this.tail += needed;
          break;
        case BackpressurePolicy.BlockWithTimeout:
          throw new Error('BufferTimeout: timed out waiting for consumer');
      }
    }

    for (let i = 0; i < buf.length; i++) {
      this.buffer[(this.head + i) & this.mask] = buf[i];
    }
    this.head += buf.length;
    return buf.length;
  }

  read(maxLen) {
    const available = this.availableRead();
    if (available === 0) return Buffer.alloc(0);
    const toRead = Math.min(available, maxLen);
    const out = Buffer.alloc(toRead);

    for (let i = 0; i < toRead; i++) {
      out[i] = this.buffer[(this.tail + i) & this.mask];
    }
    this.tail += toRead;
    return out;
  }
}
