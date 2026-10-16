import { cn } from '@/client/lib/utils.js';
import { Button, type ButtonProps } from '@/client/ui/button.js';

/** What a click leaves on the button: armed for a second click, or acknowledged. Pair it with a `ButtonLabel`. */
export type Feedback = 'confirm' | 'posted' | 'copied';

/** coss has no success variant, so an acknowledged action reuses the filled look in the success colour. */
const ACKNOWLEDGED = 'border-success bg-success text-background shadow-success/24 hover:bg-success/90';

/**
 * A coss `Button` for the review's two-step and confirmed actions. `feedback` swaps in the filled variant
 * (destructive when `danger`); at rest `danger` only colours the label.
 */
export function ActionButton({
  feedback,
  danger,
  variant = 'ghost',
  className,
  ...props
}: ButtonProps & { feedback?: Feedback; danger?: boolean }) {
  const filled = feedback === 'confirm' ? (danger ? 'destructive' : 'default') : feedback ? 'default' : variant;
  return (
    <Button
      {...props}
      variant={filled}
      data-feedback={feedback}
      className={cn(
        !feedback && danger && 'text-destructive-foreground',
        (feedback === 'posted' || feedback === 'copied') && ACKNOWLEDGED,
        className,
      )}
    />
  );
}
