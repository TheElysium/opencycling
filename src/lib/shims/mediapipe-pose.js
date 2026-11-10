// pose-detection imports `Pose` from this IIFE-only package, which Rolldown rejects.
// We only use MoveNet (tfjs), so the MediaPipe runtime is never reached.
export class Pose {}
