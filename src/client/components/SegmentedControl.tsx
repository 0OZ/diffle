import type { ComponentProps } from 'react';
import { ToggleGroup, ToggleGroupItem } from '@/client/ui/toggle-group.js';

type GroupProps = Omit<ComponentProps<typeof ToggleGroup>, 'value' | 'defaultValue' | 'onValueChange' | 'multiple'>;

/**
 * A coss `ToggleGroup` that always holds one choice: pressing the selected `Segment` again keeps it,
 * where the primitive would clear it.
 */
export function SegmentedControl<T extends string>({
  value,
  onValueChange,
  ...props
}: GroupProps & { value: T; onValueChange: (value: T) => void }) {
  return (
    <ToggleGroup
      variant="outline"
      size="sm"
      {...props}
      value={[value]}
      onValueChange={(next) => {
        const [choice] = next;
        if (choice !== undefined) onValueChange(choice as T);
      }}
    />
  );
}

/** One choice of a `SegmentedControl`. */
export const Segment = ToggleGroupItem;
