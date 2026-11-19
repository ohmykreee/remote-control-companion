import { toast as sonnerToast } from 'svelte-sonner';

export type ToastTone = 'info' | 'success' | 'error';

/** Thin wrapper so feature code keeps one call site for interface messages. */
export function toast(message: string, tone: ToastTone = 'info'): void {
  if (tone === 'error') {
    sonnerToast.error(message);
  } else if (tone === 'success') {
    sonnerToast.success(message);
  } else {
    sonnerToast(message);
  }
}
