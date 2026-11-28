import assert from "node:assert/strict";
import test from "node:test";
import { createMeetingReconcileSchedule } from "../../src/app/runtime/meetingReconcileSchedule.ts";

function fixture(read: () => void | Promise<void>) {
  let sequence = 0;
  const timers = new Map<number, { callback: () => void; delay: number }>();
  const schedule = createMeetingReconcileSchedule(read, {
    set(callback, delay) {
      const id = ++sequence;
      timers.set(id, { callback, delay });
      return id;
    },
    clear(id) { timers.delete(id); },
  }, 80, 10_000);
  return {
    schedule, timers,
    fire(delay: number) {
      assert.equal(timers.size, 1);
      const [id, timer] = [...timers.entries()][0]!;
      assert.equal(timer.delay, delay);
      timers.delete(id);
      timer.callback();
    },
  };
}
const settle = async () => { await Promise.resolve(); await Promise.resolve(); };

test("burst cannot postpone the first event window; idle fallback is replaced once", async () => {
  let reads = 0;
  const f = fixture(() => { reads += 1; });
  f.schedule.request();
  const first = [...f.timers.keys()][0];
  for (let i = 0; i < 100; i++) f.schedule.request();
  assert.deepEqual([...f.timers.keys()], [first]);
  assert.equal(reads, 0);
  f.fire(80);
  await settle();
  assert.equal(reads, 1);
  f.schedule.request();
  f.fire(80);
  await settle();
  assert.equal(reads, 2);
  f.fire(10_000);
  await settle();
  assert.equal(reads, 3);
  f.schedule.dispose();
  assert.equal(f.timers.size, 0);
});

test("events during IPC produce exactly one subsequent read", async () => {
  let reads = 0;
  let release!: () => void;
  const f = fixture(() => {
    reads += 1;
    return new Promise<void>((resolve) => { release = resolve; });
  });
  f.schedule.request();
  f.fire(80);
  for (let i = 0; i < 100; i++) f.schedule.request();
  assert.equal(reads, 1);
  assert.equal(f.timers.size, 0);
  release();
  await settle();
  f.fire(80);
  assert.equal(reads, 2);
  release();
  await settle();
  assert.equal([...f.timers.values()][0]?.delay, 10_000);
  f.schedule.dispose();
});

test("disposal suppresses pending work; failed reads retain fallback", async () => {
  let reject!: (error: Error) => void;
  const f = fixture(() => new Promise<void>((_resolve, fail) => { reject = fail; }));
  f.schedule.request();
  f.fire(80);
  reject(new Error("fixture read unavailable"));
  await settle();
  f.fire(10_000);
  f.schedule.request();
  f.schedule.dispose();
  reject(new Error("fixture read completed after dispose"));
  await settle();
  f.schedule.request();
  assert.equal(f.timers.size, 0);
});
