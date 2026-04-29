import { createSignal, onCleanup, type Accessor } from "solid-js";

export interface MotionValueConfig {
  initial?: number;
  animate: number | number[];
  transition?: {
    duration?: number;
    ease?: "linear" | "ease-in" | "ease-out" | "ease-in-out";
    times?: number[];
    repeat?: number;
    repeatType?: "loop" | "reverse";
    delay?: number;
  };
}

export function useMotionValue(config: MotionValueConfig): Accessor<number> {
  const keyframes = Array.isArray(config.animate)
    ? config.animate
    : [config.initial ?? 0, config.animate];
  const times = config.transition?.times ?? evenlySpaced(keyframes.length);
  const duration = (config.transition?.duration ?? 0.3) * 1000;
  const delay = (config.transition?.delay ?? 0) * 1000;
  const repeat = config.transition?.repeat ?? 0;
  const repeatType = config.transition?.repeatType ?? "loop";
  const easeType = config.transition?.ease ?? "ease-in-out";

  const [value, setValue] = createSignal(keyframes[0]!);

  const startTime = Date.now() + delay;
  const timerId = setInterval(() => {
    const elapsed = Date.now() - startTime;
    if (elapsed < 0) return;

    const rawProgress = elapsed / duration;
    const totalIterations = repeat === Infinity ? Infinity : repeat + 1;
    const iteration = Math.floor(rawProgress);

    if (totalIterations !== Infinity && iteration >= totalIterations) {
      const finalKf =
        repeatType === "reverse" && totalIterations % 2 === 0
          ? keyframes[0]!
          : keyframes[keyframes.length - 1]!;
      setValue(finalKf);
      clearInterval(timerId);
      return;
    }

    let progress = rawProgress - iteration;
    if (repeatType === "reverse" && iteration % 2 === 1) {
      progress = 1 - progress;
    }

    progress = applyEasing(progress, easeType);
    setValue(sampleKeyframes(keyframes, times, progress));
  }, 16);

  onCleanup(() => clearInterval(timerId));

  return value;
}

function evenlySpaced(n: number): number[] {
  const result: number[] = [];
  for (let i = 0; i < n; i++) {
    result.push(i / (n - 1));
  }
  return result;
}

function sampleKeyframes(
  values: number[],
  times: number[],
  progress: number,
): number {
  const p = Math.max(0, Math.min(1, progress));
  for (let i = 0; i < times.length - 1; i++) {
    const t0 = times[i]!;
    const t1 = times[i + 1]!;
    if (p >= t0 && p <= t1) {
      const segT = (p - t0) / (t1 - t0);
      return values[i]! + (values[i + 1]! - values[i]!) * segT;
    }
  }
  return values[values.length - 1]!;
}

function applyEasing(t: number, ease: string): number {
  switch (ease) {
    case "linear":
      return t;
    case "ease-in":
      return t * t;
    case "ease-out":
      return t * (2 - t);
    case "ease-in-out":
      return t < 0.5 ? 2 * t * t : -1 + (4 - 2 * t) * t;
    default:
      return t;
  }
}
