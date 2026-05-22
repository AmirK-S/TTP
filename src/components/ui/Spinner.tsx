import { cn } from '../../lib/cn';

interface SpinnerProps {
  size?: number;
  className?: string;
}

export function Spinner({ size = 16, className }: SpinnerProps) {
  return (
    <span
      role="status"
      aria-label="Loading"
      style={{ width: size, height: size }}
      className={cn(
        'inline-block rounded-full border-2 border-current border-r-transparent animate-spin',
        className,
      )}
    />
  );
}
