import { test, expect } from './fixtures';

const requestedRuns = Number.parseInt(process.env.GOSLING_PERFORMANCE_RUNS ?? '10', 10);
const sampleCount = Number.isSafeInteger(requestedRuns) && requestedRuns >= 5 ? requestedRuns : 10;
const enabled = process.env.GOSLING_RUN_PERFORMANCE === '1';
const readinessSamples: number[] = [];

function percentile(samples: number[], fraction: number): number {
  const sorted = [...samples].sort((left, right) => left - right);
  return sorted[Math.ceil(fraction * sorted.length) - 1];
}

test.describe('Desktop renderer-readiness benchmark', () => {
  test.skip(!enabled, 'Set GOSLING_RUN_PERFORMANCE=1 to run the opt-in benchmark.');
  test.describe.configure({ mode: 'serial' });

  for (let sample = 1; sample <= sampleCount; sample += 1) {
    test(`records renderer readiness sample ${sample}/${sampleCount}`, async ({ goslingPage }) => {
      await goslingPage.waitForSelector('[data-testid="chat-input"]', { timeout: 30000 });
      const readinessMs = await goslingPage.evaluate(() => Math.round(performance.now()));

      readinessSamples.push(readinessMs);
      await test.info().attach(`renderer-readiness-${sample}.json`, {
        body: JSON.stringify({ sample, readinessMs }, null, 2),
        contentType: 'application/json',
      });

      expect(readinessMs).toBeGreaterThan(0);
    });
  }

  test.afterAll(() => {
    const summary = {
      metric: 'renderer navigation start to chat input availability',
      samples: readinessSamples.length,
      p50Ms: percentile(readinessSamples, 0.5),
      p95Ms: percentile(readinessSamples, 0.95),
      cacheProtocol: 'fresh Electron process per sample; OS page cache is not controlled',
    };
    console.log(`Desktop performance summary: ${JSON.stringify(summary)}`);
  });
});
